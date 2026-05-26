//! `anvil status` — project workflow status overview (P6).
//!
//! Shows rotation position, round count, convergence declarations,
//! open advisory findings, and full-pool clean status for the active project.

use std::collections::HashSet;
use std::path::Path;

use anvil_audit::{
    records::{ArbiterFindingResolution, ReviewerFindingPacket},
    AuditStore, RecordType,
};
use anvil_core::{
    config::load_config,
    error::AnvilError,
    pipeline::check_advisory_gate,
    rotation::{rotation_select, FullPoolCheckResult, ADVISORY_THRESHOLD_ROUND},
};

use crate::setup::ROLE_REVIEWER_1;

// ── Public entry point ────────────────────────────────────────────────────────

/// Runs `anvil status` — displays project workflow status.
///
/// # Errors
///
/// Returns [`AnvilError`] on config or audit-store failure.
pub fn run_status(project_root: &Path) -> Result<(), AnvilError> {
    let config = load_config(project_root)?;
    let store = AuditStore::open(project_root)?;

    // Effective reviewer pool.
    let pool: Vec<String> = if config.reviewer_pool.is_empty() {
        vec![ROLE_REVIEWER_1.to_owned()]
    } else {
        config.reviewer_pool.clone()
    };

    // Count review rounds (one RFP per round).
    let rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
    let round_count = u32::try_from(rfp_entries.len()).unwrap_or(u32::MAX);

    // Rotation position.
    let current_reviewer = rotation_select(&pool, round_count).unwrap_or("-");
    let next_reviewer = rotation_select(&pool, round_count + 1).unwrap_or("-");

    // Convergence declarations.
    let conv_entries = store.list(RecordType::ConvergenceDeclaration)?;
    let convergence_count = conv_entries.len();

    // Arbiter-decided findings.
    let arbiter_entries = store.list(RecordType::ArbiterFindingResolution)?;
    let arbiter_decided_ids: HashSet<String> = arbiter_entries
        .iter()
        .filter_map(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<ArbiterFindingResolution>(v).ok())
                .map(|r| r.finding_id)
        })
        .collect();
    let arbiter_decided_count = arbiter_decided_ids.len();

    // Open advisory findings (advisory findings in latest RFP without curated advisory disposition).
    let open_advisory = count_open_advisory(&store, &rfp_entries)?;

    // Full-pool clean check.
    let pool_result = check_full_pool_clean(
        &pool,
        &store,
        &rfp_entries,
        &arbiter_decided_ids,
        config.single_clean_pass_override,
    );

    // ── Display ──────────────────────────────────────────────────────────────

    println!("Anvil Project Status");
    println!("────────────────────────────────────────────────────────────────────");
    println!();
    println!("Reviewer pool ({} member(s)):", pool.len());
    for (i, name) in pool.iter().enumerate() {
        println!("  [{i}] {name}");
    }
    println!();
    println!("Review rounds completed:    {round_count}");
    println!(
        "Advisory threshold:         round {} (rounds {}+ are advisory)",
        ADVISORY_THRESHOLD_ROUND,
        ADVISORY_THRESHOLD_ROUND + 1
    );
    println!("Current reviewer:           {current_reviewer}");
    println!("Next reviewer:              {next_reviewer}");
    println!();
    println!("Convergence declarations:   {convergence_count}");
    println!("Open advisory findings:     {open_advisory}");
    println!("Arbiter-decided findings:   {arbiter_decided_count}");
    println!();
    println!(
        "Single-clean-pass override: {}",
        if config.single_clean_pass_override {
            "ON"
        } else {
            "off"
        }
    );
    println!();

    if pool_result.all_clean {
        if pool_result.override_active {
            println!("Full-pool clean: SATISFIED (single-clean-pass override active)");
        } else {
            println!("Full-pool clean: SATISFIED");
        }
    } else if pool_result.not_clean.is_empty() && round_count == 0 {
        println!("Full-pool clean: PENDING (no reviews submitted yet)");
    } else {
        println!(
            "Full-pool clean: NOT SATISFIED ({} reviewer(s) pending clean pass)",
            pool_result.not_clean.len()
        );
        for reviewer in &pool_result.not_clean {
            println!("  pending: {reviewer}");
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Counts advisory findings in the latest RFP without curated advisory dispositions.
fn count_open_advisory(
    store: &AuditStore,
    rfp_entries: &[anvil_audit::index::IndexEntry],
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

/// Checks whether all pool members have submitted a clean pass.
///
/// A "clean pass" = the most recent `ReviewerFindingPacket` from a reviewer contains
/// no P1 findings and no non-advisory, non-arbiter-decided P2/P3 findings.
fn check_full_pool_clean(
    pool: &[String],
    store: &AuditStore,
    rfp_entries: &[anvil_audit::index::IndexEntry],
    arbiter_decided_ids: &HashSet<String>,
    single_clean_pass_override: bool,
) -> FullPoolCheckResult {
    if rfp_entries.is_empty() {
        return FullPoolCheckResult {
            all_clean: false,
            not_clean: pool.to_vec(),
            override_active: false,
        };
    }

    // Collect the latest RFP per reviewer_id.
    let mut latest_by_reviewer: std::collections::HashMap<String, ReviewerFindingPacket> =
        std::collections::HashMap::new();
    for entry in rfp_entries {
        if let Ok(val) = store.get(&entry.id) {
            if let Ok(rfp) = serde_json::from_value::<ReviewerFindingPacket>(val) {
                let reviewer_id = rfp.packet.reviewer_id.clone();
                // Keep the latest by created_at.
                latest_by_reviewer
                    .entry(reviewer_id)
                    .and_modify(|existing| {
                        if rfp.created_at > existing.created_at {
                            *existing = rfp.clone();
                        }
                    })
                    .or_insert(rfp);
            }
        }
    }

    let mut not_clean: Vec<String> = Vec::new();
    let mut clean_count = 0usize;

    for binding_name in pool {
        let reviewer_rfp = latest_by_reviewer.get(binding_name.as_str());
        match reviewer_rfp {
            None => {
                not_clean.push(binding_name.clone());
            }
            Some(rfp) => {
                let has_blocking = rfp.packet.findings.iter().any(|f| {
                    // Blocking = P1 (always), or non-advisory P2/P3 not resolved by arbiter.
                    let composite_id = format!("{}:{}", rfp.packet.packet_id, f.id);
                    let arbiter_decided = arbiter_decided_ids.contains(&composite_id);
                    if arbiter_decided {
                        return false;
                    }
                    if f.advisory {
                        return false;
                    }
                    // Non-advisory, non-arbiter-decided: blocking.
                    true
                });
                if has_blocking {
                    not_clean.push(binding_name.clone());
                } else {
                    clean_count += 1;
                }
            }
        }
    }

    // Single-clean-pass override: one clean reviewer pass is enough.
    if single_clean_pass_override && clean_count >= 1 {
        return FullPoolCheckResult {
            all_clean: true,
            not_clean,
            override_active: true,
        };
    }

    let all_clean = not_clean.is_empty();
    FullPoolCheckResult {
        all_clean,
        not_clean,
        override_active: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_audit::records::{ReviewerFindingPacket, ALL_RECORD_TYPES};
    use anvil_core::pipeline::{Finding, FindingSeverity, FindingsPacket, LocationAnchor};

    fn init_store() -> (tempfile::TempDir, AuditStore) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().to_path_buf();
        std::fs::create_dir_all(root.join("audit-store")).unwrap();
        for rt in ALL_RECORD_TYPES {
            std::fs::create_dir_all(root.join("audit-store").join(rt.dir_name())).unwrap();
        }
        std::fs::write(root.join("audit-store/_index.json"), b"{\"records\":[]}\n").unwrap();
        let store = AuditStore::open(&root).expect("store");
        (tmp, store)
    }

    fn make_clean_rfp(reviewer_id: &str) -> ReviewerFindingPacket {
        let packet = FindingsPacket::new(
            "charter.md:§root:R1".to_owned(),
            1,
            reviewer_id.to_owned(),
            "model-v1".to_owned(),
            vec![],
        );
        ReviewerFindingPacket::from_packet("charter-R1".to_owned(), packet, vec![])
    }

    fn make_rfp_with_finding(reviewer_id: &str, advisory: bool) -> ReviewerFindingPacket {
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
            advisory,
        };
        let packet = FindingsPacket::new(
            "charter.md:§root:R6".to_owned(),
            6,
            reviewer_id.to_owned(),
            "model-v1".to_owned(),
            vec![finding],
        );
        ReviewerFindingPacket::from_packet("charter-R6".to_owned(), packet, vec![])
    }

    #[test]
    fn test_full_pool_clean_empty_store() {
        let (_tmp, store) = init_store();
        let pool = vec!["reviewer-1".to_owned()];
        let entries = store.list(RecordType::ReviewerFindingPacket).unwrap();
        let result = check_full_pool_clean(&pool, &store, &entries, &HashSet::new(), false);
        assert!(!result.all_clean);
        assert_eq!(result.not_clean, vec!["reviewer-1"]);
    }

    #[test]
    fn test_full_pool_clean_with_clean_pass() {
        let (_tmp, store) = init_store();
        let rfp = make_clean_rfp("reviewer-1");
        store.append(&rfp).unwrap();

        let pool = vec!["reviewer-1".to_owned()];
        let entries = store.list(RecordType::ReviewerFindingPacket).unwrap();
        let result = check_full_pool_clean(&pool, &store, &entries, &HashSet::new(), false);
        assert!(result.all_clean);
        assert!(result.not_clean.is_empty());
    }

    #[test]
    fn test_full_pool_clean_advisory_finding_does_not_block() {
        let (_tmp, store) = init_store();
        // Advisory finding (P2 at round 6): should NOT block clean pass.
        let rfp = make_rfp_with_finding("reviewer-1", true);
        store.append(&rfp).unwrap();

        let pool = vec!["reviewer-1".to_owned()];
        let entries = store.list(RecordType::ReviewerFindingPacket).unwrap();
        let result = check_full_pool_clean(&pool, &store, &entries, &HashSet::new(), false);
        assert!(
            result.all_clean,
            "advisory finding must not block clean pass"
        );
    }

    #[test]
    fn test_full_pool_clean_blocking_finding_blocks() {
        let (_tmp, store) = init_store();
        // Non-advisory finding: blocks clean pass.
        let rfp = make_rfp_with_finding("reviewer-1", false);
        store.append(&rfp).unwrap();

        let pool = vec!["reviewer-1".to_owned()];
        let entries = store.list(RecordType::ReviewerFindingPacket).unwrap();
        let result = check_full_pool_clean(&pool, &store, &entries, &HashSet::new(), false);
        assert!(!result.all_clean, "blocking finding must block clean pass");
        assert_eq!(result.not_clean, vec!["reviewer-1"]);
    }

    #[test]
    fn test_single_clean_pass_override() {
        let (_tmp, store) = init_store();
        // Two-member pool; only reviewer-1 has submitted a clean pass.
        let rfp = make_clean_rfp("reviewer-1");
        store.append(&rfp).unwrap();

        let pool = vec!["reviewer-1".to_owned(), "reviewer-2".to_owned()];
        let entries = store.list(RecordType::ReviewerFindingPacket).unwrap();

        // Without override: not satisfied (reviewer-2 is missing).
        let result_no_override =
            check_full_pool_clean(&pool, &store, &entries, &HashSet::new(), false);
        assert!(!result_no_override.all_clean);

        // With override: satisfied by reviewer-1's clean pass.
        let result_override = check_full_pool_clean(&pool, &store, &entries, &HashSet::new(), true);
        assert!(result_override.all_clean);
        assert!(result_override.override_active);
    }
}
