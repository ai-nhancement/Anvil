//! Phase Dependency Graph for queryable transitive dependency resolution (P7).
//!
//! Distinct from `ProvenanceGraph` (which indexes audit-store cross-references).
//! This graph encodes the declared phase-to-phase dependency edges from the
//! Planner Contract and answers transitive-reachability queries.

use std::collections::{HashMap, HashSet, VecDeque};

use anvil_core::plan::PlannerContract;

/// Queryable directed phase dependency graph.
///
/// Edges are directed from a phase to its **dependencies** (prerequisite phases).
/// A "blast radius" query walks the reverse edges (dependents).
pub struct PhaseDepGraph {
    /// `phase_id` → direct dependency phase IDs (forward edges: "depends on").
    deps: HashMap<String, Vec<String>>,
    /// `phase_id` → direct dependent phase IDs (reverse edges: "required by").
    rdeps: HashMap<String, Vec<String>>,
}

impl PhaseDepGraph {
    /// Builds the graph from a `PlannerContract`.
    ///
    /// All phase IDs declared in the contract are registered, even those with
    /// no dependencies, so `dependencies` and `dependents` return empty vecs
    /// for leaf phases rather than `None`.
    #[must_use]
    pub fn build_from_contract(contract: &PlannerContract) -> Self {
        let mut deps: HashMap<String, Vec<String>> = HashMap::new();
        let mut rdeps: HashMap<String, Vec<String>> = HashMap::new();

        for phase in &contract.phases {
            deps.entry(phase.phase_id.clone()).or_default();
            rdeps.entry(phase.phase_id.clone()).or_default();
        }

        for phase in &contract.phases {
            for dep in &phase.dependencies {
                deps.entry(phase.phase_id.clone())
                    .or_default()
                    .push(dep.clone());
                rdeps
                    .entry(dep.clone())
                    .or_default()
                    .push(phase.phase_id.clone());
            }
        }

        Self { deps, rdeps }
    }

    /// Returns all **transitive** dependencies of `phase_id` (phases it depends on,
    /// directly or indirectly), in BFS order. The result does not include `phase_id` itself.
    ///
    /// Returns an empty vec if the phase is unknown or has no dependencies.
    #[must_use]
    pub fn dependencies(&self, phase_id: &str) -> Vec<String> {
        Self::reachable(&self.deps, phase_id)
    }

    /// Returns all **transitive** dependents of `phase_id` (phases that depend on it,
    /// directly or indirectly), in BFS order. The result does not include `phase_id` itself.
    ///
    /// Returns an empty vec if the phase is unknown or has no dependents.
    #[must_use]
    pub fn dependents(&self, phase_id: &str) -> Vec<String> {
        Self::reachable(&self.rdeps, phase_id)
    }

    /// Alias for `dependents`: the set of phases that would be affected by a change to
    /// `phase_id`.
    #[must_use]
    pub fn blast_radius(&self, phase_id: &str) -> Vec<String> {
        self.dependents(phase_id)
    }

    /// BFS traversal from `start` following `edges`. Does not include `start`.
    fn reachable(edges: &HashMap<String, Vec<String>>, start: &str) -> Vec<String> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut order: Vec<String> = Vec::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        if let Some(nexts) = edges.get(start) {
            for n in nexts {
                if visited.insert(n.clone()) {
                    queue.push_back(n.clone());
                    order.push(n.clone());
                }
            }
        }

        while let Some(cur) = queue.pop_front() {
            if let Some(nexts) = edges.get(cur.as_str()) {
                for n in nexts {
                    if visited.insert(n.clone()) {
                        queue.push_back(n.clone());
                        order.push(n.clone());
                    }
                }
            }
        }

        order
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use anvil_core::plan::{PlannerContract, PlannerPhase};

    use super::*;

    fn phase(id: &str, deps: &[&str]) -> PlannerPhase {
        PlannerPhase {
            phase_id: id.to_owned(),
            name: id.to_owned(),
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

    fn make_graph(phases: Vec<PlannerPhase>) -> PhaseDepGraph {
        let contract = PlannerContract {
            plan_version: "1.0.0".to_owned(),
            charter_ref: "charter.md:v1".to_owned(),
            phases,
        };
        PhaseDepGraph::build_from_contract(&contract)
    }

    #[test]
    fn test_phase_graph_no_deps() {
        let g = make_graph(vec![phase("P0", &[])]);
        assert!(g.dependencies("P0").is_empty());
        assert!(g.dependents("P0").is_empty());
        assert!(g.blast_radius("P0").is_empty());
    }

    #[test]
    fn test_phase_graph_direct_dep() {
        // P1 depends on P0.
        let g = make_graph(vec![phase("P0", &[]), phase("P1", &["P0"])]);
        assert_eq!(g.dependencies("P1"), vec!["P0"]);
        assert!(g.dependencies("P0").is_empty());
        assert_eq!(g.dependents("P0"), vec!["P1"]);
        assert_eq!(g.blast_radius("P0"), vec!["P1"]);
    }

    #[test]
    fn test_phase_graph_transitive_deps() {
        // P0 → P1 → P2 (P2 depends on P1 which depends on P0).
        let g = make_graph(vec![
            phase("P0", &[]),
            phase("P1", &["P0"]),
            phase("P2", &["P1"]),
        ]);

        let deps_p2 = g.dependencies("P2");
        assert!(deps_p2.contains(&"P1".to_owned()), "P2 depends on P1");
        assert!(
            deps_p2.contains(&"P0".to_owned()),
            "P2 transitively depends on P0"
        );

        let br_p0 = g.blast_radius("P0");
        assert!(
            br_p0.contains(&"P1".to_owned()),
            "P0 blast radius includes P1"
        );
        assert!(
            br_p0.contains(&"P2".to_owned()),
            "P0 blast radius includes P2"
        );
    }

    #[test]
    fn test_phase_graph_blast_radius_diamond() {
        // P0 → P1, P0 → P2, P1 → P3, P2 → P3 (diamond).
        let g = make_graph(vec![
            phase("P0", &[]),
            phase("P1", &["P0"]),
            phase("P2", &["P0"]),
            phase("P3", &["P1", "P2"]),
        ]);

        let br = g.blast_radius("P0");
        assert!(br.contains(&"P1".to_owned()));
        assert!(br.contains(&"P2".to_owned()));
        assert!(br.contains(&"P3".to_owned()));
        // P3 appears exactly once (no duplicates from diamond).
        assert_eq!(br.iter().filter(|x| x.as_str() == "P3").count(), 1);
    }

    #[test]
    fn test_phase_graph_unknown_phase_empty() {
        let g = make_graph(vec![phase("P0", &[])]);
        assert!(g.dependencies("P99").is_empty());
        assert!(g.blast_radius("P99").is_empty());
    }
}
