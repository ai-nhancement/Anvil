//! `anvil arbiter` subcommands (P6):
//! - `anvil arbiter declare-convergence <artifact> --reason "<text>"`
//! - `anvil arbiter resolve-finding <finding-id> --reason "<text>"`

use std::path::Path;

use anvil_audit::{
    index::IndexEntry,
    records::{ArbiterFindingResolution, ConvergenceDeclaration, ReviewerFindingPacket},
    AuditStore, CrossRefKey, RecordType,
};
use anvil_core::{config::load_config, error::AnvilError, pipeline::check_advisory_gate};

// ── anvil arbiter declare-convergence ────────────────────────────────────────

/// Runs `anvil arbiter declare-convergence <artifact> --reason "<text>"`.
///
/// Creates a `ConvergenceDeclaration` audit record for the given artifact.
/// All counts (rounds, advisory findings, arbiter-decided) are scoped to the artifact.
///
/// # Errors
///
/// Returns [`AnvilError::EmptyReasoning`] if reasoning is empty, or audit-store errors.
pub fn run_declare_convergence(
    project_root: &Path,
    artifact: &str,
    reasoning: &str,
) -> Result<(), AnvilError> {
    if reasoning.trim().is_empty() {
        return Err(AnvilError::EmptyReasoning {
            command: "declare-convergence",
        });
    }

    let store = AuditStore::open(project_root)?;

    // Collect RFPs for this artifact only (F6).
    let all_rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
    let artifact_rfps = filter_rfps_by_artifact(&store, &all_rfp_entries, artifact);
    let round_count = u32::try_from(artifact_rfps.len()).unwrap_or(u32::MAX);

    // Count open advisory findings scoped to this artifact.
    let advisory_finding_count = count_open_advisory_findings(&store, &artifact_rfps)?;

    // Count arbiter-decided findings scoped to this artifact (F6).
    let artifact_packet_ids: std::collections::HashSet<String> = artifact_rfps
        .iter()
        .map(|rfp| rfp.packet.packet_id.clone())
        .collect();
    let arbiter_entries = store.list(RecordType::ArbiterFindingResolution)?;
    let arbiter_decided_count = u32::try_from(
        arbiter_entries
            .iter()
            .filter(|e| {
                store
                    .get(&e.id)
                    .ok()
                    .and_then(|v| serde_json::from_value::<ArbiterFindingResolution>(v).ok())
                    .is_some_and(|r| {
                        r.finding_id
                            .split_once(':')
                            .is_some_and(|(pkt, _)| artifact_packet_ids.contains(pkt))
                    })
            })
            .count(),
    )
    .unwrap_or(u32::MAX);

    // Compute the artifact file hash so the plan-invoke gate can detect post-declaration edits.
    let artifact_hash = std::fs::read(project_root.join(artifact))
        .ok()
        .map(|bytes| crate::utils::sha256_hex(&bytes));

    let cross_ref = CrossRefKey::new(artifact, "§root", &format!("R{round_count}")).to_key_string();
    let record = ConvergenceDeclaration::new(
        artifact.to_owned(),
        round_count,
        reasoning.to_owned(),
        advisory_finding_count,
        arbiter_decided_count,
        vec![cross_ref],
        artifact_hash,
    );
    store.append(&record)?;

    println!("✓ Convergence declared for '{artifact}'.");
    println!("  Round count:            {round_count}");
    println!("  Open advisory findings: {advisory_finding_count}");
    println!("  Arbiter-decided:        {arbiter_decided_count}");
    println!("  Record ID:              {}", record.id);
    Ok(())
}

// ── anvil arbiter resolve-finding ────────────────────────────────────────────

/// Runs `anvil arbiter resolve-finding <finding-id> --reason "<text>"`.
///
/// `finding_id` must be in composite form `"<packet_id>:<finding_id>"`.
/// Validates that the referenced packet and finding exist before creating the record.
/// Creates an `ArbiterFindingResolution` record; that finding is excluded from
/// the full-pool clean blocking set on subsequent termination checks.
///
/// # Errors
///
/// Returns [`AnvilError::EmptyReasoning`] if reasoning is empty, or audit-store errors.
pub fn run_resolve_finding(
    project_root: &Path,
    finding_id: &str,
    reason: &str,
    chosen_direction_summary: &str,
    contradiction_context: &str,
) -> Result<(), AnvilError> {
    if reason.trim().is_empty() {
        return Err(AnvilError::EmptyReasoning {
            command: "resolve-finding",
        });
    }

    // Parse composite ID: "<packet_id>:<finding_id>" (F5).
    let colon_pos = finding_id.find(':').ok_or_else(|| {
        AnvilError::Io(std::io::Error::other(
            "finding_id must be in composite form '<packet_id>:<finding_id>'",
        ))
    })?;
    let packet_id_part = &finding_id[..colon_pos];
    let finding_id_part = &finding_id[colon_pos + 1..];
    if packet_id_part.is_empty() || finding_id_part.is_empty() {
        return Err(AnvilError::Io(std::io::Error::other(
            "finding_id must be in composite form '<packet_id>:<finding_id>'",
        )));
    }

    let config = load_config(project_root)?;
    let arbiter_id = config
        .model_bindings
        .first()
        .map_or_else(|| "coordinator".to_owned(), |b| b.name.clone());

    let store = AuditStore::open(project_root)?;

    // Verify the referenced packet and finding exist (F5).
    let rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
    let packet = rfp_entries
        .iter()
        .find_map(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<ReviewerFindingPacket>(v).ok())
                .filter(|rfp| rfp.packet.packet_id == packet_id_part)
        })
        .ok_or_else(|| AnvilError::PacketNotFound(packet_id_part.to_owned()))?;
    if !packet
        .packet
        .findings
        .iter()
        .any(|f| f.id == finding_id_part)
    {
        return Err(AnvilError::FindingNotFound {
            packet_id: packet_id_part.to_owned(),
            finding_id: finding_id_part.to_owned(),
        });
    }

    let record = ArbiterFindingResolution::new(
        finding_id.to_owned(),
        arbiter_id,
        reason.to_owned(),
        chosen_direction_summary.to_owned(),
        contradiction_context.to_owned(),
        vec![],
    );
    store.append(&record)?;

    println!("✓ Finding '{finding_id}' resolved as Arbiter-Decided.");
    println!("  Record ID: {}", record.id);
    println!("  This finding is excluded from the full-pool clean blocking set.");
    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Returns all `ReviewerFindingPacket` records whose `artifact_ref` starts with `artifact`.
pub(crate) fn filter_rfps_by_artifact(
    store: &AuditStore,
    all_entries: &[IndexEntry],
    artifact: &str,
) -> Vec<ReviewerFindingPacket> {
    all_entries
        .iter()
        .filter_map(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<ReviewerFindingPacket>(v).ok())
                .filter(|rfp| rfp.packet.artifact_ref.starts_with(artifact))
        })
        .collect()
}

/// Counts advisory findings in the latest artifact-scoped RFP without curated advisory
/// dispositions.
///
/// Uses the latest `CuratedFindingsRecord` whose `packet_id` matches `rfp.packet.packet_id`
/// so that curation records from other packets/artifacts cannot satisfy this packet's advisory
/// findings.
fn count_open_advisory_findings(
    store: &AuditStore,
    artifact_rfps: &[ReviewerFindingPacket],
) -> Result<u32, AnvilError> {
    let Some(rfp) = artifact_rfps.last() else {
        return Ok(0);
    };

    let curated_entries = store.list(RecordType::CuratedFindings)?;
    let dispositions = curated_entries
        .iter()
        .rev()
        .find_map(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| {
                    serde_json::from_value::<anvil_audit::records::CuratedFindingsRecord>(v).ok()
                })
                .filter(|c| c.packet_id == rfp.packet.packet_id)
                .map(|c| c.dispositions)
        })
        .unwrap_or_default();

    let missing = check_advisory_gate(&dispositions, &rfp.packet.findings);
    Ok(u32::try_from(missing.len()).unwrap_or(u32::MAX))
}

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

    #[test]
    fn test_declare_convergence_rejects_empty_reason() {
        let (tmp, _store) = init_store();
        let result = run_declare_convergence(tmp.path(), "charter.md", "");
        assert!(result.is_err(), "empty reasoning must be rejected");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("non-empty"),
            "error should mention non-empty requirement: {msg}"
        );
    }

    #[test]
    fn test_resolve_finding_rejects_empty_reason() {
        let (tmp, _store) = init_store();
        let config_toml = r"
[choices]
";
        std::fs::write(tmp.path().join("anvil.toml"), config_toml).unwrap();
        let result = run_resolve_finding(tmp.path(), "pkt-abc:F1", "", "", "");
        assert!(result.is_err(), "empty reasoning must be rejected");
    }

    #[test]
    fn test_resolve_finding_rejects_malformed_id() {
        let (tmp, _store) = init_store();
        let config_toml = r"
[choices]
";
        std::fs::write(tmp.path().join("anvil.toml"), config_toml).unwrap();
        // No colon separator.
        let result = run_resolve_finding(tmp.path(), "nocolon", "some reason", "", "");
        assert!(result.is_err(), "malformed ID must be rejected");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("composite form"),
            "error should mention composite form: {msg}"
        );
    }

    #[test]
    fn test_resolve_finding_rejects_unknown_packet() {
        let (tmp, _store) = init_store();
        let config_toml = r"
[choices]
";
        std::fs::write(tmp.path().join("anvil.toml"), config_toml).unwrap();
        // Valid composite form but packet does not exist in the store.
        let result = run_resolve_finding(
            tmp.path(),
            "00000000-0000-0000-0000-000000000000:F1",
            "some reason",
            "",
            "",
        );
        let err = result.expect_err("unknown packet must be rejected");
        assert!(
            matches!(err, AnvilError::PacketNotFound(ref id) if id == "00000000-0000-0000-0000-000000000000"),
            "expected PacketNotFound with correct id, got: {err}"
        );
        assert!(
            err.to_string()
                .contains("00000000-0000-0000-0000-000000000000"),
            "error message must contain the packet_id: {err}"
        );
    }

    #[test]
    fn test_resolve_finding_rejects_unknown_finding() {
        use anvil_audit::records::ReviewerFindingPacket;
        use anvil_core::pipeline::{Finding, FindingSeverity, FindingsPacket, LocationAnchor};

        let (tmp, store) = init_store();
        let config_toml = r"
[choices]
";
        std::fs::write(tmp.path().join("anvil.toml"), config_toml).unwrap();

        // Build a real RFP with finding "F1" and store it.
        let finding = Finding {
            id: "F1".to_owned(),
            severity: FindingSeverity::P2,
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
            advisory: false,
        };
        let packet = FindingsPacket::new(
            "charter.md:R1".to_owned(),
            1,
            "reviewer-1".to_owned(),
            "model-v1".to_owned(),
            vec![finding],
        );
        let packet_id = packet.packet_id.clone();
        let rfp = ReviewerFindingPacket::from_packet("charter-R1".to_owned(), packet, vec![]);
        store.append(&rfp).expect("append RFP");

        // Reference the real packet but a non-existent finding ID.
        let composite = format!("{packet_id}:NONEXISTENT");
        let result = run_resolve_finding(tmp.path(), &composite, "some reason", "", "");
        let err = result.expect_err("unknown finding must be rejected");
        assert!(
            matches!(
                &err,
                AnvilError::FindingNotFound { packet_id: pid, finding_id: fid }
                    if pid == &packet_id && fid == "NONEXISTENT"
            ),
            "expected FindingNotFound with correct ids, got: {err}"
        );
        assert!(
            err.to_string().contains("NONEXISTENT"),
            "error message must contain the finding_id: {err}"
        );
    }

    #[test]
    fn test_convergence_declaration_persists_reasoning() {
        let (_tmp, store) = init_store();
        store
            .append(&ConvergenceDeclaration::new(
                "charter.md".to_owned(),
                3,
                "All findings resolved.".to_owned(),
                0,
                0,
                vec![CrossRefKey::new("charter.md", "§root", "R3").to_key_string()],
                None,
            ))
            .expect("append");

        let entries = store
            .list(RecordType::ConvergenceDeclaration)
            .expect("list");
        assert_eq!(entries.len(), 1);
        let val = store.get(&entries[0].id).expect("get");
        let record: ConvergenceDeclaration = serde_json::from_value(val).expect("deserialize");
        assert_eq!(record.reasoning, "All findings resolved.");
        assert_eq!(record.round_count, 3);
    }
}
