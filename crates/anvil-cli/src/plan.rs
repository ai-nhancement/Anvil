//! `anvil plan` subcommands (P7):
//! - `anvil plan invoke`        — invoke Planner, validate contract, render + save Plan
//! - `anvil plan review`        — invoke reviewer against Plan (reuses Charter review machinery)
//! - `anvil plan findings`      — curate Plan review findings and render disposition
//! - `anvil plan consolidate`   — absorb hardening notes, bump Plan version, save snapshot

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use crate::session::{
    connect_and_handshake, ensure_sidecar_running, find_model_binding, retrieve_api_key,
};
use crate::setup::{with_tokio, ROLE_REVIEWER_1};
use anvil_audit::{
    records::{
        ArbiterFindingResolution, CuratedFindingsRecord, PlanConsolidationRecord,
        ReviewerFindingPacket, RotationLog, VerifierResult,
    },
    AuditStore, CrossRefKey, RecordType,
};
use anvil_core::{
    config::load_config,
    error::AnvilError,
    pipeline::{
        apply_severity_tiering, check_advisory_gate, extract_findings_packet_json, verify_findings,
        AdvisoryDispositionType, CurationAction, CurationDisposition, DispositionLabel, Finding,
        FindingsPacket, VerifiedFinding, REVIEWER_SYSTEM_PROMPT,
    },
    plan::extract_planner_contract_json,
    render::{
        append_plan_hardening_history, render_disposition_doc, render_plan_doc, DispositionInput,
    },
    rotation::rotation_select,
};
use dialoguer::{Input, Select};

const PLANNER_SYSTEM_PROMPT: &str = "\
You are a rigorous project Planner. Given the approved Charter and locked Required Choices, \
produce a structured Planner Contract that decomposes the project into phases.

Each phase must include all nine required fields:
- phase_id (short alphanumeric code, e.g. P0)
- name (human-readable title)
- goal (one-sentence statement)
- action_list (array of concrete actions the Coder will take)
- deliverable (what artifact(s) the phase produces)
- acceptance_criteria (numbered conditions; each must be testable)
- dependencies (array of phase IDs this phase depends on; empty array if none)
- hinge_tests (array of deferred-decision test names; empty array if none)
- evaluation_metric_impact (which metrics this phase moves)

Produce the Planner Contract as JSON wrapped in <planner_contract>...</planner_contract> tags.

Format:
<planner_contract>
{
  \"plan_version\": \"1.0.0\",
  \"charter_ref\": \"<charter cross-ref>\",
  \"phases\": [
    {
      \"phase_id\": \"P0\",
      \"name\": \"Bootstrap\",
      \"goal\": \"Initialize the workspace.\",
      \"action_list\": [\"Create directory scaffold.\"],
      \"deliverable\": \"Scaffold present with anvil.toml.\",
      \"acceptance_criteria\": [\"anvil init succeeds.\"],
      \"dependencies\": [],
      \"hinge_tests\": [],
      \"evaluation_metric_impact\": \"None at P0.\"
    }
  ]
}
</planner_contract>";

// ── anvil plan invoke ──────────────────────────────────────────────────────────

/// Default plan file name within the project root.
pub const DEFAULT_PLAN_FILE: &str = "plan.md";

/// Runs `anvil plan invoke` — invokes the Planner model, validates the contract,
/// renders the Plan document, and writes it to `plan.md`.
///
/// Exits non-zero if no Charter `ConvergenceDeclaration` exists for `charter.md`
/// (Charter not approved), or if `charter.md` contents differ from the hash recorded
/// in the declaration (Charter modified after approval).
///
/// # Errors
///
/// Returns [`AnvilError`] on config, sidecar, model, audit-store, or validation failure.
#[allow(clippy::too_many_lines)]
pub fn run_plan_invoke(project_root: &Path) -> Result<(), AnvilError> {
    let config = load_config(project_root)?;
    let store = AuditStore::open(project_root)?;

    // Gate: Charter must be approved — find the most recent ConvergenceDeclaration for
    // charter.md (last entry in the append-only store is the latest).
    let conv_entries = store.list(RecordType::ConvergenceDeclaration)?;
    let charter_decl = conv_entries.iter().rev().find_map(|e| {
        store
            .get(&e.id)
            .ok()
            .and_then(|v| {
                serde_json::from_value::<anvil_audit::records::ConvergenceDeclaration>(v).ok()
            })
            .filter(|r| r.phase_id == "charter.md")
    });
    let Some(charter_decl) = charter_decl else {
        return Err(AnvilError::Io(std::io::Error::other(
            "Charter is not in approved state — declare convergence with \
             `anvil arbiter declare-convergence charter.md` before invoking the Planner",
        )));
    };

    // Read charter.md now — used for both the artifact-hash check and the Planner prompt.
    let charter_path = project_root.join("charter.md");
    let charter_content = std::fs::read_to_string(&charter_path).map_err(|e| {
        AnvilError::Io(std::io::Error::other(format!(
            "charter.md not found at {} — run `anvil discuss` first: {e}",
            charter_path.display()
        )))
    })?;

    // If the declaration recorded an artifact hash, verify the current charter matches.
    if let Some(ref approved_hash) = charter_decl.artifact_hash {
        let current_hash = crate::utils::sha256_hex(charter_content.as_bytes());
        if &current_hash != approved_hash {
            return Err(AnvilError::Io(std::io::Error::other(
                "charter.md has been modified since the convergence declaration — \
                 re-review and re-declare convergence before invoking the Planner",
            )));
        }
    }

    let choices_summary: String = config
        .choices
        .iter()
        .map(|(k, v)| format!("- {k}: {}", v.value))
        .collect::<Vec<_>>()
        .join("\n");

    // Use the first configured binding (planner or coder).
    let planner_binding_name = config
        .model_bindings
        .iter()
        .find(|b| b.name.starts_with("planner-") || b.name.starts_with("coder-"))
        .or_else(|| config.model_bindings.first())
        .map_or_else(|| ROLE_REVIEWER_1.to_owned(), |b| b.name.clone());

    let binding = find_model_binding(&config, &planner_binding_name)?;
    let conn_name = binding.provider_connection.clone();
    let model_id = binding.model_identity.clone();
    let conn = config
        .provider_connections
        .get(&conn_name)
        .ok_or_else(|| AnvilError::ProviderConnectionMissing(conn_name.clone()))?;
    let api_key = retrieve_api_key(&conn_name, &conn.credential_ref)?;

    println!("Invoking Planner '{planner_binding_name}'…");

    let user_message = format!(
        "Approved Charter:\n\n{charter_content}\n\n\
         Locked Required Choices:\n\n{choices_summary}\n\n\
         Produce the Planner Contract as described."
    );

    let port = ensure_sidecar_running(project_root, &config)?;
    let mut client = connect_and_handshake(port, &config)?;

    let response = with_tokio(invoke_model(
        &mut client,
        PLANNER_SYSTEM_PROMPT,
        &user_message,
        &model_id,
        &conn_name,
        &api_key,
    ))?;

    // Extract and parse contract — parse_planner_contract gives a precise field-level error.
    let contract_json = extract_planner_contract_json(&response)
        .ok_or_else(|| AnvilError::ModelResponseMissingPacket("planner_contract".to_owned()))?;

    let contract = anvil_core::plan::parse_planner_contract(contract_json).map_err(|e| {
        eprintln!("error: Planner Contract invalid: {e}");
        e
    })?;

    println!("  Contract valid: {} phase(s).", contract.phases.len());

    // Render and write Plan document.
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let plan_doc = render_plan_doc(&contract, &today);
    let plan_path = project_root.join(DEFAULT_PLAN_FILE);
    std::fs::write(&plan_path, plan_doc.as_bytes())?;

    // Persist the contract JSON for graph commands.
    let anvil_dir = project_root.join(".anvil");
    std::fs::create_dir_all(&anvil_dir)?;
    let contract_json = serde_json::to_string_pretty(&contract)?;
    std::fs::write(
        anvil_dir.join("plan_contract.json"),
        contract_json.as_bytes(),
    )?;

    println!("✓ Plan written to {}", plan_path.display());
    println!("\nNext step: `anvil plan review` to start the review cycle.");

    Ok(())
}

// ── anvil plan review ──────────────────────────────────────────────────────────

/// Runs `anvil plan review` — invokes the reviewer model against the Plan document.
///
/// Reuses the Charter review machinery: same finding/curation/verifier/disposition cycle.
///
/// # Errors
///
/// Returns [`AnvilError`] on config, sidecar, model, or audit-store failure.
#[allow(clippy::too_many_lines)]
pub fn run_plan_review(project_root: &Path) -> Result<(), AnvilError> {
    let plan_file = DEFAULT_PLAN_FILE;
    let config = load_config(project_root)?;

    let pool: Vec<String> = if config.reviewer_pool.is_empty() {
        vec![ROLE_REVIEWER_1.to_owned()]
    } else {
        config.reviewer_pool.clone()
    };

    let store = AuditStore::open(project_root)?;

    // Count existing plan RFPs to determine round number.
    let existing_packets = store.list(RecordType::ReviewerFindingPacket)?;
    let plan_rfp_count = existing_packets
        .iter()
        .filter(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<ReviewerFindingPacket>(v).ok())
                .is_some_and(|rfp| rfp.packet.artifact_ref.starts_with(plan_file))
        })
        .count();
    let round_number = u32::try_from(plan_rfp_count).unwrap_or(u32::MAX) + 1;

    let reviewer_binding_name = rotation_select(&pool, round_number)
        .ok_or(AnvilError::ReviewerPoolEmpty)?
        .to_owned();
    let prev_reviewer: Option<String> = if round_number > 1 {
        Some(
            rotation_select(&pool, round_number - 1)
                .unwrap_or(&reviewer_binding_name)
                .to_owned(),
        )
    } else {
        None
    };

    let binding = find_model_binding(&config, &reviewer_binding_name)?;
    let conn_name = binding.provider_connection.clone();
    let model_id = binding.model_identity.clone();
    let conn = config
        .provider_connections
        .get(&conn_name)
        .ok_or_else(|| AnvilError::ProviderConnectionMissing(conn_name.clone()))?;
    let api_key = retrieve_api_key(&conn_name, &conn.credential_ref)?;

    let plan_path = project_root.join(plan_file);
    let plan_content = std::fs::read_to_string(&plan_path).map_err(|e| {
        AnvilError::Io(std::io::Error::other(format!(
            "{plan_file} not found at {} — run `anvil plan invoke` first: {e}",
            plan_path.display()
        )))
    })?;
    if plan_content.trim().is_empty() {
        return Err(AnvilError::Io(std::io::Error::other(format!(
            "{plan_file} is empty — run `anvil plan invoke` first"
        ))));
    }

    let plan_hash = crate::utils::sha256_hex(plan_content.as_bytes());

    // Load Arbiter-Decided findings for the plan artifact.
    let arbiter_entries = store.list(RecordType::ArbiterFindingResolution)?;
    let mut arbiter_briefing = String::new();
    for entry in &arbiter_entries {
        if let Ok(val) = store.get(&entry.id) {
            if let Ok(record) = serde_json::from_value::<ArbiterFindingResolution>(val) {
                writeln!(
                    arbiter_briefing,
                    "- Finding {}: {}. Reasoning: {}",
                    record.finding_id, record.chosen_direction_summary, record.reasoning
                )
                .ok();
            }
        }
    }

    println!("Invoking reviewer '{reviewer_binding_name}' for plan R{round_number}…");

    let port = ensure_sidecar_running(project_root, &config)?;
    let mut client = connect_and_handshake(port, &config)?;

    let user_message = if arbiter_briefing.is_empty() {
        format!(
            "Please review the following Plan document (round {round_number}):\n\n{plan_content}"
        )
    } else {
        format!(
            "Please review the following Plan document (round {round_number}).\n\n\
             Arbiter-Decided findings (already resolved):\n\n{arbiter_briefing}\n\
             Plan document:\n\n{plan_content}"
        )
    };

    let response = with_tokio(invoke_model(
        &mut client,
        REVIEWER_SYSTEM_PROMPT,
        &user_message,
        &model_id,
        &conn_name,
        &api_key,
    ))?;

    let packet_json = extract_findings_packet_json(&response)
        .ok_or_else(|| AnvilError::ModelResponseMissingPacket("findings_packet".to_owned()))?;

    let partial: PartialFindingsPacket =
        serde_json::from_str(packet_json).map_err(|e| AnvilError::ModelResponseBadJson {
            reason: e.to_string(),
        })?;

    let reviewer_model_identity = if partial.reviewer_model_identity.trim().is_empty() {
        model_id.clone()
    } else {
        partial.reviewer_model_identity
    };

    let mut packet = FindingsPacket::new(
        format!("{plan_file}:R{round_number}"),
        round_number,
        partial.reviewer_id,
        reviewer_model_identity,
        partial.findings,
    );
    packet.artifact_hash = Some(plan_hash);
    apply_severity_tiering(&mut packet, round_number);

    let finding_count = packet.findings.len();
    let advisory_count = packet.findings.iter().filter(|f| f.advisory).count();
    println!("  Reviewer produced {finding_count} finding(s) ({advisory_count} advisory).");

    let verified_findings = verify_findings(&packet.findings, project_root);
    let grounded = verified_findings
        .iter()
        .filter(|vf| vf.outcome == anvil_core::pipeline::VerificationOutcome::Grounded)
        .count();
    let refuted = verified_findings
        .iter()
        .filter(|vf| vf.outcome == anvil_core::pipeline::VerificationOutcome::Refuted)
        .count();
    println!(
        "  Verifier: {grounded} grounded, {refuted} refuted, {} unverifiable.",
        finding_count - grounded - refuted
    );

    let cross_ref_key =
        CrossRefKey::new(plan_file, "§root", &format!("R{round_number}")).to_key_string();
    let cross_refs = vec![cross_ref_key];

    let rfp_record = ReviewerFindingPacket::from_packet(
        format!("plan-R{round_number}"),
        packet.clone(),
        cross_refs.clone(),
    );
    store.append(&rfp_record)?;

    let vr_record = VerifierResult::from_verified(
        format!("plan-R{round_number}"),
        "local-verifier-v1".to_owned(),
        packet.packet_id.clone(),
        verified_findings,
        cross_refs,
    );
    store.append(&vr_record)?;

    let rotation_log = RotationLog::new(
        prev_reviewer,
        reviewer_binding_name.clone(),
        format!("round-robin selection for plan R{round_number}"),
        round_number,
        vec![CrossRefKey::new(plan_file, "§root", &format!("R{round_number}")).to_key_string()],
    );
    store.append(&rotation_log)?;

    println!("\n✓ Findings stored:");
    println!("  ReviewerFindingPacket: {}", rfp_record.id);
    println!("  VerifierResult:        {}", vr_record.id);
    println!("  RotationLog:           {}", rotation_log.id);
    println!("\nNext step: `anvil plan findings` to curate and render the disposition.");

    Ok(())
}

// ── anvil plan findings ────────────────────────────────────────────────────────

/// Runs `anvil plan findings` — interactively curates Plan review findings and
/// renders the disposition document. Reuses Charter findings machinery.
///
/// # Errors
///
/// Returns [`AnvilError`] on config, audit-store, curation, or I/O failure.
#[allow(clippy::too_many_lines)]
pub fn run_plan_findings(project_root: &Path, non_interactive: bool) -> Result<(), AnvilError> {
    let plan_file = DEFAULT_PLAN_FILE;
    let store = AuditStore::open(project_root)?;

    // Load latest plan RFP + paired VR.
    let rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
    let rfp_entry = rfp_entries
        .iter()
        .rev()
        .find(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<ReviewerFindingPacket>(v).ok())
                .is_some_and(|rfp| rfp.packet.artifact_ref.starts_with(plan_file))
        })
        .ok_or_else(|| AnvilError::NoFindingsPacket(plan_file.to_owned()))?;
    let rfp: ReviewerFindingPacket =
        serde_json::from_value(store.get(&rfp_entry.id)?).map_err(|e| {
            AnvilError::ModelResponseBadJson {
                reason: format!("ReviewerFindingPacket corrupt: {e}"),
            }
        })?;

    let vr_entries = store.list(RecordType::VerifierResult)?;
    let vr_entry = vr_entries
        .iter()
        .rev()
        .find(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<VerifierResult>(v).ok())
                .is_some_and(|vr| vr.source_packet_id == rfp.packet.packet_id)
        })
        .ok_or_else(|| {
            AnvilError::Io(std::io::Error::other(
                "no VerifierResult found for latest plan RFP — re-run `anvil plan review`",
            ))
        })?;
    let vr: VerifierResult = serde_json::from_value(store.get(&vr_entry.id)?).map_err(|e| {
        AnvilError::ModelResponseBadJson {
            reason: format!("VerifierResult corrupt: {e}"),
        }
    })?;

    let verified_findings: Vec<VerifiedFinding> = vr.verified_findings;
    let round_number = rfp.packet.round_number;
    let reviewer_id = rfp.packet.reviewer_id.clone();

    println!(
        "Curating {plan_file} review R{round_number} ({} finding(s)):",
        verified_findings.len()
    );
    println!();

    let CurationResult {
        actions: curation_actions,
        disposition_map,
        dispositions,
        advisory_dispositions,
    } = curate_findings(&verified_findings, non_interactive)?;

    let NarrativeInputs {
        narrative_summary,
        corrections,
        residual_notes,
        reproducibility,
        bottom_line,
    } = collect_narrative(non_interactive)?;

    // Advisory gate check BEFORE any file writes.
    let missing_advisory = check_advisory_gate(&dispositions, &rfp.packet.findings);
    if !missing_advisory.is_empty() {
        eprintln!(
            "error: advisory gate check failed — {} advisory finding(s) lack explicit or \
             complete disposition:",
            missing_advisory.len()
        );
        for id in &missing_advisory {
            eprintln!("  {id}");
        }
        eprintln!(
            "Re-run `anvil plan findings` and provide Accept-Advisory, or \
             Drop-Advisory/Defer-Advisory with non-empty reason/target for each advisory finding."
        );
        return Err(AnvilError::Io(std::io::Error::other(
            "advisory gate check failed — one or more advisory findings lack complete disposition",
        )));
    }

    // Render disposition document.
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let disp_input = DispositionInput {
        artifact_name: "plan",
        round_number,
        reviewer_id: &reviewer_id,
        date: &today,
        verified_findings: &verified_findings,
        disposition_map: &disposition_map,
        files_changed: &[],
        narrative_summary: &narrative_summary,
        corrections: &corrections,
        residual_notes: &residual_notes,
        reproducibility_commands: &reproducibility,
        bottom_line: &bottom_line,
        curation_actions: &curation_actions,
        advisory_dispositions: &advisory_dispositions,
    };
    let doc = render_disposition_doc(&disp_input);

    let reviews_dir = project_root.join("reviews");
    std::fs::create_dir_all(&reviews_dir)?;
    let disp_path = reviews_dir.join(format!("REVIEW_plan_R{round_number}.md"));
    std::fs::write(&disp_path, doc.as_bytes())?;

    // Append plan hardening history.
    append_plan_hardening_history(
        project_root,
        round_number,
        &reviewer_id,
        &today,
        &narrative_summary,
    )?;

    // Persist CuratedFindingsRecord.
    let cross_ref =
        CrossRefKey::new(plan_file, "§root", &format!("R{round_number}")).to_key_string();
    let curated = CuratedFindingsRecord::new(
        rfp.packet.packet_id.clone(),
        "coordinator".to_owned(),
        dispositions,
        vec![cross_ref],
    );
    store.append(&curated)?;

    println!("\n✓ Disposition written to {}", disp_path.display());
    println!("  CuratedFindingsRecord: {}", curated.id);

    Ok(())
}

// ── anvil plan consolidate ─────────────────────────────────────────────────────

/// Runs `anvil plan consolidate` — absorbs accumulated hardening notes into the
/// Plan body, bumps the Plan version, and stores a `PlanConsolidationRecord` with
/// the prior Plan snapshot (making the prior version queryable).
///
// hinge_test: pins=plan_consolidation_preserves_provenance, intended=version-provenance, phase=P7
/// # Errors
///
/// Returns [`AnvilError`] on I/O or audit-store failure.
pub fn run_plan_consolidate(project_root: &Path, trigger: &str) -> Result<(), AnvilError> {
    let plan_path = project_root.join(DEFAULT_PLAN_FILE);
    let prior_plan = std::fs::read_to_string(&plan_path).map_err(|e| {
        AnvilError::Io(std::io::Error::other(format!(
            "{DEFAULT_PLAN_FILE} not found — run `anvil plan invoke` first: {e}"
        )))
    })?;

    // Extract current version from the plan header line "# Anvil Plan — vX.Y.Z".
    let version_from = extract_plan_version(&prior_plan).unwrap_or_else(|| "1.0.0".to_owned());
    let version_to = bump_minor_version(&version_from);

    // Read accumulated hardening notes.
    let history_path = project_root.join("PLAN_HARDENING_HISTORY.md");
    let hardening_content = std::fs::read_to_string(&history_path).unwrap_or_default();

    // Extract which round numbers are present in the hardening history.
    let absorbed_rounds: Vec<u32> = extract_round_numbers(&hardening_content);

    // Build the consolidated plan: update version header and append a Hardening Notes section.
    let consolidated = consolidate_plan_content(&prior_plan, &version_to, &hardening_content);

    // Store the PlanConsolidationRecord for provenance BEFORE mutating files.
    // If the audit store is unavailable, no file changes occur.
    let store = AuditStore::open(project_root)?;
    let cross_ref = CrossRefKey::new(DEFAULT_PLAN_FILE, "§root", &version_to).to_key_string();
    let record = PlanConsolidationRecord::new(
        version_from.clone(),
        version_to.clone(),
        trigger.to_owned(),
        absorbed_rounds,
        prior_plan,
        vec![cross_ref],
    );
    store.append(&record)?;

    // Provenance record is durable — now commit file mutations.
    std::fs::write(&plan_path, consolidated.as_bytes())?;

    // Clear the hardening history (entries absorbed).
    std::fs::write(&history_path, b"")?;

    println!("✓ Plan consolidated: v{version_from} → v{version_to}");
    println!("  Trigger: {trigger}");
    println!("  PlanConsolidationRecord: {}", record.id);
    println!(
        "  Prior version queryable via: anvil audit show {}",
        record.id
    );

    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn extract_plan_version(plan_content: &str) -> Option<String> {
    for line in plan_content.lines() {
        if let Some(rest) = line.strip_prefix("# Anvil Plan — v") {
            let version = rest.split_whitespace().next()?.to_owned();
            return Some(version);
        }
    }
    None
}

fn bump_minor_version(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() == 3 {
        let major = parts[0];
        let minor: u32 = parts[1].parse().unwrap_or(0);
        let patch = parts[2];
        // Reset patch on minor bump only if patch is 0; otherwise preserve.
        let _ = patch;
        return format!("{major}.{}.0", minor + 1);
    }
    format!("{version}.1")
}

fn extract_round_numbers(hardening_content: &str) -> Vec<u32> {
    hardening_content
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("## R")?;
            rest.split_whitespace()
                .next()
                .and_then(|s| s.trim_end_matches(" —").parse::<u32>().ok())
        })
        .collect()
}

fn consolidate_plan_content(
    prior_plan: &str,
    new_version: &str,
    hardening_content: &str,
) -> String {
    // Replace the version header line.
    let updated: String = prior_plan
        .lines()
        .map(|line| {
            if line.starts_with("# Anvil Plan — v") {
                format!("# Anvil Plan — v{new_version}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    if hardening_content.trim().is_empty() {
        return updated;
    }

    // Append a Hardening Notes section.
    format!(
        "{updated}\n\n## Hardening Notes (consolidated)\n\n{}\n",
        hardening_content.trim()
    )
}

// ── Shared sidecar invocation (reused by plan review) ─────────────────────────

#[derive(serde::Deserialize)]
struct PartialFindingsPacket {
    #[serde(default = "default_reviewer_id")]
    reviewer_id: String,
    #[serde(default)]
    reviewer_model_identity: String,
    #[serde(default)]
    findings: Vec<Finding>,
}

fn default_reviewer_id() -> String {
    "reviewer-1".to_owned()
}

async fn invoke_model(
    client: &mut anvil_sidecar_client::client::AnvilSidecarClient,
    system_prompt: &str,
    user_message: &str,
    model_id: &str,
    conn_name: &str,
    api_key: &str,
) -> Result<String, AnvilError> {
    use anvil_sidecar_client::proto::{self, invoke_request::Payload};
    let request = proto::InvokeRequest {
        idempotency_key: String::new(),
        model_id: model_id.to_owned(),
        provider_connection_id: conn_name.to_owned(),
        credentials: Some(proto::Credentials {
            credential: Some(proto::credentials::Credential::ApiKey(api_key.to_owned())),
        }),
        timeout: Some(proto::Timeout { millis: 480_000 }),
        payload: Some(Payload::Chat(proto::ChatRequest {
            system_prompt: system_prompt.to_owned(),
            messages: vec![proto::Message {
                role: "user".to_owned(),
                content: user_message.to_owned(),
            }],
            max_tokens: Some(8192),
            temperature: None,
        })),
    };

    let resp = client
        .invoke(request)
        .await
        .map_err(|e| AnvilError::Io(std::io::Error::other(format!("invoke: {e}"))))?;

    match resp.result {
        Some(proto::invoke_response::Result::Chat(ref chat)) => Ok(chat.content.clone()),
        Some(proto::invoke_response::Result::Error(ref e)) => Err(AnvilError::Io(
            std::io::Error::other(format!("model error: {}", e.message)),
        )),
        None => Err(AnvilError::Io(std::io::Error::other(
            "sidecar invoke returned no result",
        ))),
        Some(_) => Err(AnvilError::Io(std::io::Error::other(
            "sidecar invoke returned unexpected result variant",
        ))),
    }
}

// ── Curation helpers (mirrors charter.rs) ─────────────────────────────────────

struct CurationResult {
    actions: BTreeMap<String, CurationAction>,
    disposition_map: BTreeMap<String, DispositionLabel>,
    dispositions: Vec<CurationDisposition>,
    advisory_dispositions: BTreeMap<String, (AdvisoryDispositionType, Option<String>)>,
}

#[allow(clippy::too_many_lines)]
fn curate_findings(
    verified_findings: &[VerifiedFinding],
    non_interactive: bool,
) -> Result<CurationResult, AnvilError> {
    let mut actions: BTreeMap<String, CurationAction> = BTreeMap::new();
    let mut disposition_map: BTreeMap<String, DispositionLabel> = BTreeMap::new();
    let mut dispositions: Vec<CurationDisposition> = Vec::new();
    let mut advisory_dispositions: BTreeMap<String, (AdvisoryDispositionType, Option<String>)> =
        BTreeMap::new();

    for vf in verified_findings {
        let f = &vf.finding;
        let advisory_badge = if f.advisory { " [ADVISORY]" } else { "" };
        println!(
            "Finding {} [{}{}]: {}",
            f.id,
            f.severity.as_str(),
            advisory_badge,
            f.claim
        );
        println!("  Evidence:       {}", f.evidence);
        println!("  Recommendation: {}", f.recommendation);
        println!(
            "  Verification:   {} — {}",
            vf.outcome.as_str(),
            vf.evidence_note
        );
        println!();

        let (action, annotation, advisory_disposition) = if non_interactive {
            if f.advisory {
                println!("  Advisory disposition: Accept-Advisory (non-interactive default)");
                (CurationAction::Keep, None, Some(AdvisoryDispositionType::AcceptAdvisory))
            } else {
                println!("  Action: Keep (non-interactive default)");
                (CurationAction::Keep, None, None)
            }
        } else if f.advisory {
            let adv_idx = Select::new()
                .with_prompt("  Advisory disposition")
                .items(&["Accept-Advisory", "Drop-Advisory", "Defer-Advisory"])
                .default(0)
                .interact()
                .map_err(|_| AnvilError::SetupCancelled)?;
            let adv_type = match adv_idx {
                1 => AdvisoryDispositionType::DropAdvisory,
                2 => AdvisoryDispositionType::DeferAdvisory,
                _ => AdvisoryDispositionType::AcceptAdvisory,
            };
            let note: String = match adv_type {
                AdvisoryDispositionType::AcceptAdvisory => Input::new()
                    .with_prompt("  Note (optional)")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|_| AnvilError::SetupCancelled)?,
                AdvisoryDispositionType::DropAdvisory => Input::new()
                    .with_prompt("  Reason (required)")
                    .allow_empty(false)
                    .interact_text()
                    .map_err(|_| AnvilError::SetupCancelled)?,
                AdvisoryDispositionType::DeferAdvisory => Input::new()
                    .with_prompt("  Target phase (required)")
                    .allow_empty(false)
                    .interact_text()
                    .map_err(|_| AnvilError::SetupCancelled)?,
            };
            let annotation = if note.is_empty() { None } else { Some(note) };
            let act = match adv_type {
                AdvisoryDispositionType::DropAdvisory => CurationAction::Drop,
                _ => CurationAction::Keep,
            };
            (act, annotation, Some(adv_type))
        } else {
            let action_idx = Select::new()
                .with_prompt("  Action")
                .items(&["Keep", "Drop", "Annotate"])
                .default(0)
                .interact()
                .map_err(|_| AnvilError::SetupCancelled)?;
            let action = match action_idx {
                1 => CurationAction::Drop,
                2 => CurationAction::Annotate,
                _ => CurationAction::Keep,
            };
            let annotation = if matches!(action, CurationAction::Drop | CurationAction::Annotate) {
                let note: String = Input::new()
                    .with_prompt("  Note")
                    .allow_empty(true)
                    .interact_text()
                    .map_err(|_| AnvilError::SetupCancelled)?;
                if note.is_empty() {
                    None
                } else {
                    Some(note)
                }
            } else {
                None
            };
            (action, annotation, None)
        };

        if !f.advisory && matches!(action, CurationAction::Keep) {
            let label = if non_interactive {
                println!("  Disposition label: Locked in Plan (non-interactive default)");
                DispositionLabel::LockedPendingPlan
            } else {
                let label_idx = Select::new()
                    .with_prompt("  Disposition label")
                    .items(&[
                        "Fixed",
                        "Locked in Charter (pending Plan)",
                        "Refuted",
                        "Deferred",
                    ])
                    .default(0)
                    .interact()
                    .map_err(|_| AnvilError::SetupCancelled)?;
                match label_idx {
                    1 => DispositionLabel::LockedPendingPlan,
                    2 => DispositionLabel::Refuted,
                    3 => DispositionLabel::Deferred,
                    _ => DispositionLabel::Fixed,
                }
            };
            disposition_map.insert(f.id.clone(), label);
        }

        actions.insert(f.id.clone(), action.clone());
        if let Some(ref adv) = advisory_disposition {
            advisory_dispositions.insert(f.id.clone(), (adv.clone(), annotation.clone()));
        }
        dispositions.push(CurationDisposition {
            finding_id: f.id.clone(),
            action,
            edited_finding: None,
            annotation,
            advisory_disposition,
        });

        println!();
    }

    Ok(CurationResult {
        actions,
        disposition_map,
        dispositions,
        advisory_dispositions,
    })
}

struct NarrativeInputs {
    narrative_summary: String,
    corrections: String,
    residual_notes: String,
    reproducibility: String,
    bottom_line: String,
}

fn collect_narrative(non_interactive: bool) -> Result<NarrativeInputs, AnvilError> {
    if non_interactive {
        return Ok(NarrativeInputs {
            narrative_summary: String::new(),
            corrections: String::new(),
            residual_notes: String::new(),
            reproducibility: String::new(),
            bottom_line: String::new(),
        });
    }
    let narrative_summary: String = Input::new()
        .with_prompt("Narrative summary (what changed this round)")
        .allow_empty(true)
        .interact_text()
        .map_err(|_| AnvilError::SetupCancelled)?;
    let corrections: String = Input::new()
        .with_prompt("Corrections to prior narrative (optional)")
        .allow_empty(true)
        .interact_text()
        .map_err(|_| AnvilError::SetupCancelled)?;
    let residual_notes: String = Input::new()
        .with_prompt("Residual / deferred notes (optional)")
        .allow_empty(true)
        .interact_text()
        .map_err(|_| AnvilError::SetupCancelled)?;
    let reproducibility: String = Input::new()
        .with_prompt("Reproducibility commands (optional)")
        .allow_empty(true)
        .interact_text()
        .map_err(|_| AnvilError::SetupCancelled)?;
    let bottom_line: String = Input::new()
        .with_prompt("Bottom line")
        .allow_empty(true)
        .interact_text()
        .map_err(|_| AnvilError::SetupCancelled)?;

    Ok(NarrativeInputs {
        narrative_summary,
        corrections,
        residual_notes,
        reproducibility,
        bottom_line,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_audit::records::ALL_RECORD_TYPES;

    fn init_store() -> (tempfile::TempDir, AuditStore) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::create_dir_all(root.join("audit-store")).unwrap();
        for rt in ALL_RECORD_TYPES {
            std::fs::create_dir_all(root.join("audit-store").join(rt.dir_name())).unwrap();
        }
        std::fs::write(root.join("audit-store/_index.json"), b"{\"records\":[]}\n").unwrap();
        AuditStore::open(root).expect("store");
        let store = AuditStore::open(root).expect("store");
        (tmp, store)
    }

    // hinge_test: pins=plan_consolidation_preserves_provenance, intended=version-provenance, phase=P7
    #[test]
    fn test_plan_consolidation_preserves_provenance() {
        // Pins: run_plan_consolidate must store a PlanConsolidationRecord containing
        // the full prior Plan text, making the previous version queryable from the audit store.
        // Flipping requires updating the consolidation logic and this test together.
        let (tmp, store) = init_store();
        let root = tmp.path();

        let prior_plan = "# Anvil Plan — v1.0.0\n\nOriginal content.";
        std::fs::write(root.join(DEFAULT_PLAN_FILE), prior_plan).unwrap();
        std::fs::write(
            root.join("PLAN_HARDENING_HISTORY.md"),
            "\n## R1 — 2026-05-26 (reviewer: reviewer-1)\n\nFixed P0 criteria.\n",
        )
        .unwrap();
        std::fs::write(root.join("anvil.toml"), "[choices]\n").unwrap();

        run_plan_consolidate(root, "end-of-P7").expect("consolidate");

        // PlanConsolidationRecord must be stored.
        let entries = store.list(RecordType::PlanConsolidation).expect("list");
        assert_eq!(entries.len(), 1, "exactly one consolidation record");

        let val = store.get(&entries[0].id).expect("get");
        let record: PlanConsolidationRecord = serde_json::from_value(val).expect("deserialize");

        assert_eq!(record.plan_version_from, "1.0.0");
        assert_eq!(record.plan_version_to, "1.1.0");
        assert_eq!(record.trigger, "end-of-P7");
        assert_eq!(
            record.prior_plan_snapshot, prior_plan,
            "prior plan text must be preserved verbatim"
        );
        assert!(
            record.hardening_rounds_absorbed.contains(&1),
            "round 1 must be recorded as absorbed"
        );

        // New plan must have bumped version.
        let new_plan = std::fs::read_to_string(root.join(DEFAULT_PLAN_FILE)).unwrap();
        assert!(
            new_plan.contains("v1.1.0"),
            "new plan must show bumped version"
        );

        // Hardening history must be cleared.
        let history = std::fs::read_to_string(root.join("PLAN_HARDENING_HISTORY.md")).unwrap();
        assert!(
            history.is_empty(),
            "hardening history must be cleared after consolidation"
        );
    }

    #[test]
    fn test_extract_plan_version() {
        let content = "# Anvil Plan — v2.3.1\n\nSome content.";
        assert_eq!(extract_plan_version(content), Some("2.3.1".to_owned()));
        assert_eq!(extract_plan_version("no version header"), None);
    }

    #[test]
    fn test_bump_minor_version() {
        assert_eq!(bump_minor_version("1.0.0"), "1.1.0");
        assert_eq!(bump_minor_version("2.5.3"), "2.6.0");
    }

    #[test]
    fn test_extract_round_numbers() {
        let content = "\n## R1 — 2026-05-01 (reviewer: r1)\n\n## R3 — 2026-05-10\n";
        let rounds = extract_round_numbers(content);
        assert!(rounds.contains(&1));
        assert!(rounds.contains(&3));
    }

    #[test]
    fn test_consolidate_plan_content_bumps_version() {
        let prior = "# Anvil Plan — v1.0.0\n\nContent.";
        let result = consolidate_plan_content(prior, "1.1.0", "");
        assert!(result.contains("v1.1.0"));
        assert!(!result.contains("v1.0.0"));
    }

    #[test]
    fn test_consolidate_plan_content_appends_notes() {
        let prior = "# Anvil Plan — v1.0.0\n\nContent.";
        let hardening = "## R1 — date\n\nNote content.";
        let result = consolidate_plan_content(prior, "1.1.0", hardening);
        assert!(result.contains("Hardening Notes"));
        assert!(result.contains("Note content."));
    }

    #[test]
    fn test_plan_invoke_rejects_unapproved_charter() {
        let (tmp, _store) = init_store();
        std::fs::write(tmp.path().join("anvil.toml"), "[choices]\n").unwrap();
        // No ConvergenceDeclaration → must fail.
        let result = run_plan_invoke(tmp.path());
        assert!(result.is_err(), "unapproved charter must be rejected");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not in approved state"),
            "error must mention approved state: {msg}"
        );
    }

    #[test]
    fn test_plan_invoke_charter_gate_passes_with_declaration() {
        // Gate must pass when a ConvergenceDeclaration for charter.md exists.
        // (Fails at the next step — missing charter.md — not at the gate.)
        let (tmp, store) = init_store();
        std::fs::write(tmp.path().join("anvil.toml"), "[choices]\n").unwrap();

        let decl = anvil_audit::records::ConvergenceDeclaration::new(
            "charter.md".to_owned(),
            3,
            "all P1s resolved".to_owned(),
            0,
            0,
            vec![],
            None,
        );
        store.append(&decl).expect("store declaration");

        let result = run_plan_invoke(tmp.path());
        // Gate passed — fails at missing charter.md, not at the approved-state check.
        let msg = result.unwrap_err().to_string();
        assert!(
            !msg.contains("not in approved state"),
            "gate must have passed; error should be about missing charter.md, got: {msg}"
        );
        assert!(
            msg.contains("charter.md"),
            "error must be about missing charter.md: {msg}"
        );
    }

    #[test]
    fn test_plan_invoke_charter_gate_fails_with_modified_charter() {
        let (tmp, store) = init_store();
        std::fs::write(tmp.path().join("anvil.toml"), "[choices]\n").unwrap();

        let charter_content_a = "# Charter v1\n\nOriginal approved content.\n";
        let hash_a = crate::utils::sha256_hex(charter_content_a.as_bytes());

        // Create declaration recording the hash of charter state A.
        let decl = anvil_audit::records::ConvergenceDeclaration::new(
            "charter.md".to_owned(),
            1,
            "all findings resolved".to_owned(),
            0,
            0,
            vec![],
            Some(hash_a),
        );
        store.append(&decl).expect("store declaration");

        // Write charter.md with DIFFERENT content (post-declaration edit).
        std::fs::write(
            tmp.path().join("charter.md"),
            "# Charter v1\n\nMODIFIED content.\n",
        )
        .unwrap();

        let result = run_plan_invoke(tmp.path());
        assert!(result.is_err(), "modified charter must be rejected");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("modified since the convergence declaration"),
            "error must mention modification: {msg}"
        );
    }
}
