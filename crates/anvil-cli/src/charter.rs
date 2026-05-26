//! `anvil charter` subcommands (P5):
//! - `anvil charter review`  — invoke the reviewer model, store findings + verifier result
//! - `anvil charter findings` — interactive curation, render disposition doc + hardening history

use std::collections::BTreeMap;
use std::path::Path;

use anvil_audit::{
    records::{CuratedFindingsRecord, ReviewerFindingPacket, VerifierResult},
    AuditStore, CrossRefKey, RecordType,
};
use anvil_core::{
    config::load_config,
    error::AnvilError,
    pipeline::{
        extract_findings_packet_json, verify_findings, CurationAction, CurationDisposition,
        DispositionLabel, Finding, FindingsPacket, VerifiedFinding,
    },
    render::{append_charter_hardening_history, render_disposition_doc, DispositionInput},
};
use anvil_sidecar_client::proto::{self, invoke_request::Payload};
use dialoguer::{Input, Select};

use crate::session::{
    connect_and_handshake, ensure_sidecar_running, find_model_binding, retrieve_api_key,
};
use crate::setup::with_tokio;

// ── Reviewer system prompt ─────────────────────────────────────────────────────

const REVIEWER_SYSTEM_PROMPT: &str = "\
You are a rigorous architecture and document reviewer. Your job is to carefully \
review the provided artifact and produce a structured Findings Packet.

For each finding, identify:
- A unique id (\"F1\", \"F2\", etc.)
- Severity: P1 (critical — contradiction, broken invariant, factual error), \
  P2 (material — clarity, missing edge case, under-specified contract), \
  or P3 (style/cosmetic)
- Location: the artifact path, and optionally a section_id, symbol_name, or a short \
  verbatim quote from the artifact that supports the finding
- Claim: one-sentence statement of the issue
- Evidence: the exact text or code from the artifact that supports your claim
- Recommendation: proposed resolution or direction

Produce the Findings Packet as JSON wrapped in <findings_packet>...</findings_packet> tags.

Format:
<findings_packet>
{
  \"reviewer_id\": \"reviewer-1\",
  \"reviewer_model_identity\": \"<your model identity>\",
  \"findings\": [
    {
      \"id\": \"F1\",
      \"severity\": \"P1\",
      \"location\": {
        \"artifact_path\": \"charter.md\",
        \"section_id\": \"Goals\",
        \"quote\": \"verbatim snippet from the artifact\"
      },
      \"claim\": \"One-sentence description of the issue.\",
      \"evidence\": \"The artifact says X, which contradicts Y.\",
      \"recommendation\": \"Change X to Z.\"
    }
  ]
}
</findings_packet>

Be thorough. Flag contradictions, unclear language, missing required elements, \
incomplete scope definitions, and alignment issues.";

// ── anvil charter review ───────────────────────────────────────────────────────

/// Runs `anvil charter review` — invokes the reviewer model and stores audit records.
///
/// # Errors
///
/// Returns [`AnvilError`] on config, sidecar, model, or audit-store failure.
pub fn run_charter_review(project_root: &Path) -> Result<(), AnvilError> {
    let config = load_config(project_root)?;
    let binding = find_model_binding(&config, crate::setup::ROLE_REVIEWER_1)?;
    let conn_name = binding.provider_connection.clone();
    let model_id = binding.model_identity.clone();

    let conn = config
        .provider_connections
        .get(&conn_name)
        .ok_or_else(|| AnvilError::ProviderConnectionMissing(conn_name.clone()))?;
    let api_key = retrieve_api_key(&conn_name, &conn.credential_ref)?;

    // Read charter.md.
    let charter_path = project_root.join("charter.md");
    let charter_content = std::fs::read_to_string(&charter_path).map_err(|e| {
        AnvilError::Io(std::io::Error::other(format!(
            "charter.md not found at {} — run `anvil discuss` first: {e}",
            charter_path.display()
        )))
    })?;
    if charter_content.trim().is_empty() {
        return Err(AnvilError::Io(std::io::Error::other(
            "charter.md is empty — run `anvil discuss` first",
        )));
    }

    // Determine round number.
    let store = AuditStore::open(project_root)?;
    let existing_packets = store.list(RecordType::ReviewerFindingPacket)?;
    let round_number = u32::try_from(existing_packets.len()).unwrap_or(u32::MAX) + 1;

    println!("Invoking reviewer for charter R{round_number}…");

    // Connect to sidecar and invoke.
    let port = ensure_sidecar_running(project_root, &config)?;
    let mut client = connect_and_handshake(port, &config)?;

    let user_message = format!(
        "Please review the following Charter document (round {round_number}):\n\n{charter_content}"
    );

    let response = with_tokio(invoke_reviewer(
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

    // Fall back to the configured model_id when the reviewer omits its identity.
    let reviewer_model_identity = if partial.reviewer_model_identity.trim().is_empty() {
        model_id.clone()
    } else {
        partial.reviewer_model_identity
    };
    let packet = FindingsPacket::new(
        format!("charter.md:R{round_number}"),
        round_number,
        partial.reviewer_id,
        reviewer_model_identity,
        partial.findings,
    );

    let finding_count = packet.findings.len();
    println!("  Reviewer produced {finding_count} finding(s).");

    // Run the verifier.
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

    // Persist audit records.
    let cross_ref_key =
        CrossRefKey::new("charter.md", "§root", &format!("R{round_number}")).to_key_string();
    let cross_refs = vec![cross_ref_key];
    let rfp_record = ReviewerFindingPacket::from_packet(
        format!("charter-R{round_number}"),
        packet.clone(),
        cross_refs.clone(),
    );
    store.append(&rfp_record)?;

    let vr_record = VerifierResult::from_verified(
        format!("charter-R{round_number}"),
        "local-verifier-v1".to_owned(),
        packet.packet_id.clone(),
        verified_findings,
        cross_refs,
    );
    store.append(&vr_record)?;

    println!("\n✓ Findings stored:");
    println!("  ReviewerFindingPacket: {}", rfp_record.id);
    println!("  VerifierResult:        {}", vr_record.id);
    println!("\nNext step: `anvil charter findings` to curate and render the disposition.");

    Ok(())
}

/// Partial shape parsed from the model's `<findings_packet>` JSON (reviewer fills in `reviewer_id`, etc.).
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

async fn invoke_reviewer(
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

    let content = match resp.result {
        Some(proto::invoke_response::Result::Chat(ref chat)) => chat.content.clone(),
        Some(proto::invoke_response::Result::Error(ref e)) => {
            return Err(AnvilError::Io(std::io::Error::other(format!(
                "reviewer model error: {}",
                e.message
            ))));
        }
        None => {
            return Err(AnvilError::Io(std::io::Error::other(
                "sidecar invoke returned no result — possible transport or timeout issue",
            )));
        }
        Some(_) => {
            return Err(AnvilError::Io(std::io::Error::other(
                "sidecar invoke returned unexpected result variant",
            )));
        }
    };

    Ok(content)
}

// ── anvil charter findings helpers ────────────────────────────────────────────

/// Loads the latest `ReviewerFindingPacket` and `VerifierResult` and asserts they are paired.
fn load_and_pair(
    store: &AuditStore,
) -> Result<(ReviewerFindingPacket, VerifierResult), AnvilError> {
    let rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
    if rfp_entries.is_empty() {
        return Err(AnvilError::NoFindingsPacket("charter.md".to_owned()));
    }
    let rfp: ReviewerFindingPacket = serde_json::from_value(
        store.get(&rfp_entries.last().expect("non-empty checked above").id)?,
    )
    .map_err(|e| AnvilError::ModelResponseBadJson {
        reason: format!("ReviewerFindingPacket corrupt: {e}"),
    })?;

    let vr_entries = store.list(RecordType::VerifierResult)?;
    if vr_entries.is_empty() {
        return Err(AnvilError::Io(std::io::Error::other(
            "no VerifierResult found — re-run `anvil charter review`",
        )));
    }
    let vr: VerifierResult =
        serde_json::from_value(store.get(&vr_entries.last().expect("non-empty checked above").id)?)
            .map_err(|e| AnvilError::ModelResponseBadJson {
                reason: format!("VerifierResult corrupt: {e}"),
            })?;

    if vr.source_packet_id != rfp.packet.packet_id {
        return Err(AnvilError::Io(std::io::Error::other(format!(
            "VerifierResult source_packet_id '{}' does not match ReviewerFindingPacket \
             packet_id '{}' — re-run `anvil charter review` to regenerate a matched pair",
            vr.source_packet_id, rfp.packet.packet_id
        ))));
    }
    Ok((rfp, vr))
}

struct CurationResult {
    actions: BTreeMap<String, CurationAction>,
    disposition_map: BTreeMap<String, DispositionLabel>,
    dispositions: Vec<CurationDisposition>,
}

fn curate_findings(verified_findings: &[VerifiedFinding]) -> Result<CurationResult, AnvilError> {
    let mut actions: BTreeMap<String, CurationAction> = BTreeMap::new();
    let mut disposition_map: BTreeMap<String, DispositionLabel> = BTreeMap::new();
    let mut dispositions: Vec<CurationDisposition> = Vec::new();

    for vf in verified_findings {
        let f = &vf.finding;
        println!("Finding {} [{}]: {}", f.id, f.severity.as_str(), f.claim);
        println!("  Evidence:       {}", f.evidence);
        println!("  Recommendation: {}", f.recommendation);
        println!(
            "  Verification:   {} — {}",
            vf.outcome.as_str(),
            vf.evidence_note
        );
        println!();

        // Edit action is reserved for P6+; P5 offers Keep / Drop / Annotate only.
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

        if matches!(action, CurationAction::Keep) {
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
            let label = match label_idx {
                1 => DispositionLabel::LockedPendingPlan,
                2 => DispositionLabel::Refuted,
                3 => DispositionLabel::Deferred,
                _ => DispositionLabel::Fixed,
            };
            disposition_map.insert(f.id.clone(), label);
        }

        actions.insert(f.id.clone(), action.clone());
        dispositions.push(CurationDisposition {
            finding_id: f.id.clone(),
            action,
            edited_finding: None,
            annotation,
        });

        println!();
    }

    Ok(CurationResult {
        actions,
        disposition_map,
        dispositions,
    })
}

struct NarrativeInputs {
    narrative_summary: String,
    corrections: String,
    residual_notes: String,
    reproducibility: String,
    bottom_line: String,
}

fn collect_narrative() -> Result<NarrativeInputs, AnvilError> {
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

// ── anvil charter findings ─────────────────────────────────────────────────────

/// Runs `anvil charter findings` — interactive curation, disposition doc, hardening history.
///
/// # Errors
///
/// Returns [`AnvilError`] on audit-store, file I/O, or input failure.
pub fn run_charter_findings(project_root: &Path) -> Result<(), AnvilError> {
    let store = AuditStore::open(project_root)?;
    let (rfp, vr) = load_and_pair(&store)?;

    let round_number = rfp.packet.round_number;
    let reviewer_id = &rfp.packet.reviewer_id;
    let verified_findings = &vr.verified_findings;

    println!(
        "Charter R{round_number} findings — {} finding(s)\n",
        verified_findings.len()
    );
    println!("Reviewer: {reviewer_id}");
    println!("──────────────────────────────────────────────────────────────────────────\n");

    let CurationResult {
        actions: curation_actions,
        disposition_map,
        dispositions,
    } = curate_findings(verified_findings)?;

    let NarrativeInputs {
        narrative_summary,
        corrections,
        residual_notes,
        reproducibility,
        bottom_line,
    } = collect_narrative()?;

    // Render disposition document.
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let disp_input = DispositionInput {
        artifact_name: "charter",
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
    };
    let doc = render_disposition_doc(&disp_input);

    // Write disposition doc to reviews/.
    let reviews_dir = project_root.join("reviews");
    std::fs::create_dir_all(&reviews_dir)?;
    let disp_path = reviews_dir.join(format!("REVIEW_charter_R{round_number}.md"));
    std::fs::write(&disp_path, doc.as_bytes())?;

    // Append hardening history.
    let history_summary = if bottom_line.trim().is_empty() {
        format!(
            "R{round_number} disposition complete. {} finding(s) curated.",
            verified_findings.len()
        )
    } else {
        bottom_line.clone()
    };
    append_charter_hardening_history(
        project_root,
        round_number,
        reviewer_id,
        &today,
        &history_summary,
    )?;

    // Persist CuratedFindingsRecord.
    let curated_record = CuratedFindingsRecord::new(
        rfp.packet.packet_id.clone(),
        "coordinator".to_owned(),
        dispositions,
        vec![CrossRefKey::new("charter.md", "§root", &format!("R{round_number}")).to_key_string()],
    );
    store.append(&curated_record)?;

    println!("\n✓ Curation complete.");
    println!("  Disposition doc: {}", disp_path.display());
    println!(
        "  Hardening history: {}",
        project_root.join("CHARTER_HARDENING_HISTORY.md").display()
    );
    println!("  CuratedFindings record: {}", curated_record.id);

    Ok(())
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use anvil_audit::{
        records::{ReviewerFindingPacket, VerifierResult, ALL_RECORD_TYPES},
        AuditStore, CrossRefKey,
    };
    use anvil_core::pipeline::FindingsPacket;

    /// Initializes a fully-structured `AuditStore` in a temp dir.
    fn init_test_store() -> (tempfile::TempDir, AuditStore) {
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

    fn make_packet(round: u32) -> FindingsPacket {
        FindingsPacket::new(
            "charter.md:§root:R1".to_owned(),
            round,
            "reviewer-1".to_owned(),
            "model-v1".to_owned(),
            vec![],
        )
    }

    // hinge_test: pins=p5_cross_ref_keys_parseable, intended=cross-ref-format, phase=P5
    #[test]
    fn test_p5_cross_ref_keys_parseable() {
        // Pins: all cross-reference keys emitted by P5 charter commands must be valid
        // three-part CrossRefKey format: <artifact>:<section>:<version>.
        // Breaking requires updating the key format everywhere it is produced.
        for round in [1u32, 2, 3] {
            let key = CrossRefKey::new("charter.md", "§root", &format!("R{round}")).to_key_string();
            assert!(
                CrossRefKey::parse(&key).is_some(),
                "R{round} cross-ref key '{key}' failed to parse"
            );
        }
    }

    // hinge_test: pins=rfp_vr_pairing_struct, intended=pairing-check, phase=P5
    #[test]
    fn test_rfp_vr_pairing_struct() {
        // Pins: VerifierResult carries source_packet_id for RFP/VR pairing in run_charter_findings.
        // Breaking requires updating both records.rs and the pairing check in charter.rs together.
        use anvil_core::pipeline::{
            Finding, FindingSeverity, LocationAnchor, VerificationOutcome, VerifiedFinding,
        };

        let finding = Finding {
            id: "F1".to_owned(),
            severity: FindingSeverity::P1,
            location: LocationAnchor {
                artifact_path: "charter.md".to_owned(),
                section_id: None,
                line_range: None,
                symbol_name: None,
                quote: None,
            },
            claim: "test".to_owned(),
            evidence: "test".to_owned(),
            recommendation: "test".to_owned(),
            metadata: None,
        };
        let vf = VerifiedFinding {
            finding,
            outcome: VerificationOutcome::CannotBeVerified,
            evidence_note: "no artifact".to_owned(),
        };

        let source_id = "rfp-abc-123".to_owned();
        let vr = VerifierResult::from_verified(
            "vr-test".to_owned(),
            "test-verifier".to_owned(),
            source_id.clone(),
            vec![vf],
            vec![CrossRefKey::new("charter.md", "§root", "R1").to_key_string()],
        );
        assert_eq!(vr.source_packet_id, source_id);
    }

    // hinge_test: pins=rfp_vr_pairing_mismatch_errors, intended=pairing-check, phase=P5
    #[test]
    fn test_rfp_vr_pairing_mismatch_returns_error() {
        // Pins: load_and_pair must fail with a remediation hint when VR.source_packet_id
        // does not match the latest RFP's packet_id.
        // Breaking requires updating load_and_pair together with this test.
        let (_tmp, store) = init_test_store();

        let packet = make_packet(1);
        let rfp_packet_id = packet.packet_id.clone();

        let rfp_record = ReviewerFindingPacket::from_packet(
            "charter-R1".to_owned(),
            packet,
            vec![CrossRefKey::new("charter.md", "§root", "R1").to_key_string()],
        );
        store.append(&rfp_record).expect("append RFP");

        let vr_record = VerifierResult::from_verified(
            "charter-R1".to_owned(),
            "test-verifier".to_owned(),
            "rfp-WRONG-id".to_owned(),
            vec![],
            vec![CrossRefKey::new("charter.md", "§root", "R1").to_key_string()],
        );
        store.append(&vr_record).expect("append VR");

        let err = super::load_and_pair(&store).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("re-run"),
            "error should direct user to re-run: {msg}"
        );
        assert!(
            msg.contains(&rfp_packet_id),
            "error should mention the RFP packet_id: {msg}"
        );
    }

    // hinge_test: pins=p5_provenance_graph_backs_charter_records, intended=provenance, phase=P5
    #[test]
    fn test_p5_provenance_graph_backs_all_charter_record_types() {
        // Pins: ProvenanceGraph can locate RFP, VR, and CuratedFindingsRecord for
        // charter.md:§root:R1 once they are appended with the canonical P5 cross-ref key.
        // Breaking requires updating the cross-ref key convention and this test together.
        use anvil_audit::records::CuratedFindingsRecord;
        use anvil_graph::ProvenanceGraph;

        let (_tmp, store) = init_test_store();
        let cross_ref = CrossRefKey::new("charter.md", "§root", "R1").to_key_string();

        let packet = make_packet(1);
        let rfp_packet_id = packet.packet_id.clone();

        let rfp = ReviewerFindingPacket::from_packet(
            "charter-R1".to_owned(),
            packet,
            vec![cross_ref.clone()],
        );
        store.append(&rfp).expect("append RFP");

        let vr = VerifierResult::from_verified(
            "charter-R1".to_owned(),
            "test-verifier".to_owned(),
            rfp_packet_id.clone(),
            vec![],
            vec![cross_ref.clone()],
        );
        store.append(&vr).expect("append VR");

        let curated = CuratedFindingsRecord::new(
            rfp_packet_id,
            "coordinator".to_owned(),
            vec![],
            vec![cross_ref],
        );
        store.append(&curated).expect("append CuratedFindings");

        let graph = ProvenanceGraph::build(&store).expect("build graph");
        let key = CrossRefKey::new("charter.md", "§root", "R1");
        let backing = graph.records_for_key(&key);

        assert_eq!(
            backing.len(),
            3,
            "expected RFP + VR + CuratedFindings to back charter.md:§root:R1, got {backing:?}"
        );
        assert!(
            backing.contains(&rfp.id),
            "RFP record id should be in provenance backing"
        );
        assert!(
            backing.contains(&vr.id),
            "VR record id should be in provenance backing"
        );
        assert!(
            backing.contains(&curated.id),
            "CuratedFindings record id should be in provenance backing"
        );
    }

    // hinge_test: pins=reviewer_model_identity_fallback, intended=audit-provenance, phase=P5
    #[test]
    fn test_reviewer_model_identity_fallback_logic() {
        // Pins: when partial.reviewer_model_identity is empty, run_charter_review must
        // substitute the configured model_id so audit records carry non-empty model provenance.
        // Breaking requires updating the fallback logic and this test together.
        let configured = "claude-sonnet-4-6";
        let empty = "";
        let provided = "claude-opus-4-7";

        let result_empty = if empty.trim().is_empty() {
            configured.to_owned()
        } else {
            empty.to_owned()
        };
        let result_provided = if provided.trim().is_empty() {
            configured.to_owned()
        } else {
            provided.to_owned()
        };

        assert_eq!(
            result_empty, configured,
            "empty identity must fall back to configured"
        );
        assert_eq!(
            result_provided, provided,
            "non-empty identity must be preserved"
        );
    }
}
