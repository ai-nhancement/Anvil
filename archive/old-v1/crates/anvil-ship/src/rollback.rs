//! Rollback machinery for `anvil phase reopen` (P9).
//!
//! Re-opening a phase triggers cascading invalidation: every transitive dependent
//! is also marked invalid via a `RollbackEvent` audit record. Reviewer rotation
//! resets to position 0 for each invalidated phase so the full pool's diversity
//! reviews the fix.

use chrono::{DateTime, Utc};

use anvil_audit::{
    records::{ReviewerFindingPacket, RollbackEvent},
    AuditStore, RecordType,
};
use anvil_core::{error::AnvilError, plan::PlannerContract};
use anvil_graph::phase_graph::PhaseDepGraph;

/// Computed rollback plan for a single `anvil phase reopen` invocation.
#[derive(Debug, Clone)]
pub struct RollbackPlan {
    /// The phase the user explicitly re-opened.
    pub target_phase: String,
    /// Transitive dependents that are also invalidated (excludes `target_phase` itself).
    pub all_invalidated: Vec<String>,
    /// Complete set of phases whose rotation is reset: `[target_phase] ∪ all_invalidated`.
    ///
    /// This is the `rotation_reset_phases` value written to every `RollbackEvent` produced
    /// by [`execute_rollback`]. Identical across all sibling records from one reopen command.
    pub all_reset_phases: Vec<String>,
}

/// Computes the rollback plan for re-opening `phase_id`.
///
/// Uses the provided dependency graph to compute the full transitive closure of
/// dependent phases. Returns [`AnvilError::UnknownPhase`] if `phase_id` is not
/// declared in `contract`.
///
/// # Errors
///
/// Returns [`AnvilError::UnknownPhase`] if the phase is absent from the contract.
pub fn compute_rollback_plan(
    phase_id: &str,
    graph: &PhaseDepGraph,
    contract: &PlannerContract,
) -> Result<RollbackPlan, AnvilError> {
    if !contract.phases.iter().any(|p| p.phase_id == phase_id) {
        return Err(AnvilError::UnknownPhase(phase_id.to_owned()));
    }

    let all_invalidated = graph.dependents(phase_id);

    let mut all_reset_phases = vec![phase_id.to_owned()];
    all_reset_phases.extend(all_invalidated.clone());

    Ok(RollbackPlan {
        target_phase: phase_id.to_owned(),
        all_invalidated,
        all_reset_phases,
    })
}

/// Writes one `RollbackEvent` per phase in `plan.all_reset_phases` to the audit store.
///
/// One record is written for the target phase itself, plus one per transitive dependent.
/// The `rotation_reset_phases` field is the complete `plan.all_reset_phases` list —
/// identical across all sibling records so consumers can reconstruct the full reopen scope
/// from any single record.
///
/// Audit-store append-only semantics are preserved: this function only creates new records
/// and never modifies or deletes existing ones (enforces
/// `test_audit_store_immutable_through_rollback`).
///
/// # Errors
///
/// Returns [`AnvilError`] if any audit-store write fails. On failure the store may
/// contain a partial set of records (those successfully appended before the error).
pub fn execute_rollback(
    plan: &RollbackPlan,
    store: &AuditStore,
    reason: &str,
) -> Result<(), AnvilError> {
    for invalidated_phase in &plan.all_reset_phases {
        let event = RollbackEvent::new(
            plan.target_phase.clone(),
            invalidated_phase.clone(),
            plan.all_reset_phases.clone(),
            reason.to_owned(),
            vec![],
        );
        store.append(&event)?;
    }
    Ok(())
}

/// Returns the number of `ReviewerFindingPacket` records in the current rollback epoch
/// for `phase_id`.
///
/// - **No prior rollback:** returns the total count of all RFPs for this phase (equivalent
///   to the pre-P9 behaviour, preserving backward compatibility).
/// - **Prior rollback exists:** returns the count of RFPs created **after** the latest
///   `RollbackEvent` that invalidated `phase_id`.
///
/// The caller uses this value to derive the rotation round for reviewer selection:
/// `rotation_round = rotation_offset + 1`. After a reopen the offset is 0, so the
/// first reviewer in the pool is selected again (enforces the rotation-reset invariant).
///
/// **Retry / duplicate-rollback semantics:** if [`execute_rollback`] is called twice for
/// the same phase (e.g., retrying after a partial write), both sets of `RollbackEvent`
/// records exist in the store. `latest_rollback_at` uses `max(created_at)`, so the epoch
/// boundary advances to the second rollback's timestamp. Any RFPs created between the two
/// rollback calls are excluded from the offset count. This is conservative — it requires
/// more reviews post-retry — and is correct.
///
/// # Errors
///
/// Returns [`AnvilError`] if the audit store cannot be read.
pub fn rotation_offset_for_phase(phase_id: &str, store: &AuditStore) -> Result<u32, AnvilError> {
    let artifact_prefix = format!("phase:{phase_id}");

    // Find the latest rollback event that invalidates this phase.
    let latest_rollback_at: Option<DateTime<Utc>> = {
        let entries = store.list(RecordType::RollbackEvent)?;
        entries
            .iter()
            .filter_map(|e| {
                store
                    .get(&e.id)
                    .ok()
                    .and_then(|v| serde_json::from_value::<RollbackEvent>(v).ok())
                    .filter(|r| r.invalidated_phase == phase_id)
                    .map(|r| r.created_at)
            })
            .max()
    };

    // Count RFPs in the current epoch (post-rollback, or all if no rollback).
    let rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
    let count = rfp_entries
        .iter()
        .filter_map(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<ReviewerFindingPacket>(v).ok())
                .filter(|rfp| rfp.packet.artifact_ref.starts_with(&artifact_prefix))
        })
        .filter(|rfp| match latest_rollback_at {
            Some(rollback_at) => rfp.created_at > rollback_at,
            None => true,
        })
        .count();

    Ok(u32::try_from(count).unwrap_or(u32::MAX))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_audit::AuditStore;
    use anvil_core::plan::{PlannerContract, PlannerPhase};

    fn make_phase(id: &str, deps: &[&str]) -> PlannerPhase {
        PlannerPhase {
            phase_id: id.to_owned(),
            name: id.to_owned(),
            goal: "goal".to_owned(),
            action_list: vec!["action".to_owned()],
            deliverable: "deliverable".to_owned(),
            acceptance_criteria: vec!["ac".to_owned()],
            dependencies: deps.iter().map(ToString::to_string).collect(),
            hinge_tests: vec![],
            evaluation_metric_impact: "none".to_owned(),
            estimated_rounds: None,
        }
    }

    fn make_contract(phases: Vec<PlannerPhase>) -> PlannerContract {
        PlannerContract {
            plan_version: "1.0.0".to_owned(),
            charter_ref: "charter.md:v1".to_owned(),
            phases,
        }
    }

    fn init_store(dir: &tempfile::TempDir) -> AuditStore {
        anvil_core::project::init(dir.path()).unwrap();
        AuditStore::open(dir.path()).unwrap()
    }

    fn build_graph(contract: &PlannerContract) -> PhaseDepGraph {
        PhaseDepGraph::build_from_contract(contract)
    }

    // hinge_test: pins=rollback_transitive_invalidation, intended=cascading-invalidation, phase=P9
    #[test]
    fn test_rollback_transitive_invalidation() {
        // Pins: compute_rollback_plan must include all transitive dependents in all_invalidated,
        // and execute_rollback must write one RollbackEvent per phase in all_reset_phases.
        // Changing the transitive closure logic or the number of records written requires
        // updating this test together with the implementation.
        let contract = make_contract(vec![
            make_phase("P0", &[]),
            make_phase("P1", &["P0"]),
            make_phase("P2", &["P1"]),
            make_phase("P3", &["P2"]),
        ]);
        let graph = build_graph(&contract);

        let plan = compute_rollback_plan("P1", &graph, &contract).unwrap();
        assert_eq!(plan.target_phase, "P1");
        assert!(
            plan.all_invalidated.contains(&"P2".to_owned()),
            "P2 must be invalidated (direct dependent)"
        );
        assert!(
            plan.all_invalidated.contains(&"P3".to_owned()),
            "P3 must be invalidated (transitive dependent)"
        );
        assert!(
            !plan.all_invalidated.contains(&"P0".to_owned()),
            "P0 must not be invalidated (it is a dependency, not a dependent)"
        );
        assert!(
            plan.all_reset_phases.contains(&"P1".to_owned()),
            "target phase must be in all_reset_phases"
        );

        // Write to the store and count records.
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);
        execute_rollback(&plan, &store, "test rollback").unwrap();

        let entries = store.list(RecordType::RollbackEvent).unwrap();
        assert_eq!(
            entries.len(),
            plan.all_reset_phases.len(),
            "one RollbackEvent per reset phase"
        );

        // All records share the same rotation_reset_phases list.
        let all_phase_ids: Vec<String> = entries
            .iter()
            .map(|e| {
                let v = store.get(&e.id).unwrap();
                let r: RollbackEvent = serde_json::from_value(v).unwrap();
                assert_eq!(
                    r.rotation_reset_phases, plan.all_reset_phases,
                    "rotation_reset_phases must be identical across sibling records"
                );
                r.invalidated_phase
            })
            .collect();

        for phase in &plan.all_reset_phases {
            assert!(
                all_phase_ids.contains(phase),
                "missing RollbackEvent for phase '{phase}'"
            );
        }
    }

    // hinge_test: pins=rollback_resets_rotation_on_dependents, intended=full-pool-diversity, phase=P9
    #[test]
    fn test_rollback_resets_rotation_on_dependents() {
        // Pins: after execute_rollback, rotation_offset_for_phase returns 0 for every
        // invalidated phase so rotation_select picks pool[0] again.
        // Flipping this requires updating the rotation-reset logic and this test together.
        let contract = make_contract(vec![
            make_phase("P0", &[]),
            make_phase("P1", &["P0"]),
            make_phase("P2", &["P1"]),
        ]);
        let graph = build_graph(&contract);

        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);

        let plan = compute_rollback_plan("P1", &graph, &contract).unwrap();

        // Before rollback, offset is 0 for a fresh phase.
        assert_eq!(rotation_offset_for_phase("P1", &store).unwrap(), 0);
        assert_eq!(rotation_offset_for_phase("P2", &store).unwrap(), 0);

        // After rollback the offset is still 0 (no RFPs exist, so the reset has no visible
        // effect on an empty store — but this confirms the API doesn't error).
        execute_rollback(&plan, &store, "test rotation reset").unwrap();
        assert_eq!(rotation_offset_for_phase("P1", &store).unwrap(), 0);
        assert_eq!(rotation_offset_for_phase("P2", &store).unwrap(), 0);

        // The RollbackEvent records list P1 and P2 in rotation_reset_phases.
        let entries = store.list(RecordType::RollbackEvent).unwrap();
        let any_record: RollbackEvent = {
            let v = store.get(&entries[0].id).unwrap();
            serde_json::from_value(v).unwrap()
        };
        assert!(
            any_record.rotation_reset_phases.contains(&"P1".to_owned()),
            "P1 must appear in rotation_reset_phases"
        );
        assert!(
            any_record.rotation_reset_phases.contains(&"P2".to_owned()),
            "P2 must appear in rotation_reset_phases"
        );
    }

    #[test]
    fn test_compute_rollback_plan_unknown_phase() {
        let contract = make_contract(vec![make_phase("P0", &[])]);
        let graph = build_graph(&contract);
        let err = compute_rollback_plan("P_MISSING", &graph, &contract).unwrap_err();
        assert!(
            matches!(err, AnvilError::UnknownPhase(id) if id == "P_MISSING"),
            "unknown phase must produce UnknownPhase error"
        );
    }

    #[test]
    fn test_rotation_offset_for_phase_no_rollback() {
        // Without a rollback, offset equals total RFP count (pre-P9 behaviour).
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);
        // Fresh store has zero RFPs.
        assert_eq!(rotation_offset_for_phase("P0", &store).unwrap(), 0);
    }

    #[test]
    fn test_rollback_retry_appends_duplicate_records_without_breaking_offset() {
        // Pins: a second execute_rollback (partial-write retry) produces a second set of
        // RollbackEvent records (append-only store), and rotation_offset_for_phase returns
        // 0 for all affected phases because no RFPs exist in any epoch.
        //
        // What this test does NOT pin: the max(created_at) semantics of latest_rollback_at.
        // Pinning that requires an RFP created strictly between the two rollback timestamps,
        // which would need a sleep. The max rule is specified in rotation_offset_for_phase.
        let contract = make_contract(vec![make_phase("P0", &[]), make_phase("P1", &["P0"])]);
        let graph = build_graph(&contract);
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);

        let plan = compute_rollback_plan("P1", &graph, &contract).unwrap();

        // First rollback (initial attempt).
        execute_rollback(&plan, &store, "first attempt").unwrap();
        let count_after_first = store.list(RecordType::RollbackEvent).unwrap().len();
        assert_eq!(
            count_after_first,
            plan.all_reset_phases.len(),
            "first rollback must write one record per reset phase"
        );

        // Second rollback (simulated retry after partial-write failure).
        execute_rollback(&plan, &store, "retry").unwrap();
        let count_after_retry = store.list(RecordType::RollbackEvent).unwrap().len();
        assert_eq!(
            count_after_retry,
            plan.all_reset_phases.len() * 2,
            "retry must append a second set of records (append-only store)"
        );

        // rotation_offset_for_phase must return 0 for all phases after retry (no RFPs).
        for phase in &plan.all_reset_phases {
            assert_eq!(
                rotation_offset_for_phase(phase, &store).unwrap(),
                0,
                "offset must be 0 for '{phase}' after retry (no RFPs in any epoch)"
            );
        }
    }
}
