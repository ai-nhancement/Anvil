//! `anvil graph` subcommands (P7):
//! - `anvil graph show`         — display all phases and their direct dependencies
//! - `anvil graph blast-radius` — show transitive dependents of a given phase

use std::path::Path;

use anvil_core::{error::AnvilError, plan::PlannerContract};
use anvil_graph::PhaseDepGraph;

// ── anvil graph show ───────────────────────────────────────────────────────────

/// Runs `anvil graph show` — loads the Plan contract from `.anvil/plan_contract.json`
/// (written by `anvil plan invoke`), builds the phase dependency graph, and prints
/// each phase with its direct dependencies.
///
/// # Errors
///
/// Returns [`AnvilError`] on I/O or parse failure.
pub fn run_graph_show(project_root: &Path) -> Result<(), AnvilError> {
    let contract = load_contract(project_root)?;
    let graph = PhaseDepGraph::build_from_contract(&contract);

    let dangling = graph.dangling_deps();
    if !dangling.is_empty() {
        eprintln!(
            "warning: {} dangling dependency reference(s) in contract (phase IDs not found): {}",
            dangling.len(),
            dangling.join(", ")
        );
    }

    println!("Phase Dependency Graph ({} phases):", contract.phases.len());
    println!();

    for phase in &contract.phases {
        let deps = graph.dependencies(&phase.phase_id);
        if deps.is_empty() {
            println!("  {} — {} (no dependencies)", phase.phase_id, phase.name);
        } else {
            println!(
                "  {} — {} depends on: {}",
                phase.phase_id,
                phase.name,
                deps.join(", ")
            );
        }
    }

    Ok(())
}

// ── anvil graph blast-radius ───────────────────────────────────────────────────

/// Runs `anvil graph blast-radius <phase_id>` — shows all transitive dependents of
/// `phase_id` (phases that would be affected if `phase_id` changes).
///
/// # Errors
///
/// Returns [`AnvilError`] on I/O or parse failure.
pub fn run_graph_blast_radius(project_root: &Path, phase_id: &str) -> Result<(), AnvilError> {
    let contract = load_contract(project_root)?;
    let graph = PhaseDepGraph::build_from_contract(&contract);

    let dangling = graph.dangling_deps();
    if !dangling.is_empty() {
        eprintln!(
            "warning: {} dangling dependency reference(s) in contract (phase IDs not found): {}",
            dangling.len(),
            dangling.join(", ")
        );
    }

    let affected = graph.blast_radius(phase_id);

    if affected.is_empty() {
        println!("Blast radius of '{phase_id}': no dependents (leaf phase or unknown).");
    } else {
        println!(
            "Blast radius of '{phase_id}': {} phase(s) affected:",
            affected.len()
        );
        for dep in &affected {
            println!("  {dep}");
        }
    }

    Ok(())
}

// ── Contract loader ────────────────────────────────────────────────────────────

/// Loads the `PlannerContract` from `.anvil/plan_contract.json`, written by `anvil plan invoke`.
fn load_contract(project_root: &Path) -> Result<PlannerContract, AnvilError> {
    let contract_path = project_root.join(".anvil/plan_contract.json");
    if !contract_path.exists() {
        return Err(AnvilError::Io(std::io::Error::other(
            ".anvil/plan_contract.json not found — run `anvil plan invoke` first",
        )));
    }
    let json = std::fs::read_to_string(&contract_path)?;
    serde_json::from_str(&json).map_err(|e| AnvilError::ModelResponseBadJson {
        reason: format!("plan_contract.json: {e}"),
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_core::plan::{PlannerContract, PlannerPhase};

    fn make_phase(id: &str, deps: &[&str]) -> PlannerPhase {
        PlannerPhase {
            phase_id: id.to_owned(),
            name: format!("Phase {id}"),
            goal: "goal".to_owned(),
            action_list: vec!["action".to_owned()],
            deliverable: "deliverable".to_owned(),
            acceptance_criteria: vec!["ac".to_owned()],
            dependencies: deps.iter().map(std::string::ToString::to_string).collect(),
            hinge_tests: vec![],
            evaluation_metric_impact: "none".to_owned(),
            estimated_rounds: None,
        }
    }

    fn write_plan_contract(root: &Path, contract: &PlannerContract) {
        let anvil_dir = root.join(".anvil");
        std::fs::create_dir_all(&anvil_dir).unwrap();
        let json = serde_json::to_string_pretty(contract).unwrap();
        std::fs::write(anvil_dir.join("plan_contract.json"), json).unwrap();
    }

    #[test]
    fn test_graph_show_no_deps() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let contract = PlannerContract {
            plan_version: "1.0.0".to_owned(),
            charter_ref: "charter.md:v1".to_owned(),
            phases: vec![make_phase("P0", &[])],
        };
        write_plan_contract(root, &contract);
        // Should succeed without error.
        run_graph_show(root).expect("graph show");
    }

    #[test]
    fn test_graph_blast_radius_known_phase() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let contract = PlannerContract {
            plan_version: "1.0.0".to_owned(),
            charter_ref: "charter.md:v1".to_owned(),
            phases: vec![
                make_phase("P0", &[]),
                make_phase("P1", &["P0"]),
                make_phase("P2", &["P1"]),
            ],
        };
        write_plan_contract(root, &contract);
        // Should succeed — P0 blast radius is P1, P2.
        run_graph_blast_radius(root, "P0").expect("blast radius");
    }

    #[test]
    fn test_graph_blast_radius_unknown_phase() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let contract = PlannerContract {
            plan_version: "1.0.0".to_owned(),
            charter_ref: "charter.md:v1".to_owned(),
            phases: vec![make_phase("P0", &[])],
        };
        write_plan_contract(root, &contract);
        // Unknown phase returns empty — should not error.
        run_graph_blast_radius(root, "P99").expect("blast radius unknown");
    }

    #[test]
    fn test_load_contract_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let result = load_contract(tmp.path());
        assert!(result.is_err(), "must fail when plan_contract.json absent");
    }
}
