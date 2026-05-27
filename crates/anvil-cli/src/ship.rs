//! `anvil ship` — project-level ship gate (P9).
//!
//! Checks that all phases are in shipped state, then executes configured transport
//! actions in declaration order.

use std::path::Path;

use anvil_audit::AuditStore;
use anvil_core::{config::load_config, error::AnvilError, plan::parse_planner_contract};
use anvil_ship::{
    check_all_phases_shipped, check_unresolved_rollbacks, execute_transport,
    parse_transport_actions,
};

/// Runs `anvil ship`.
///
/// # Errors
///
/// Returns [`AnvilError::ProjectShipBlocked`] when the readiness checks fail, or
/// [`AnvilError::TransportFailed`] when a transport action exits non-zero.
pub fn run_project_ship(project_root: &Path) -> Result<(), AnvilError> {
    let config = load_config(project_root)?;
    let store = AuditStore::open(project_root)?;

    let contract = load_plan_contract(project_root)?;

    // AC1 + AC6 — Run both readiness checks and surface a combined error so the caller
    // sees all blockers in a single message rather than having to fix-and-retry.
    let readiness = check_all_phases_shipped(&store, &contract)?;
    let unresolved = check_unresolved_rollbacks(&store, &contract)?;

    if !readiness.is_ready() || !unresolved.is_empty() {
        let mut parts = Vec::new();
        // Phases that were never shipped (or have no ship gate newer than their rollback).
        let never_shipped: Vec<&String> = readiness
            .unshipped_phases
            .iter()
            .filter(|id| !unresolved.contains(id))
            .collect();
        if !never_shipped.is_empty() {
            parts.push(format!(
                "never shipped: {}",
                never_shipped
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !unresolved.is_empty() {
            parts.push(format!(
                "rolled back without re-ship: {}",
                unresolved.join(", ")
            ));
        }
        return Err(AnvilError::ProjectShipBlocked(parts.join("; ")));
    }

    // AC2 — Execute configured transport actions in declared order.
    let actions = parse_transport_actions(&config);
    if actions.is_empty() {
        println!("No transport_actions configured — ship gate passed with no external commands.");
        println!("Add [[transport_actions]] entries to anvil.toml to run commands on ship.");
    } else {
        println!("Executing {} transport action(s)…", actions.len());
        execute_transport(&actions, project_root)?;
    }

    println!("✓ Project shipped successfully.");
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn load_plan_contract(
    project_root: &Path,
) -> Result<anvil_core::plan::PlannerContract, AnvilError> {
    let contract_path = project_root.join(".anvil/plan_contract.json");
    let json = std::fs::read_to_string(&contract_path).map_err(|_| {
        AnvilError::Io(std::io::Error::other(
            "plan_contract.json not found — run `anvil plan invoke` first",
        ))
    })?;
    parse_planner_contract(&json)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_audit::records::{GateApproval, PhaseDisposition};
    use anvil_core::plan::{PlannerContract, PlannerPhase};

    fn init_project(dir: &tempfile::TempDir) -> AuditStore {
        anvil_core::project::init(dir.path()).unwrap();
        AuditStore::open(dir.path()).unwrap()
    }

    fn write_contract(dir: &tempfile::TempDir, contract: &PlannerContract) {
        std::fs::create_dir_all(dir.path().join(".anvil")).unwrap();
        let json = serde_json::to_string(contract).unwrap();
        std::fs::write(dir.path().join(".anvil/plan_contract.json"), json).unwrap();
    }

    fn make_contract(phase_ids: &[&str]) -> PlannerContract {
        PlannerContract {
            plan_version: "1.0.0".to_owned(),
            charter_ref: "charter.md:v1".to_owned(),
            phases: phase_ids
                .iter()
                .map(|id| PlannerPhase {
                    phase_id: id.to_string(),
                    name: id.to_string(),
                    goal: "goal".to_owned(),
                    action_list: vec!["action".to_owned()],
                    deliverable: "deliverable".to_owned(),
                    acceptance_criteria: vec!["ac".to_owned()],
                    dependencies: vec![],
                    hinge_tests: vec![],
                    evaluation_metric_impact: "none".to_owned(),
                    estimated_rounds: None,
                })
                .collect(),
        }
    }

    #[test]
    fn test_project_ship_blocked_when_phases_not_shipped() {
        let tmp = tempfile::TempDir::new().unwrap();
        let _store = init_project(&tmp);
        let contract = make_contract(&["P0", "P1"]);
        write_contract(&tmp, &contract);

        let err = run_project_ship(tmp.path()).unwrap_err();
        assert!(
            matches!(err, AnvilError::ProjectShipBlocked(_)),
            "unshipped phases must block ship: {err}"
        );
    }

    #[test]
    fn test_project_ship_succeeds_when_all_shipped() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = init_project(&tmp);
        let contract = make_contract(&["P0", "P1"]);
        write_contract(&tmp, &contract);

        for id in &["P0", "P1"] {
            let gate =
                GateApproval::new(format!("phase-{id}-ship"), "coordinator".to_owned(), vec![]);
            store.append(&gate).unwrap();
            let disposition = PhaseDisposition::new(id.to_string(), "shipped".to_owned(), vec![]);
            store.append(&disposition).unwrap();
        }

        // Should succeed (no transport actions configured in the default config).
        run_project_ship(tmp.path()).unwrap();
    }
}
