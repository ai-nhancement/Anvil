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
/// Exits non-zero if `reasoning` is empty.
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

    // Count rounds completed for this artifact.
    let rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
    let round_count = u32::try_from(rfp_entries.len()).unwrap_or(u32::MAX);

    // Count open advisory findings (findings marked advisory without explicit advisory disposition).
    let advisory_finding_count = count_open_advisory_findings(&store, &rfp_entries)?;

    // Count arbiter-decided findings.
    let arbiter_entries = store.list(RecordType::ArbiterFindingResolution)?;
    let arbiter_decided_count = u32::try_from(arbiter_entries.len()).unwrap_or(u32::MAX);

    let cross_ref = CrossRefKey::new(artifact, "§root", &format!("R{round_count}")).to_key_string();
    let record = ConvergenceDeclaration::new(
        artifact.to_owned(),
        round_count,
        reasoning.to_owned(),
        advisory_finding_count,
        arbiter_decided_count,
        vec![cross_ref],
    );
    store.append(&record)?;

    println!("✓ Convergence declared for '{artifact}'.");
    println!("  Round count:           {round_count}");
    println!("  Open advisory findings: {advisory_finding_count}");
    println!("  Arbiter-decided:        {arbiter_decided_count}");
    println!("  Record ID:              {}", record.id);
    Ok(())
}

// ── anvil arbiter resolve-finding ────────────────────────────────────────────

/// Runs `anvil arbiter resolve-finding <finding-id> --reason "<text>"`.
///
/// `finding_id` must be in composite form `"<packet_id>:<finding_id>"`.
/// Creates an `ArbiterFindingResolution` record; that finding is excluded from
/// the full-pool clean blocking set on subsequent termination checks.
///
/// # Errors
///
/// Returns [`AnvilError::EmptyReasoning`] if reasoning is empty.
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

    let config = load_config(project_root)?;
    let arbiter_id = config
        .model_bindings
        .first()
        .map_or_else(|| "coordinator".to_owned(), |b| b.name.clone());

    let store = AuditStore::open(project_root)?;
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

// ── Helper ────────────────────────────────────────────────────────────────────

/// Counts advisory findings in the latest `ReviewerFindingPacket` that lack an
/// explicit advisory disposition in the most recent `CuratedFindingsRecord`.
fn count_open_advisory_findings(
    store: &AuditStore,
    rfp_entries: &[IndexEntry],
) -> Result<u32, AnvilError> {
    if rfp_entries.is_empty() {
        return Ok(0);
    }
    let rfp: ReviewerFindingPacket =
        serde_json::from_value(store.get(&rfp_entries.last().unwrap().id)?).map_err(|e| {
            AnvilError::ModelResponseBadJson {
                reason: format!("ReviewerFindingPacket corrupt: {e}"),
            }
        })?;

    // Load latest curated dispositions if available.
    let curated_entries = store.list(RecordType::CuratedFindings)?;
    let dispositions = if let Some(last) = curated_entries.last() {
        let curated: anvil_audit::records::CuratedFindingsRecord =
            serde_json::from_value(store.get(&last.id)?).map_err(|e| {
                AnvilError::ModelResponseBadJson {
                    reason: format!("CuratedFindingsRecord corrupt: {e}"),
                }
            })?;
        curated.dispositions
    } else {
        vec![]
    };

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
        // Also need anvil.toml for load_config in resolve_finding; for declare-convergence
        // we only need an AuditStore, so we call the inner logic directly.
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
        // We need a minimal anvil.toml so load_config doesn't fail.
        let config_toml = r"
[choices]
";
        std::fs::write(tmp.path().join("anvil.toml"), config_toml).unwrap();
        let result = run_resolve_finding(tmp.path(), "pkt-abc:F1", "", "", "");
        assert!(result.is_err(), "empty reasoning must be rejected");
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
