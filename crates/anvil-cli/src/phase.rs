//! `anvil phase` subcommands (P8):
//! - `anvil phase build <id> [--format json] [--describe-schema]`
//! - `anvil phase review <id> [--project <path>]`
//! - `anvil phase ship <id> [--project <path>]`

use std::collections::HashSet;
use std::path::Path;

use anvil_audit::{
    records::{
        ArbiterFindingResolution, GateApproval, PhaseDisposition, ReviewerFindingPacket,
        RotationLog, VerifierResult,
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
        apply_severity_tiering, extract_findings_packet_json, verify_findings, FindingsPacket,
        REVIEWER_SYSTEM_PROMPT,
    },
    rotation::rotation_select,
};
use anvil_sidecar_client::proto::{self, invoke_request::Payload};

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

// ── anvil phase build ──────────────────────────────────────────────────────────

/// Runs `anvil phase build <phase_id>`.
///
/// With `describe_schema = true`: prints the `PhaseBriefingContract` JSON Schema and returns.
/// With `format = OutputFormat::Json`: prints the briefing contract as JSON (no file write).
/// Otherwise: invokes the Coder via sidecar, validates the briefing contract, writes the
/// briefing markdown to `reviews/BRIEFING_{id}_R{N}.md`, and records a
/// `phase-{id}-briefing-sent` gate.
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

    // Round number = existing RFP count for this phase + 1.
    let artifact_ref_prefix = format!("phase:{phase_id}");
    let all_rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
    let phase_rfps = filter_rfps_by_artifact(&store, &all_rfp_entries, &artifact_ref_prefix);
    let round_number = u32::try_from(phase_rfps.len()).unwrap_or(u32::MAX) + 1;

    let binding = find_model_binding(&config, ROLE_CODER)?;
    let conn_name = binding.provider_connection.clone();
    let model_id = binding.model_identity.clone();
    let conn = config
        .provider_connections
        .get(&conn_name)
        .ok_or_else(|| AnvilError::ProviderConnectionMissing(conn_name.clone()))?;
    let api_key = retrieve_api_key(&conn_name, &conn.credential_ref)?;

    // Load plan contract for phase context.
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

    if format == OutputFormat::Json {
        println!("{}", serde_json::to_string_pretty(&briefing)?);
        return Ok(());
    }

    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let doc = render_phase_briefing_doc(&briefing, &date, round_number);
    let reviews_dir = project_root.join("reviews");
    std::fs::create_dir_all(&reviews_dir)?;
    let briefing_path = reviews_dir.join(format!("BRIEFING_{phase_id}_R{round_number}.md"));

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
/// Reads the latest briefing document, invokes the next reviewer in rotation,
/// runs the local Finding Verifier on the returned packet, and stores:
/// - `GateApproval` for `phase-{id}-findings-received`
/// - `ReviewerFindingPacket`
/// - `VerifierResult`
/// - `RotationLog`
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
    let round_count = u32::try_from(phase_rfps.len()).unwrap_or(u32::MAX);
    let round_number = round_count + 1;

    let pool: Vec<String> = if config.reviewer_pool.is_empty() {
        vec![ROLE_REVIEWER_1.to_owned()]
    } else {
        config.reviewer_pool.clone()
    };

    let reviewer_name = rotation_select(&pool, round_count)
        .ok_or(AnvilError::ReviewerPoolEmpty)?
        .to_owned();
    let prev_reviewer: Option<String> = if round_count > 0 {
        rotation_select(&pool, round_count - 1).map(ToOwned::to_owned)
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

    // Load briefing document for this round.
    let briefing_path = project_root
        .join("reviews")
        .join(format!("BRIEFING_{phase_id}_R{round_number}.md"));
    let briefing_doc = std::fs::read_to_string(&briefing_path).map_err(|_| {
        AnvilError::Io(std::io::Error::other(format!(
            "briefing not found at '{}' — run `anvil phase build {phase_id}` first",
            briefing_path.display()
        )))
    })?;

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

    // Parse findings packet.
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
        format!("{artifact_ref_prefix}:R{round_number}"),
        round_number,
        partial.reviewer_id,
        reviewer_model_identity,
        partial.findings,
    );

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

    let cross_ref = CrossRefKey::new(&artifact_ref_prefix, "§review", &format!("R{round_number}"))
        .to_key_string();
    let cross_refs = vec![cross_ref];

    // Gate record before packet (provenance safety).
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
    println!("  Reviewer:           {reviewer_name}");
    println!("  Findings:           {finding_count}");
    println!("  ReviewerFindingPacket: {}", rfp.id);
    println!("  VerifierResult:        {}", vr.id);
    println!("  Gate:                  phase-{phase_id}-findings-received");
    Ok(())
}

// ── anvil phase ship ───────────────────────────────────────────────────────────

/// Runs `anvil phase ship <phase_id>`.
///
/// Checks the termination condition (full-pool clean) for the phase.
/// Exits non-zero with a named list of unmet conditions if not satisfied.
/// On success, records a `PhaseDisposition` and a `phase-{id}-ship` gate.
///
/// # Errors
///
/// Returns [`AnvilError::PhaseShipBlocked`] if the termination condition is not met,
/// or other [`AnvilError`] variants on config or audit-store failure.
pub fn run_phase_ship(project_root: &Path, phase_id: &str) -> Result<(), AnvilError> {
    let config = load_config(project_root)?;
    let store = AuditStore::open(project_root)?;

    let artifact_ref_prefix = format!("phase:{phase_id}");

    let pool: Vec<String> = if config.reviewer_pool.is_empty() {
        vec![ROLE_REVIEWER_1.to_owned()]
    } else {
        config.reviewer_pool.clone()
    };

    let all_rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
    let phase_rfps = filter_rfps_by_artifact(&store, &all_rfp_entries, &artifact_ref_prefix);

    // Arbiter-decided IDs scoped to this phase.
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
        None,
        config.single_clean_pass_override,
    );

    if !pool_result.all_clean {
        let reason = if phase_rfps.is_empty() {
            format!("no reviews submitted for phase '{phase_id}'")
        } else {
            format!(
                "{} reviewer(s) have not submitted a clean pass: {}",
                pool_result.not_clean.len(),
                pool_result.not_clean.join(", ")
            )
        };
        return Err(AnvilError::PhaseShipBlocked {
            phase_id: phase_id.to_owned(),
            reason,
        });
    }

    let round_count = u32::try_from(phase_rfps.len()).unwrap_or(u32::MAX);
    let cross_ref =
        CrossRefKey::new(&artifact_ref_prefix, "§ship", &format!("R{round_count}")).to_key_string();

    // Records before any file ops (provenance safety).
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

#[derive(serde::Deserialize)]
struct PartialFindingsPacket {
    #[serde(default = "default_reviewer_id")]
    reviewer_id: String,
    #[serde(default)]
    reviewer_model_identity: String,
    #[serde(default)]
    findings: Vec<anvil_core::pipeline::Finding>,
}

fn default_reviewer_id() -> String {
    ROLE_REVIEWER_1.to_owned()
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
        let store = AuditStore::open(root).expect("store");
        (tmp, store)
    }

    // hinge_test: pins=ship_requires_full_pool_clean, intended=termination, phase=P8
    #[test]
    fn test_phase_cannot_ship_without_termination() {
        // Pins: run_phase_ship exits non-zero when the full-pool clean condition is not met.
        // Flipping requires updating check_full_pool_clean semantics and this test together.
        let (tmp, _store) = init_store();
        std::fs::write(tmp.path().join("anvil.toml"), "[choices]\n").unwrap();

        // No reviews submitted — termination condition not met.
        let result = run_phase_ship(tmp.path(), "P8");
        assert!(
            result.is_err(),
            "ship must fail when termination condition is not met"
        );
        let err = result.unwrap_err();
        assert!(
            matches!(err, AnvilError::PhaseShipBlocked { ref phase_id, .. } if phase_id == "P8"),
            "expected PhaseShipBlocked for P8, got: {err}"
        );
        assert!(
            err.to_string().contains("P8"),
            "error message must contain phase_id: {err}"
        );
    }

    #[test]
    fn test_phase_ship_succeeds_with_clean_pass() {
        use anvil_core::pipeline::FindingsPacket;

        let (tmp, store) = init_store();
        std::fs::write(tmp.path().join("anvil.toml"), "[choices]\n").unwrap();

        // Submit a clean-pass RFP for phase "P8".
        let packet = FindingsPacket::new(
            "phase:P8:R1".to_owned(),
            1,
            "reviewer-1".to_owned(),
            "model-v1".to_owned(),
            vec![],
        );
        let rfp = ReviewerFindingPacket::from_packet("phase:P8:R1".to_owned(), packet, vec![]);
        store.append(&rfp).expect("append RFP");

        let result = run_phase_ship(tmp.path(), "P8");
        assert!(
            result.is_ok(),
            "ship must succeed with a clean pass: {result:?}"
        );

        // Verify gate and disposition records were created.
        let gates = store.list(RecordType::GateApproval).expect("list gates");
        let gate_exists = gates.iter().any(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<anvil_audit::records::GateApproval>(v).ok())
                .is_some_and(|g| g.gate_name == "phase-P8-ship")
        });
        assert!(gate_exists, "phase-P8-ship gate record must exist");

        let dispositions = store
            .list(RecordType::PhaseDisposition)
            .expect("list dispositions");
        assert_eq!(
            dispositions.len(),
            1,
            "one PhaseDisposition must be created"
        );
    }

    #[test]
    fn test_describe_schema_prints_schema() {
        // run_phase_build with describe_schema skips all model/config work and exits Ok.
        // We can't easily capture stdout in tests, but we verify no error is returned
        // even without a valid project root or config file.
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = run_phase_build(tmp.path(), "P8", OutputFormat::Text, true);
        assert!(
            result.is_ok(),
            "describe-schema must succeed without config: {result:?}"
        );
    }
}
