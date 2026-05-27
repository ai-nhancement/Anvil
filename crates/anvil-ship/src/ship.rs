//! Project-level ship readiness checks for `anvil ship` (P9).
//!
//! ## Shipped-state authority
//!
//! A phase is considered "currently shipped" when a `PhaseDisposition` record with
//! `disposition == DISPOSITION_SHIPPED` exists and is newer than the latest `RollbackEvent`
//! that invalidated it.  The `phase-{id}-ship` `GateApproval` is **not** used for this
//! determination; it is retained solely as historical audit provenance.
//!
//! **Rationale (P9 R3):** `run_phase_ship` appends the gate first and the disposition second.
//! If the disposition append fails the gate exists without the corresponding state record,
//! which would cause the old gate-based check to falsely consider the phase shipped.
//! Using `PhaseDisposition` as the sole authority eliminates this partial-failure gap.

use chrono::{DateTime, Utc};

use anvil_audit::{
    records::{PhaseDisposition, RollbackEvent, DISPOSITION_SHIPPED},
    AuditStore, RecordType,
};
use anvil_core::{error::AnvilError, plan::PlannerContract};

/// Result of the project-level ship readiness check.
#[derive(Debug, Clone)]
pub struct ShipReadiness {
    /// Phase IDs that are NOT currently in shipped state.
    ///
    /// A phase is "not shipped" if it has no `PhaseDisposition` with
    /// `disposition == DISPOSITION_SHIPPED`, or its latest shipped disposition is older
    /// than its latest `RollbackEvent`.
    pub unshipped_phases: Vec<String>,
}

impl ShipReadiness {
    /// Returns `true` when every phase in the checked contract is currently shipped.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.unshipped_phases.is_empty()
    }
}

/// Checks that every phase declared in `contract` is currently in shipped state.
///
/// A phase is "currently shipped" when it has a `PhaseDisposition` record with
/// `disposition == DISPOSITION_SHIPPED` whose `created_at` is strictly newer than the
/// latest `RollbackEvent` that invalidated it (or, if no rollback event exists, any
/// shipped disposition suffices). The `phase-{id}-ship` `GateApproval` is not used for
/// this determination; see the module-level docs for the rationale.
///
/// # Errors
///
/// Returns [`AnvilError`] if the audit store cannot be read.
pub fn check_all_phases_shipped(
    store: &AuditStore,
    contract: &PlannerContract,
) -> Result<ShipReadiness, AnvilError> {
    let mut unshipped_phases = Vec::new();
    for phase in &contract.phases {
        if !is_phase_currently_shipped(store, &phase.phase_id)? {
            unshipped_phases.push(phase.phase_id.clone());
        }
    }
    Ok(ShipReadiness { unshipped_phases })
}

/// Returns the phase IDs in `contract` that have been rolled back without a subsequent re-ship.
///
/// This is the subset of unshipped phases that specifically have an existing `RollbackEvent`
/// (i.e. phases that were once shipped, then explicitly re-opened). Phases that were never
/// shipped at all are excluded — use [`check_all_phases_shipped`] to catch those.
///
/// The CLI uses this to distinguish "never shipped" from "shipped then rolled back" in its
/// error messaging, and to enforce AC6 of the P9 acceptance criteria.
///
/// # Errors
///
/// Returns [`AnvilError`] if the audit store cannot be read.
pub fn check_unresolved_rollbacks(
    store: &AuditStore,
    contract: &PlannerContract,
) -> Result<Vec<String>, AnvilError> {
    let mut unresolved = Vec::new();
    for phase in &contract.phases {
        let phase_id = &phase.phase_id;
        if latest_rollback_at(store, phase_id)?.is_some()
            && !is_phase_currently_shipped(store, phase_id)?
        {
            unresolved.push(phase_id.clone());
        }
    }
    Ok(unresolved)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns `true` if `phase_id` has a shipped `PhaseDisposition` newer than any rollback.
fn is_phase_currently_shipped(store: &AuditStore, phase_id: &str) -> Result<bool, AnvilError> {
    let Some(ship_at) = latest_shipped_disposition_at(store, phase_id)? else {
        return Ok(false);
    };
    match latest_rollback_at(store, phase_id)? {
        // Strict greater-than is intentional: a RollbackEvent is always written after
        // the shipped PhaseDisposition it invalidates, so equality is impossible in a
        // well-formed store.
        Some(rollback_at) if rollback_at > ship_at => Ok(false),
        _ => Ok(true),
    }
}

/// Returns the `created_at` of the most recent `PhaseDisposition` for `phase_id` with
/// `disposition == DISPOSITION_SHIPPED`.
///
/// Using `PhaseDisposition` (rather than the `phase-{id}-ship` `GateApproval`) as the
/// authoritative shipped-state record ensures that a partial phase-ship failure — where
/// the gate was written but the disposition was not — does not incorrectly satisfy the
/// readiness check.
///
/// **Performance note:** this function walks the full `PhaseDisposition` list and deserializes
/// every record to filter by `phase_id`. Acceptable at v1 scale; a future secondary index
/// (`phase_id` → latest disposition timestamp) would eliminate the linear scan.
fn latest_shipped_disposition_at(
    store: &AuditStore,
    phase_id: &str,
) -> Result<Option<DateTime<Utc>>, AnvilError> {
    let entries = store.list(RecordType::PhaseDisposition)?;
    Ok(entries
        .iter()
        .filter_map(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<PhaseDisposition>(v).ok())
                .filter(|d| d.phase_id == phase_id && d.disposition == DISPOSITION_SHIPPED)
                .map(|d| d.created_at)
        })
        .max())
}

/// Returns the `created_at` of the most recent `RollbackEvent` where `invalidated_phase == phase_id`.
fn latest_rollback_at(
    store: &AuditStore,
    phase_id: &str,
) -> Result<Option<DateTime<Utc>>, AnvilError> {
    let entries = store.list(RecordType::RollbackEvent)?;
    Ok(entries
        .iter()
        .filter_map(|e| {
            store
                .get(&e.id)
                .ok()
                .and_then(|v| serde_json::from_value::<RollbackEvent>(v).ok())
                .filter(|r| r.invalidated_phase == phase_id)
                .map(|r| r.created_at)
        })
        .max())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_audit::records::{GateApproval, PhaseDisposition};
    use anvil_core::plan::{PlannerContract, PlannerPhase};

    fn make_phase(id: &str) -> PlannerPhase {
        PlannerPhase {
            phase_id: id.to_owned(),
            name: id.to_owned(),
            goal: "goal".to_owned(),
            action_list: vec!["action".to_owned()],
            deliverable: "deliverable".to_owned(),
            acceptance_criteria: vec!["ac".to_owned()],
            dependencies: vec![],
            hinge_tests: vec![],
            evaluation_metric_impact: "none".to_owned(),
            estimated_rounds: None,
        }
    }

    fn make_contract(phase_ids: &[&str]) -> PlannerContract {
        PlannerContract {
            plan_version: "1.0.0".to_owned(),
            charter_ref: "charter.md:v1".to_owned(),
            phases: phase_ids.iter().map(|id| make_phase(id)).collect(),
        }
    }

    fn init_store(dir: &tempfile::TempDir) -> AuditStore {
        anvil_core::project::init(dir.path()).unwrap();
        AuditStore::open(dir.path()).unwrap()
    }

    // hinge_test: pins=audit_store_immutable_through_rollback, intended=append-only, phase=P9
    #[test]
    fn test_audit_store_immutable_through_rollback() {
        // Pins: execute_rollback only appends new RollbackEvent records; it never modifies
        // or deletes any existing record. Changing this invariant requires updating the
        // rollback implementation and this test together.
        use crate::rollback::{compute_rollback_plan, execute_rollback};
        use anvil_graph::phase_graph::PhaseDepGraph;

        let contract = make_contract(&["P0", "P1", "P2"]);
        let graph = PhaseDepGraph::build_from_contract(&contract);
        let plan = compute_rollback_plan("P1", &graph, &contract).unwrap();

        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);

        // Populate pre-existing records to count baseline.
        let pre_gate =
            GateApproval::new("phase-P0-ship".to_owned(), "coordinator".to_owned(), vec![]);
        store.append(&pre_gate).unwrap();

        let pre_existing_count = store.list(RecordType::GateApproval).unwrap().len()
            + store.list(RecordType::RollbackEvent).unwrap().len();

        execute_rollback(&plan, &store, "immutability test").unwrap();

        let post_gate_count = store.list(RecordType::GateApproval).unwrap().len();
        let post_rollback_count = store.list(RecordType::RollbackEvent).unwrap().len();

        // GateApproval count must be unchanged (rollback does not touch existing records).
        assert_eq!(
            post_gate_count, 1,
            "rollback must not modify or delete the pre-existing GateApproval"
        );
        // RollbackEvent count equals the number of reset phases.
        assert_eq!(
            post_rollback_count,
            plan.all_reset_phases.len(),
            "one RollbackEvent per reset phase"
        );
        // Total record count grew by exactly plan.all_reset_phases.len().
        let post_total = post_gate_count + post_rollback_count;
        assert_eq!(
            post_total,
            pre_existing_count + plan.all_reset_phases.len(),
            "only new records were added; no deletions"
        );
    }

    #[test]
    fn test_check_all_phases_shipped_empty_store() {
        let contract = make_contract(&["P0", "P1"]);
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);

        let readiness = check_all_phases_shipped(&store, &contract).unwrap();
        assert!(!readiness.is_ready());
        assert!(readiness.unshipped_phases.contains(&"P0".to_owned()));
        assert!(readiness.unshipped_phases.contains(&"P1".to_owned()));
    }

    #[test]
    fn test_check_all_phases_shipped_all_shipped() {
        let contract = make_contract(&["P0", "P1"]);
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);

        for phase_id in &["P0", "P1"] {
            let gate = GateApproval::new(
                format!("phase-{phase_id}-ship"),
                "coordinator".to_owned(),
                vec![],
            );
            store.append(&gate).unwrap();
            let disposition =
                PhaseDisposition::new(phase_id.to_string(), "shipped".to_owned(), vec![]);
            store.append(&disposition).unwrap();
        }

        let readiness = check_all_phases_shipped(&store, &contract).unwrap();
        assert!(
            readiness.is_ready(),
            "all phases shipped should pass: {:?}",
            readiness.unshipped_phases
        );
    }

    #[test]
    fn test_check_all_phases_shipped_gate_without_disposition_blocks() {
        // Regression for F2: a phase-{id}-ship GateApproval alone must not satisfy shipped
        // state; the PhaseDisposition record is the authoritative signal.
        let contract = make_contract(&["P0"]);
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);

        let gate = GateApproval::new("phase-P0-ship".to_owned(), "coordinator".to_owned(), vec![]);
        store.append(&gate).unwrap();

        let readiness = check_all_phases_shipped(&store, &contract).unwrap();
        assert!(
            !readiness.is_ready(),
            "gate without PhaseDisposition must not satisfy shipped state"
        );
    }

    #[test]
    fn test_check_all_phases_shipped_after_rollback_blocks() {
        use crate::rollback::{compute_rollback_plan, execute_rollback};
        use anvil_graph::phase_graph::PhaseDepGraph;

        let contract = make_contract(&["P0", "P1"]);
        let graph = PhaseDepGraph::build_from_contract(&contract);
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);

        // Ship P1 — gate + disposition (both required for shipped state).
        let gate = GateApproval::new("phase-P1-ship".to_owned(), "coordinator".to_owned(), vec![]);
        store.append(&gate).unwrap();
        let disposition = PhaseDisposition::new("P1".to_owned(), "shipped".to_owned(), vec![]);
        store.append(&disposition).unwrap();

        // Rollback P1 (no dependents in this contract).
        let plan = compute_rollback_plan("P1", &graph, &contract).unwrap();
        execute_rollback(&plan, &store, "regression").unwrap();

        // P1 should now appear as unshipped.
        let readiness = check_all_phases_shipped(&store, &contract).unwrap();
        assert!(
            readiness.unshipped_phases.contains(&"P1".to_owned()),
            "P1 must be unshipped after rollback"
        );
    }

    #[test]
    fn test_check_unresolved_rollbacks_empty() {
        let contract = make_contract(&["P0"]);
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_store(&tmp);
        let unresolved = check_unresolved_rollbacks(&store, &contract).unwrap();
        assert!(unresolved.is_empty(), "no rollbacks → no unresolved");
    }
}
