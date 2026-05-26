//! `anvil status` — project workflow status overview (P6).
//!
//! Shows rotation position, round count, convergence declarations,
//! open advisory findings, and full-pool clean status for the active project,
//! scoped to a specific artifact (default: `charter.md`).

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;

use anvil_audit::{
    records::{ArbiterFindingResolution, ConvergenceDeclaration, ReviewerFindingPacket},
    AuditStore, RecordType,
};
use anvil_core::{
    config::load_config,
    error::AnvilError,
    pipeline::check_advisory_gate,
    rotation::{rotation_select, FullPoolCheckResult, ADVISORY_THRESHOLD_ROUND},
};

use crate::arbiter::filter_rfps_by_artifact;
use crate::setup::ROLE_REVIEWER_1;

// ── Public entry point ────────────────────────────────────────────────────────

/// Runs `anvil status --artifact <artifact>` — displays project workflow status.
#[allow(clippy::too_many_lines)]
///
/// All counts are scoped to `artifact` (default: `"charter.md"`).
///
/// # Errors
///
/// Returns [`AnvilError`] on config or audit-store failure.
pub fn run_status(project_root: &Path, artifact: &str) -> Result<(), AnvilError> {
    let config = load_config(project_root)?;
    let store = AuditStore::open(project_root)?;

    // Effective reviewer pool.
    let pool: Vec<String> = if config.reviewer_pool.is_empty() {
        vec![ROLE_REVIEWER_1.to_owned()]
    } else {
        config.reviewer_pool.clone()
    };

    // Load and filter RFPs to the active artifact (F6).
    let all_rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
    let artifact_rfps = filter_rfps_by_artifact(&store, &all_rfp_entries, artifact);
    let round_count = u32::try_from(artifact_rfps.len()).unwrap_or(u32::MAX);

    // Rotation position.
    let current_reviewer = rotation_select(&pool, round_count).unwrap_or("-");
    let next_reviewer = rotation_select(&pool, round_count + 1).unwrap_or("-");

    // Convergence declarations scoped to artifact (F6).
    let conv_entries = store.list(RecordType::ConvergenceDeclaration)?;
    let convergence_count = conv_entries
        .iter()
        .filter(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<ConvergenceDeclaration>(v).ok())
                .is_some_and(|r| r.phase_id == artifact)
        })
        .count();

    // Artifact packet IDs for scoped arbiter filtering (F6).
    let artifact_packet_ids: HashSet<String> = artifact_rfps
        .iter()
        .map(|rfp| rfp.packet.packet_id.clone())
        .collect();

    // Arbiter-decided findings scoped to artifact (F6).
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
                        .is_some_and(|(pkt, _)| artifact_packet_ids.contains(pkt))
                })
                .map(|r| r.finding_id)
        })
        .collect();
    let arbiter_decided_count = arbiter_decided_ids.len();

    // Open advisory findings from the latest artifact RFP.
    let open_advisory = count_open_advisory(&store, &artifact_rfps)?;

    // Compute current artifact hash for same-state check (F2).
    let current_hash = std::fs::read_to_string(project_root.join(artifact))
        .ok()
        .map(|content| compute_hex_hash(&content));

    // Full-pool clean check.
    let pool_result = check_full_pool_clean(
        &pool,
        &artifact_rfps,
        &arbiter_decided_ids,
        current_hash.as_deref(),
        config.single_clean_pass_override,
    );

    // Reviewers whose latest RFP lacks artifact_hash (pre-R2) cannot be same-state verified.
    let hash_unknown: Vec<&str> = if current_hash.is_some() {
        pool.iter()
            .filter_map(|name| {
                artifact_rfps
                    .iter()
                    .filter(|rfp| &rfp.packet.reviewer_id == name)
                    .max_by_key(|rfp| rfp.created_at)
                    .filter(|rfp| rfp.packet.artifact_hash.is_none())
                    .map(|_| name.as_str())
            })
            .collect()
    } else {
        vec![]
    };

    // ── Display ──────────────────────────────────────────────────────────────

    println!("Anvil Project Status — artifact: {artifact}");
    println!("────────────────────────────────────────────────────────────────────");
    println!();
    println!("Reviewer pool ({} member(s)):", pool.len());
    for (i, name) in pool.iter().enumerate() {
        println!("  [{i}] {name}");
    }
    println!();
    println!("Review rounds completed:    {round_count}");
    println!(
        "Advisory threshold:         round {} (rounds {}+ are advisory for P2; P3 always advisory)",
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

    if !hash_unknown.is_empty() {
        println!();
        println!(
            "Note: {} reviewer packet(s) predate artifact-hash tracking (pre-R2); \
             same-state verification skipped for: {}",
            hash_unknown.len(),
            hash_unknown.join(", ")
        );
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Counts advisory findings in the latest artifact RFP without curated advisory dispositions.
fn count_open_advisory(
    store: &AuditStore,
    artifact_rfps: &[ReviewerFindingPacket],
) -> Result<u32, AnvilError> {
    let Some(rfp) = artifact_rfps.last() else {
        return Ok(0);
    };

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

/// Returns the SHA-256 hex digest of a string.
pub(crate) fn compute_hex_hash(content: &str) -> String {
    use sha2::Digest as _;
    let digest = sha2::Sha256::digest(content.as_bytes());
    let mut hex = String::with_capacity(64);
    for b in &digest {
        write!(hex, "{b:02x}").unwrap();
    }
    hex
}

/// Checks whether all pool members have submitted a clean pass on the current artifact state.
///
/// A "clean pass" = the reviewer's latest RFP contains no blocking findings (P1 always
/// blocks; advisory or arbiter-decided findings do not block) AND, when `current_hash`
/// is provided, the RFP's `artifact_hash` matches (same-state verification, F2).
///
/// Packets without an `artifact_hash` (pre-R2 records) are treated as state-unknown
/// and allowed through, preserving backwards compatibility.
pub(crate) fn check_full_pool_clean(
    pool: &[String],
    artifact_rfps: &[ReviewerFindingPacket],
    arbiter_decided_ids: &HashSet<String>,
    current_hash: Option<&str>,
    single_clean_pass_override: bool,
) -> FullPoolCheckResult {
    if artifact_rfps.is_empty() {
        return FullPoolCheckResult {
            all_clean: false,
            not_clean: pool.to_vec(),
            override_active: false,
        };
    }

    // Collect the latest RFP per reviewer_id.
    let mut latest_by_reviewer: std::collections::HashMap<String, ReviewerFindingPacket> =
        std::collections::HashMap::new();
    for rfp in artifact_rfps {
        let reviewer_id = rfp.packet.reviewer_id.clone();
        latest_by_reviewer
            .entry(reviewer_id)
            .and_modify(|existing| {
                if rfp.created_at > existing.created_at {
                    *existing = rfp.clone();
                }
            })
            .or_insert_with(|| rfp.clone());
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
                // Verify the clean pass is for the current artifact state (F2).
                let is_current_state = match (&rfp.packet.artifact_hash, current_hash) {
                    (Some(rfp_hash), Some(expected)) => rfp_hash == expected,
                    _ => true, // No hash on either side: unknown state, allow
                };

                let has_blocking = rfp.packet.findings.iter().any(|f| {
                    let composite_id = format!("{}:{}", rfp.packet.packet_id, f.id);
                    let arbiter_decided = arbiter_decided_ids.contains(&composite_id);
                    if arbiter_decided {
                        return false;
                    }
                    if f.advisory {
                        return false;
                    }
                    true
                });

                if has_blocking || !is_current_state {
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
            "charter.md:R1".to_owned(),
            1,
            reviewer_id.to_owned(),
            "model-v1".to_owned(),
            vec![],
        );
        ReviewerFindingPacket::from_packet("charter-R1".to_owned(), packet, vec![])
    }

    fn make_clean_rfp_with_hash(reviewer_id: &str, hash: &str) -> ReviewerFindingPacket {
        let mut packet = FindingsPacket::new(
            "charter.md:R1".to_owned(),
            1,
            reviewer_id.to_owned(),
            "model-v1".to_owned(),
            vec![],
        );
        packet.artifact_hash = Some(hash.to_owned());
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
            "charter.md:R6".to_owned(),
            6,
            reviewer_id.to_owned(),
            "model-v1".to_owned(),
            vec![finding],
        );
        ReviewerFindingPacket::from_packet("charter-R6".to_owned(), packet, vec![])
    }

    #[test]
    fn test_full_pool_clean_empty_store() {
        let (_tmp, _store) = init_store();
        let pool = vec!["reviewer-1".to_owned()];
        let result = check_full_pool_clean(&pool, &[], &HashSet::new(), None, false);
        assert!(!result.all_clean);
        assert_eq!(result.not_clean, vec!["reviewer-1"]);
    }

    #[test]
    fn test_full_pool_clean_with_clean_pass() {
        let (_tmp, _store) = init_store();
        let rfp = make_clean_rfp("reviewer-1");

        let pool = vec!["reviewer-1".to_owned()];
        let result = check_full_pool_clean(&pool, &[rfp], &HashSet::new(), None, false);
        assert!(result.all_clean);
        assert!(result.not_clean.is_empty());
    }

    #[test]
    fn test_full_pool_clean_advisory_finding_does_not_block() {
        let (_tmp, _store) = init_store();
        let rfp = make_rfp_with_finding("reviewer-1", true);

        let pool = vec!["reviewer-1".to_owned()];
        let result = check_full_pool_clean(&pool, &[rfp], &HashSet::new(), None, false);
        assert!(
            result.all_clean,
            "advisory finding must not block clean pass"
        );
    }

    #[test]
    fn test_full_pool_clean_blocking_finding_blocks() {
        let (_tmp, _store) = init_store();
        let rfp = make_rfp_with_finding("reviewer-1", false);

        let pool = vec!["reviewer-1".to_owned()];
        let result = check_full_pool_clean(&pool, &[rfp], &HashSet::new(), None, false);
        assert!(!result.all_clean, "blocking finding must block clean pass");
        assert_eq!(result.not_clean, vec!["reviewer-1"]);
    }

    #[test]
    fn test_single_clean_pass_override() {
        let (_tmp, _store) = init_store();
        let rfp1 = make_clean_rfp("reviewer-1");
        let pool = vec!["reviewer-1".to_owned(), "reviewer-2".to_owned()];

        // Without override: reviewer-2 is missing.
        let result_no_override = check_full_pool_clean(
            &pool,
            std::slice::from_ref(&rfp1),
            &HashSet::new(),
            None,
            false,
        );
        assert!(!result_no_override.all_clean);

        // With override: reviewer-1's clean pass satisfies.
        let result_override = check_full_pool_clean(&pool, &[rfp1], &HashSet::new(), None, true);
        assert!(result_override.all_clean);
        assert!(result_override.override_active);
    }

    // hinge_test: pins=full_pool_clean_requires_same_artifact_state, intended=convergence, phase=P6
    #[test]
    fn test_full_pool_clean_stale_hash_not_satisfied() {
        // Pins: if the current artifact hash differs from a reviewer's clean pass hash,
        // that reviewer is not clean on the current state.
        // Flipping requires updating check_full_pool_clean and this test together.
        let (_tmp, _store) = init_store();
        let current = "deadbeef";
        let stale = "cafebabe";
        let rfp = make_clean_rfp_with_hash("reviewer-1", stale);

        let pool = vec!["reviewer-1".to_owned()];
        let result = check_full_pool_clean(&pool, &[rfp], &HashSet::new(), Some(current), false);
        assert!(
            !result.all_clean,
            "stale artifact hash must not satisfy full-pool clean"
        );
        assert_eq!(result.not_clean, vec!["reviewer-1"]);
    }

    #[test]
    fn test_full_pool_clean_matching_hash_satisfied() {
        let (_tmp, _store) = init_store();
        let hash = "aabbcc";
        let rfp = make_clean_rfp_with_hash("reviewer-1", hash);

        let pool = vec!["reviewer-1".to_owned()];
        let result = check_full_pool_clean(&pool, &[rfp], &HashSet::new(), Some(hash), false);
        assert!(result.all_clean, "matching artifact hash must be clean");
    }

    #[test]
    fn test_full_pool_clean_no_hash_on_packet_passes() {
        // Pre-R2 records without artifact_hash are treated as unknown state (pass through).
        let (_tmp, _store) = init_store();
        let rfp = make_clean_rfp("reviewer-1"); // no artifact_hash set

        let pool = vec!["reviewer-1".to_owned()];
        let result = check_full_pool_clean(&pool, &[rfp], &HashSet::new(), Some("somehash"), false);
        assert!(
            result.all_clean,
            "packet without hash must pass (backwards compat)"
        );
    }
}
