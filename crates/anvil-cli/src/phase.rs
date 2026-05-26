//! `anvil phase` subcommands (P8):
//! - `anvil phase build <id> [--format json] [--describe-schema]`
//! - `anvil phase review <id> [--project <path>]`
//! - `anvil phase findings <id> [--project <path>]`
//! - `anvil phase ship <id> [--project <path>]`

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use anvil_audit::{
    records::{
        ArbiterFindingResolution, CuratedFindingsRecord, GateApproval, PhaseDisposition,
        ReviewerFindingPacket, RotationLog, VerifierResult,
    },
    AuditStore, CrossRefKey, RecordType,
};
use anvil_core::{
    config::load_config,
    error::AnvilError,
    phase_briefing::{
        parse_phase_briefing_contract, render_phase_briefing_doc, validate_phase_briefing_contract,
    },
    pipeline::{
        apply_severity_tiering, check_advisory_gate, extract_findings_packet_json, verify_findings,
        AdvisoryDispositionType, CurationAction, CurationDisposition, DispositionLabel,
        FindingsPacket, VerifiedFinding, REVIEWER_SYSTEM_PROMPT,
    },
    render::{render_disposition_doc, DispositionInput},
    rotation::rotation_select,
};
use anvil_sidecar_client::proto::{self, invoke_request::Payload};
use dialoguer::{Input, Select};

use crate::arbiter::filter_rfps_by_artifact;
use crate::session::{
    connect_and_handshake, ensure_sidecar_running, find_model_binding, retrieve_api_key,
};
use crate::setup::{with_tokio, ROLE_CODER, ROLE_REVIEWER_1};
use crate::status::check_full_pool_clean;

/// Output format for `anvil phase build`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

/// Embedded JSON Schema for `PhaseBriefingContract` (Amendment A1 — `--describe-schema`).
pub const PHASE_BUILD_SCHEMA: &str = include_str!("../../../schemas/cli/phase_build.json");

// ── Private gate helpers ───────────────────────────────────────────────────────

/// Counts `phase-{id}-briefing-sent` gate records. Used to derive the build round number
/// independently from RFP count, preventing briefing overwrite (F4) and stale-ship (F2).
fn count_phase_briefing_rounds(store: &AuditStore, phase_id: &str) -> Result<u32, AnvilError> {
    let gate_name = format!("phase-{phase_id}-briefing-sent");
    let entries = store.list(RecordType::GateApproval)?;
    let count = entries
        .iter()
        .filter(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<GateApproval>(v).ok())
                .is_some_and(|g| g.gate_name == gate_name)
        })
        .count();
    Ok(u32::try_from(count).unwrap_or(u32::MAX))
}

/// Returns `true` if at least one `GateApproval` record with `gate_name` exists.
fn phase_gate_exists(store: &AuditStore, gate_name: &str) -> Result<bool, AnvilError> {
    let entries = store.list(RecordType::GateApproval)?;
    Ok(entries.iter().any(|e| {
        store
            .get(&e.id)
            .ok()
            .and_then(|v| serde_json::from_value::<GateApproval>(v).ok())
            .is_some_and(|g| g.gate_name == gate_name)
    }))
}

// ── anvil phase build ──────────────────────────────────────────────────────────

/// Runs `anvil phase build <phase_id>`.
///
/// Round number is derived from existing `phase-{id}-briefing-sent` gate count + 1 (not RFP
/// count), so each build targets a unique file and cannot overwrite a prior briefing (F4 fix).
///
/// # Errors
///
/// Returns [`AnvilError`] on config, sidecar, validation, or audit-store failure.
pub fn run_phase_build(
    project_root: &Path,
    phase_id: &str,
    format: OutputFormat,
    describe_schema: bool,
) -> Result<(), AnvilError> {
    if describe_schema {
        println!("{PHASE_BUILD_SCHEMA}");
        return Ok(());
    }

    let config = load_config(project_root)?;
    let store = AuditStore::open(project_root)?;

    // F4: Round number from briefing-gate count, not RFP count.
    let artifact_ref_prefix = format!("phase:{phase_id}");
    let round_number = count_phase_briefing_rounds(&store, phase_id)? + 1;

    let binding = find_model_binding(&config, ROLE_CODER)?;
    let conn_name = binding.provider_connection.clone();
    let model_id = binding.model_identity.clone();
    let conn = config
        .provider_connections
        .get(&conn_name)
        .ok_or_else(|| AnvilError::ProviderConnectionMissing(conn_name.clone()))?;
    let api_key = retrieve_api_key(&conn_name, &conn.credential_ref)?;

    let contract_path = project_root.join(".anvil/plan_contract.json");
    let contract_json = std::fs::read_to_string(&contract_path).map_err(|_| {
        AnvilError::Io(std::io::Error::other(
            "plan_contract.json not found — run `anvil plan invoke` first",
        ))
    })?;

    let system_prompt = format!(
        "You are the Coder specialist implementing phase {phase_id} of an Anvil-managed project.\n\
         Produce a Phase Review Briefing for the work you have completed.\n\
         Wrap the briefing contract in <phase_briefing>...</phase_briefing> tags.\n\
         The contract must be valid JSON matching the PhaseBriefingContract schema.\n\
         All 7 required sections must be populated: scope, files_changed, compliance_items,\n\
         what_to_review, test_areas, how_to_activate, next_phase.\n"
    );
    let user_message = format!(
        "Phase {phase_id} plan contract:\n```json\n{contract_json}\n```\n\
         Produce the Phase Review Briefing contract for round {round_number}."
    );

    println!("Invoking Coder '{ROLE_CODER}' for phase {phase_id} R{round_number}…");

    let port = ensure_sidecar_running(project_root, &config)?;
    let mut client = connect_and_handshake(port, &config)?;
    let response = with_tokio(invoke_model(
        &mut client,
        &system_prompt,
        &user_message,
        &model_id,
        &conn_name,
        &api_key,
    ))?;

    let briefing = parse_phase_briefing_contract(&response).map_err(|e| {
        eprintln!("error: Phase Briefing Contract invalid: {e}");
        e
    })?;
    validate_phase_briefing_contract(&briefing).map_err(|e| {
        eprintln!("error: Phase Briefing Contract validation failed: {e}");
        e
    })?;

    // F5: Contract phase_id must match the CLI argument.
    if briefing.phase_id.trim() != phase_id {
        return Err(AnvilError::Io(std::io::Error::other(format!(
            "briefing phase_id '{}' does not match requested phase '{phase_id}' — \
             model produced briefing for wrong phase",
            briefing.phase_id
        ))));
    }

    if format == OutputFormat::Json {
        println!("{}", serde_json::to_string_pretty(&briefing)?);
        return Ok(());
    }

    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let doc = render_phase_briefing_doc(&briefing, &date, round_number);
    let reviews_dir = project_root.join("reviews");
    std::fs::create_dir_all(&reviews_dir)?;
    let briefing_path = reviews_dir.join(format!("BRIEFING_{phase_id}_R{round_number}.md"));

    // F4: Reject if the target file already exists (guard against partial-write re-runs).
    if briefing_path.exists() {
        return Err(AnvilError::Io(std::io::Error::other(format!(
            "briefing '{}' already exists — \
             run `anvil phase review {phase_id}` before building again",
            briefing_path.display()
        ))));
    }

    // Gate record before file write (provenance safety).
    let cross_ref = CrossRefKey::new(
        &artifact_ref_prefix,
        "§briefing",
        &format!("R{round_number}"),
    )
    .to_key_string();
    let gate = GateApproval::new(
        format!("phase-{phase_id}-briefing-sent"),
        ROLE_CODER.to_owned(),
        vec![cross_ref],
    );
    store.append(&gate)?;

    std::fs::write(&briefing_path, doc.as_bytes())?;

    println!("✓ Phase briefing written to '{}'.", briefing_path.display());
    println!("  Gate recorded: phase-{phase_id}-briefing-sent");
    println!("  Round: R{round_number}");
    Ok(())
}

// ── anvil phase review ─────────────────────────────────────────────────────────

/// Runs `anvil phase review <phase_id>`.
///
/// Reads the latest briefing document, invokes the next reviewer in rotation (F1 fix:
/// uses 1-indexed `round_number` matching Charter/Plan convention), stamps the packet's
/// `artifact_hash`, uses the configured binding name as the authoritative `reviewer_id`
/// (F6 fix), and stores four audit records.
///
/// # Errors
///
/// Returns [`AnvilError`] on config, sidecar, or audit-store failure.
#[allow(clippy::too_many_lines)]
pub fn run_phase_review(project_root: &Path, phase_id: &str) -> Result<(), AnvilError> {
    let config = load_config(project_root)?;
    let store = AuditStore::open(project_root)?;

    let artifact_ref_prefix = format!("phase:{phase_id}");

    let all_rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
    let phase_rfps = filter_rfps_by_artifact(&store, &all_rfp_entries, &artifact_ref_prefix);
    let round_number = u32::try_from(phase_rfps.len()).unwrap_or(u32::MAX) + 1;

    let pool: Vec<String> = if config.reviewer_pool.is_empty() {
        vec![ROLE_REVIEWER_1.to_owned()]
    } else {
        config.reviewer_pool.clone()
    };

    // F1: Use round_number (1-indexed) for rotation, matching Charter/Plan convention.
    let reviewer_name = rotation_select(&pool, round_number)
        .ok_or(AnvilError::ReviewerPoolEmpty)?
        .to_owned();
    let prev_reviewer: Option<String> = if round_number > 1 {
        rotation_select(&pool, round_number - 1).map(ToOwned::to_owned)
    } else {
        None
    };

    let binding = find_model_binding(&config, &reviewer_name)?;
    let conn_name = binding.provider_connection.clone();
    let model_id = binding.model_identity.clone();
    let conn = config
        .provider_connections
        .get(&conn_name)
        .ok_or_else(|| AnvilError::ProviderConnectionMissing(conn_name.clone()))?;
    let api_key = retrieve_api_key(&conn_name, &conn.credential_ref)?;

    let briefing_path = project_root
        .join("reviews")
        .join(format!("BRIEFING_{phase_id}_R{round_number}.md"));
    let briefing_bytes = std::fs::read(&briefing_path).map_err(|_| {
        AnvilError::Io(std::io::Error::other(format!(
            "briefing not found at '{}' — run `anvil phase build {phase_id}` first",
            briefing_path.display()
        )))
    })?;
    let briefing_doc = String::from_utf8_lossy(&briefing_bytes).into_owned();
    let briefing_hash = crate::utils::sha256_hex(&briefing_bytes);

    println!("Invoking reviewer '{reviewer_name}' for phase {phase_id} R{round_number}…");

    let port = ensure_sidecar_running(project_root, &config)?;
    let mut client = connect_and_handshake(port, &config)?;

    let user_message = format!(
        "Phase {phase_id} Review Briefing (R{round_number}):\n\n{briefing_doc}\n\n\
         Review the implementation described in this briefing and produce a structured \
         Findings Packet."
    );

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

    // F6: Configured binding name is authoritative for reviewer_id; model claim is not trusted.
    let mut packet = FindingsPacket::new(
        format!("{artifact_ref_prefix}:R{round_number}"),
        round_number,
        reviewer_name.clone(),
        reviewer_model_identity,
        partial.findings,
    );

    apply_severity_tiering(&mut packet, round_number);
    packet.artifact_hash = Some(briefing_hash);

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

    let cross_ref = CrossRefKey::new(&artifact_ref_prefix, "§review", &format!("R{round_number}"))
        .to_key_string();
    let cross_refs = vec![cross_ref];

    let gate = GateApproval::new(
        format!("phase-{phase_id}-findings-received"),
        reviewer_name.clone(),
        cross_refs.clone(),
    );
    store.append(&gate)?;

    let rfp = ReviewerFindingPacket::from_packet(
        format!("{artifact_ref_prefix}:R{round_number}"),
        packet.clone(),
        cross_refs.clone(),
    );
    store.append(&rfp)?;

    let vr = VerifierResult::from_verified(
        format!("{artifact_ref_prefix}:R{round_number}"),
        "local-verifier-v1".to_owned(),
        packet.packet_id.clone(),
        verified_findings,
        cross_refs.clone(),
    );
    store.append(&vr)?;

    let rotation_log = RotationLog::new(
        prev_reviewer,
        reviewer_name.clone(),
        format!("round-robin selection for phase {phase_id} R{round_number}"),
        round_number,
        cross_refs,
    );
    store.append(&rotation_log)?;

    println!("\n✓ Phase review complete for '{phase_id}' R{round_number}.");
    println!("  Reviewer:              {reviewer_name}");
    println!("  Findings:              {finding_count}");
    println!("  ReviewerFindingPacket: {}", rfp.id);
    println!("  VerifierResult:        {}", vr.id);
    println!("  Gate:                  phase-{phase_id}-findings-received");
    Ok(())
}

// ── anvil phase findings ───────────────────────────────────────────────────────

/// Runs `anvil phase findings <phase_id>`.
///
/// Loads the latest phase `ReviewerFindingPacket` and its paired `VerifierResult`, runs
/// interactive finding curation, renders the disposition document, persists a
/// `CuratedFindingsRecord`, and records three gate approvals:
/// `phase-{id}-findings-curated`, `phase-{id}-disposition-rendered`,
/// `phase-{id}-next-reviewer-or-ship`.
///
/// # Errors
///
/// Returns [`AnvilError`] on audit-store, file I/O, or input failure.
#[allow(clippy::too_many_lines)]
pub fn run_phase_findings(project_root: &Path, phase_id: &str) -> Result<(), AnvilError> {
    let store = AuditStore::open(project_root)?;
    let artifact_ref_prefix = format!("phase:{phase_id}");

    let all_rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
    let phase_rfps = filter_rfps_by_artifact(&store, &all_rfp_entries, &artifact_ref_prefix);
    if phase_rfps.is_empty() {
        return Err(AnvilError::NoFindingsPacket(format!("phase:{phase_id}")));
    }
    let rfp = phase_rfps.last().expect("non-empty").clone();

    let all_vr_entries = store.list(RecordType::VerifierResult)?;
    let vr = all_vr_entries
        .iter()
        .rev()
        .find_map(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<VerifierResult>(v).ok())
                .filter(|vr| vr.source_packet_id == rfp.packet.packet_id)
        })
        .ok_or_else(|| {
            AnvilError::Io(std::io::Error::other(format!(
                "no VerifierResult for phase {phase_id} packet '{}' — \
                 re-run `anvil phase review {phase_id}`",
                rfp.packet.packet_id
            )))
        })?;

    let round_number = rfp.packet.round_number;
    let reviewer_id = &rfp.packet.reviewer_id;
    let verified_findings = &vr.verified_findings;

    println!(
        "Phase {phase_id} R{round_number} findings — {} finding(s)\n",
        verified_findings.len()
    );
    println!("Reviewer: {reviewer_id}");
    println!("──────────────────────────────────────────────────────────────────────────\n");

    let CurationResult {
        actions: curation_actions,
        disposition_map,
        dispositions,
        advisory_dispositions,
    } = curate_findings_interactive(verified_findings)?;

    let NarrativeInputs {
        narrative_summary,
        corrections,
        residual_notes,
        reproducibility,
        bottom_line,
    } = collect_narrative_inputs()?;

    let missing_advisory = check_advisory_gate(&dispositions, &rfp.packet.findings);
    if !missing_advisory.is_empty() {
        eprintln!(
            "error: advisory gate check failed — {} advisory finding(s) lack disposition:",
            missing_advisory.len()
        );
        for id in &missing_advisory {
            eprintln!("  {id}");
        }
        return Err(AnvilError::Io(std::io::Error::other(
            "advisory gate check failed — one or more advisory findings lack complete disposition",
        )));
    }

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let artifact_name = format!("phase-{phase_id}");
    let disp_input = DispositionInput {
        artifact_name: &artifact_name,
        round_number,
        reviewer_id,
        date: &today,
        verified_findings,
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
    let disp_path = reviews_dir.join(format!("REVIEW_phase-{phase_id}_R{round_number}.md"));
    std::fs::write(&disp_path, doc.as_bytes())?;

    let cross_ref = CrossRefKey::new(
        &artifact_ref_prefix,
        "§findings",
        &format!("R{round_number}"),
    )
    .to_key_string();
    let cross_refs = vec![cross_ref];

    let curated_record = CuratedFindingsRecord::new(
        rfp.packet.packet_id.clone(),
        "coordinator".to_owned(),
        dispositions,
        cross_refs.clone(),
    );
    store.append(&curated_record)?;

    let gate_curated = GateApproval::new(
        format!("phase-{phase_id}-findings-curated"),
        "coordinator".to_owned(),
        cross_refs.clone(),
    );
    store.append(&gate_curated)?;

    let gate_rendered = GateApproval::new(
        format!("phase-{phase_id}-disposition-rendered"),
        "coordinator".to_owned(),
        cross_refs.clone(),
    );
    store.append(&gate_rendered)?;

    let gate_next = GateApproval::new(
        format!("phase-{phase_id}-next-reviewer-or-ship"),
        "coordinator".to_owned(),
        cross_refs,
    );
    store.append(&gate_next)?;

    println!("\n✓ Phase findings curation complete.");
    println!("  Disposition doc:             {}", disp_path.display());
    println!("  CuratedFindings:             {}", curated_record.id);
    println!("  Gate: findings-curated:      {}", gate_curated.id);
    println!("  Gate: disposition-rendered:  {}", gate_rendered.id);
    println!("  Gate: next-reviewer-or-ship: {}", gate_next.id);
    println!("\nNext step: `anvil phase ship {phase_id}` to complete the phase loop.");
    Ok(())
}

// ── anvil phase ship ───────────────────────────────────────────────────────────

/// Runs `anvil phase ship <phase_id>`.
///
/// Preflight: all five preceding gate records must exist. Stale-briefing check: `build_round`
/// (briefing gate count) must not exceed `review_round` (RFP count). Termination check:
/// full-pool clean against the latest built briefing hash.
///
/// # Errors
///
/// Returns [`AnvilError::PhaseShipBlocked`] if any pre-condition fails.
#[allow(clippy::too_many_lines)]
pub fn run_phase_ship(project_root: &Path, phase_id: &str) -> Result<(), AnvilError> {
    let config = load_config(project_root)?;
    let store = AuditStore::open(project_root)?;

    let artifact_ref_prefix = format!("phase:{phase_id}");

    // F3: Preflight — all five preceding gate records must exist.
    let required_preflight = [
        format!("phase-{phase_id}-briefing-sent"),
        format!("phase-{phase_id}-findings-received"),
        format!("phase-{phase_id}-findings-curated"),
        format!("phase-{phase_id}-disposition-rendered"),
        format!("phase-{phase_id}-next-reviewer-or-ship"),
    ];
    let mut missing_gates: Vec<String> = Vec::new();
    for gate_name in &required_preflight {
        if !phase_gate_exists(&store, gate_name)? {
            missing_gates.push(gate_name.clone());
        }
    }
    if !missing_gates.is_empty() {
        return Err(AnvilError::PhaseShipBlocked {
            phase_id: phase_id.to_owned(),
            reason: format!(
                "required gate records missing: {}",
                missing_gates.join(", ")
            ),
        });
    }

    // F2: Stale-briefing check — build_round must equal review_round.
    let build_round = count_phase_briefing_rounds(&store, phase_id)?;

    let pool: Vec<String> = if config.reviewer_pool.is_empty() {
        vec![ROLE_REVIEWER_1.to_owned()]
    } else {
        config.reviewer_pool.clone()
    };

    let all_rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
    let phase_rfps = filter_rfps_by_artifact(&store, &all_rfp_entries, &artifact_ref_prefix);
    let review_round = u32::try_from(phase_rfps.len()).unwrap_or(u32::MAX);

    if build_round > review_round {
        return Err(AnvilError::PhaseShipBlocked {
            phase_id: phase_id.to_owned(),
            reason: format!(
                "latest briefing R{build_round} has not been reviewed — \
                 run `anvil phase review {phase_id}` first"
            ),
        });
    }

    // Current hash from latest built briefing (build_round, not review_round).
    let current_hash: Option<String> = if build_round > 0 {
        let latest_briefing = project_root
            .join("reviews")
            .join(format!("BRIEFING_{phase_id}_R{build_round}.md"));
        std::fs::read(&latest_briefing)
            .ok()
            .map(|b| crate::utils::sha256_hex(&b))
    } else {
        None
    };

    let phase_packet_ids: HashSet<String> = phase_rfps
        .iter()
        .map(|rfp| rfp.packet.packet_id.clone())
        .collect();
    let arbiter_entries = store.list(RecordType::ArbiterFindingResolution)?;
    let arbiter_decided_ids: HashSet<String> = arbiter_entries
        .iter()
        .filter_map(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<ArbiterFindingResolution>(v).ok())
                .filter(|r| {
                    r.finding_id
                        .split_once(':')
                        .is_some_and(|(pkt, _)| phase_packet_ids.contains(pkt))
                })
                .map(|r| r.finding_id)
        })
        .collect();

    let pool_result = check_full_pool_clean(
        &pool,
        &phase_rfps,
        &arbiter_decided_ids,
        current_hash.as_deref(),
        config.single_clean_pass_override,
    );

    if !pool_result.all_clean {
        let reason = format!(
            "{} reviewer(s) have not submitted a clean pass: {}",
            pool_result.not_clean.len(),
            pool_result.not_clean.join(", ")
        );
        return Err(AnvilError::PhaseShipBlocked {
            phase_id: phase_id.to_owned(),
            reason,
        });
    }

    let cross_ref = CrossRefKey::new(&artifact_ref_prefix, "§ship", &format!("R{review_round}"))
        .to_key_string();

    let gate = GateApproval::new(
        format!("phase-{phase_id}-ship"),
        "coordinator".to_owned(),
        vec![cross_ref.clone()],
    );
    let disposition =
        PhaseDisposition::new(phase_id.to_owned(), "shipped".to_owned(), vec![cross_ref]);
    store.append(&gate)?;
    store.append(&disposition)?;

    println!("✓ Phase '{phase_id}' shipped.");
    if pool_result.override_active {
        println!("  (single-clean-pass override active)");
    }
    println!("  Gate recorded:     phase-{phase_id}-ship");
    println!("  Disposition:       shipped");
    println!("  Gate ID:           {}", gate.id);
    println!("  Disposition ID:    {}", disposition.id);
    Ok(())
}

// ── Private helpers ────────────────────────────────────────────────────────────

struct CurationResult {
    actions: BTreeMap<String, CurationAction>,
    disposition_map: BTreeMap<String, DispositionLabel>,
    dispositions: Vec<CurationDisposition>,
    advisory_dispositions: BTreeMap<String, (AdvisoryDispositionType, Option<String>)>,
}

#[allow(clippy::too_many_lines)]
fn curate_findings_interactive(
    verified_findings: &[VerifiedFinding],
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

        let (action, annotation, advisory_disposition) = if f.advisory {
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
            let action = match adv_type {
                AdvisoryDispositionType::DropAdvisory => CurationAction::Drop,
                _ => CurationAction::Keep,
            };
            (action, annotation, Some(adv_type))
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
            let label_idx = Select::new()
                .with_prompt("  Disposition label")
                .items(&[
                    "Fixed",
                    "Locked in Phase (pending next)",
                    "Refuted",
                    "Deferred",
                ])
                .default(0)
                .interact()
                .map_err(|_| AnvilError::SetupCancelled)?;
            let label = match label_idx {
                1 => DispositionLabel::LockedPendingPlan,
                2 => DispositionLabel::Refuted,
                3 => DispositionLabel::Deferred,
                _ => DispositionLabel::Fixed,
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

fn collect_narrative_inputs() -> Result<NarrativeInputs, AnvilError> {
    println!("\n── Disposition document inputs ──────────────────────────────────────────────\n");
    Ok(NarrativeInputs {
        narrative_summary: Input::new()
            .with_prompt("Narrative summary (what changed in this round)")
            .allow_empty(true)
            .interact_text()
            .map_err(|_| AnvilError::SetupCancelled)?,
        corrections: Input::new()
            .with_prompt("Corrections to prior round narrative (leave blank if none)")
            .allow_empty(true)
            .interact_text()
            .map_err(|_| AnvilError::SetupCancelled)?,
        residual_notes: Input::new()
            .with_prompt("Residual / deferred notes (leave blank if none)")
            .allow_empty(true)
            .interact_text()
            .map_err(|_| AnvilError::SetupCancelled)?,
        reproducibility: Input::new()
            .with_prompt("Reproducibility commands (leave blank if none)")
            .allow_empty(true)
            .interact_text()
            .map_err(|_| AnvilError::SetupCancelled)?,
        bottom_line: Input::new()
            .with_prompt("Bottom line summary")
            .allow_empty(true)
            .interact_text()
            .map_err(|_| AnvilError::SetupCancelled)?,
    })
}

#[derive(serde::Deserialize)]
struct PartialFindingsPacket {
    #[serde(default)]
    reviewer_model_identity: String,
    #[serde(default)]
    findings: Vec<anvil_core::pipeline::Finding>,
}

async fn invoke_model(
    client: &mut anvil_sidecar_client::client::AnvilSidecarClient,
    system_prompt: &str,
    user_message: &str,
    model_id: &str,
    conn_name: &str,
    api_key: &str,
) -> Result<String, AnvilError> {
    let request = proto::InvokeRequest {
        idempotency_key: String::new(),
        model_id: model_id.to_owned(),
        provider_connection_id: conn_name.to_owned(),
        credentials: Some(proto::Credentials {
            credential: Some(proto::credentials::Credential::ApiKey(api_key.to_owned())),
        }),
        timeout: Some(proto::Timeout { millis: 180_000 }),
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

// ── Tests ──────────────────────────────────────────────────────────────────────

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
        AuditStore::open(root).map(|s| (tmp, s)).expect("store")
    }

    fn seed_preflight_gates(store: &AuditStore, phase_id: &str) {
        for name in [
            format!("phase-{phase_id}-briefing-sent"),
            format!("phase-{phase_id}-findings-received"),
            format!("phase-{phase_id}-findings-curated"),
            format!("phase-{phase_id}-disposition-rendered"),
            format!("phase-{phase_id}-next-reviewer-or-ship"),
        ] {
            store
                .append(&GateApproval::new(name, "test".to_owned(), vec![]))
                .expect("seed gate");
        }
    }

    // hinge_test: pins=ship_requires_full_pool_clean, intended=termination, phase=P8
    #[test]
    fn test_phase_cannot_ship_without_termination() {
        // Pins: run_phase_ship exits non-zero when termination conditions are not met.
        // Flipping requires updating both check_full_pool_clean semantics and ship preflight
        // together with this test.
        let (tmp, _store) = init_store();
        std::fs::write(tmp.path().join("anvil.toml"), "[choices]\n").unwrap();

        let result = run_phase_ship(tmp.path(), "P8");
        assert!(result.is_err(), "ship must fail when conditions not met");
        let err = result.unwrap_err();
        assert!(
            matches!(err, AnvilError::PhaseShipBlocked { ref phase_id, .. } if phase_id == "P8"),
            "expected PhaseShipBlocked for P8, got: {err}"
        );
        assert!(
            err.to_string().contains("P8"),
            "error must name phase: {err}"
        );
    }

    #[test]
    fn test_phase_ship_succeeds_with_clean_pass() {
        let (tmp, store) = init_store();
        std::fs::write(tmp.path().join("anvil.toml"), "[choices]\n").unwrap();

        let reviews_dir = tmp.path().join("reviews");
        std::fs::create_dir_all(&reviews_dir).unwrap();
        let briefing_content = b"# Phase P8 R1 Briefing";
        std::fs::write(reviews_dir.join("BRIEFING_P8_R1.md"), briefing_content).unwrap();
        let briefing_hash = crate::utils::sha256_hex(briefing_content);

        // Seed all five preflight gates (including briefing-sent so build_round = 1).
        seed_preflight_gates(&store, "P8");

        let mut packet = FindingsPacket::new(
            "phase:P8:R1".to_owned(),
            1,
            "reviewer-1".to_owned(),
            "model-v1".to_owned(),
            vec![],
        );
        packet.artifact_hash = Some(briefing_hash);
        let rfp = ReviewerFindingPacket::from_packet("phase:P8:R1".to_owned(), packet, vec![]);
        store.append(&rfp).expect("append RFP");

        let result = run_phase_ship(tmp.path(), "P8");
        assert!(
            result.is_ok(),
            "ship must succeed with clean pass: {result:?}"
        );

        let gates = store.list(RecordType::GateApproval).expect("list gates");
        let ship_gate_exists = gates.iter().any(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<GateApproval>(v).ok())
                .is_some_and(|g| g.gate_name == "phase-P8-ship")
        });
        assert!(ship_gate_exists, "phase-P8-ship gate record must exist");

        let dispositions = store.list(RecordType::PhaseDisposition).expect("list");
        assert_eq!(
            dispositions.len(),
            1,
            "one PhaseDisposition must be created"
        );
    }

    #[test]
    fn test_describe_schema_prints_schema() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = run_phase_build(tmp.path(), "P8", OutputFormat::Text, true);
        assert!(
            result.is_ok(),
            "describe-schema must succeed without config: {result:?}"
        );
    }

    // hinge_test: pins=phase_ship_gate_preflight, intended=completeness, phase=P8
    #[test]
    fn test_phase_ship_preflight_blocks_missing_gates() {
        // Pins: run_phase_ship must block with PhaseShipBlocked listing missing gate names
        // when required gate records are absent. Flipping requires updating the preflight
        // check in run_phase_ship and this test together.
        let (tmp, store) = init_store();
        std::fs::write(tmp.path().join("anvil.toml"), "[choices]\n").unwrap();

        // Seed only briefing-sent and findings-received; omit curated/rendered/next.
        for name in ["phase-P8-briefing-sent", "phase-P8-findings-received"] {
            store
                .append(&GateApproval::new(
                    name.to_owned(),
                    "test".to_owned(),
                    vec![],
                ))
                .expect("seed gate");
        }
        let mut packet = FindingsPacket::new(
            "phase:P8:R1".to_owned(),
            1,
            "reviewer-1".to_owned(),
            "model-v1".to_owned(),
            vec![],
        );
        packet.artifact_hash = None;
        let rfp = ReviewerFindingPacket::from_packet("phase:P8:R1".to_owned(), packet, vec![]);
        store.append(&rfp).expect("append RFP");

        let err = run_phase_ship(tmp.path(), "P8").unwrap_err();
        assert!(
            matches!(err, AnvilError::PhaseShipBlocked { .. }),
            "must be PhaseShipBlocked: {err}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("phase-P8-findings-curated"),
            "error must name the missing gate: {msg}"
        );
    }

    // hinge_test: pins=phase_ship_stale_briefing_blocked, intended=state-consistency, phase=P8
    #[test]
    fn test_phase_ship_blocked_by_stale_briefing() {
        // Pins: run_phase_ship must block when build_round > review_round (a newer briefing
        // exists that has not been reviewed). Flipping requires updating the stale-briefing
        // check in run_phase_ship and this test together.
        let (tmp, store) = init_store();
        std::fs::write(tmp.path().join("anvil.toml"), "[choices]\n").unwrap();

        let reviews_dir = tmp.path().join("reviews");
        std::fs::create_dir_all(&reviews_dir).unwrap();

        // Build R1 + review R1.
        let briefing_content = b"# Phase P8 R1 Briefing";
        std::fs::write(reviews_dir.join("BRIEFING_P8_R1.md"), briefing_content).unwrap();
        let briefing_hash = crate::utils::sha256_hex(briefing_content);
        store
            .append(&GateApproval::new(
                "phase-P8-briefing-sent".to_owned(),
                "test".to_owned(),
                vec![],
            ))
            .unwrap();
        let mut packet = FindingsPacket::new(
            "phase:P8:R1".to_owned(),
            1,
            "reviewer-1".to_owned(),
            "model-v1".to_owned(),
            vec![],
        );
        packet.artifact_hash = Some(briefing_hash);
        store
            .append(&ReviewerFindingPacket::from_packet(
                "phase:P8:R1".to_owned(),
                packet,
                vec![],
            ))
            .unwrap();

        // Build R2 (second briefing-sent gate). build_round = 2, review_round = 1.
        store
            .append(&GateApproval::new(
                "phase-P8-briefing-sent".to_owned(),
                "test".to_owned(),
                vec![],
            ))
            .unwrap();

        // Seed the remaining four preflight gates.
        for name in [
            "phase-P8-findings-received",
            "phase-P8-findings-curated",
            "phase-P8-disposition-rendered",
            "phase-P8-next-reviewer-or-ship",
        ] {
            store
                .append(&GateApproval::new(
                    name.to_owned(),
                    "test".to_owned(),
                    vec![],
                ))
                .unwrap();
        }

        let err = run_phase_ship(tmp.path(), "P8").unwrap_err();
        assert!(
            matches!(err, AnvilError::PhaseShipBlocked { .. }),
            "must be PhaseShipBlocked: {err}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("R2"),
            "error must mention unreviewed round: {msg}"
        );
    }

    #[test]
    fn test_count_phase_briefing_rounds() {
        let (tmp, store) = init_store();
        std::fs::write(tmp.path().join("anvil.toml"), "[choices]\n").unwrap();

        assert_eq!(count_phase_briefing_rounds(&store, "P8").unwrap(), 0);

        store
            .append(&GateApproval::new(
                "phase-P8-briefing-sent".to_owned(),
                "t".to_owned(),
                vec![],
            ))
            .unwrap();
        assert_eq!(count_phase_briefing_rounds(&store, "P8").unwrap(), 1);

        store
            .append(&GateApproval::new(
                "phase-P8-briefing-sent".to_owned(),
                "t".to_owned(),
                vec![],
            ))
            .unwrap();
        assert_eq!(count_phase_briefing_rounds(&store, "P8").unwrap(), 2);

        // A different phase must not affect P8's count.
        store
            .append(&GateApproval::new(
                "phase-P9-briefing-sent".to_owned(),
                "t".to_owned(),
                vec![],
            ))
            .unwrap();
        assert_eq!(
            count_phase_briefing_rounds(&store, "P8").unwrap(),
            2,
            "P9 gate must not increment P8 count"
        );
    }

    // hinge_test: pins=phase_rotation_1indexed, intended=reviewer-diversity, phase=P8
    #[test]
    fn test_phase_rotation_uses_round_number_not_round_count() {
        // Pins: run_phase_review must call rotation_select with round_number (1-indexed),
        // not round_count (0-indexed), so consecutive reviews use different pool members.
        // Flipping requires updating the rotation call in run_phase_review and this test.
        use anvil_core::rotation::rotation_select;

        let pool = vec!["reviewer-1".to_owned(), "reviewer-2".to_owned()];

        // Post-fix: round_number 1 and 2 → different indices.
        let r1 = rotation_select(&pool, 1).expect("r1");
        let r2 = rotation_select(&pool, 2).expect("r2");
        assert_eq!(r1, "reviewer-1");
        assert_eq!(r2, "reviewer-2");
        assert_ne!(
            r1, r2,
            "consecutive rounds must select different reviewers (pool=2)"
        );

        // Pre-fix (broken): round_count 0 and 1 → same reviewer.
        let broken_r1 = rotation_select(&pool, 0).expect("broken r1");
        let broken_r2 = rotation_select(&pool, 1).expect("broken r2");
        assert_eq!(
            broken_r1, broken_r2,
            "old code (round_count) selects same reviewer for both rounds"
        );
    }
}
