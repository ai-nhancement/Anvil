//! Planner Contract types and validation (P7).
//!
//! The Planner Contract is the structured output produced by the Planner specialist.
//! It declares per-phase details for the project Plan.

use serde::{Deserialize, Serialize};

use crate::error::AnvilError;

// ── Planner Contract types ─────────────────────────────────────────────────────

/// The nine required per-phase fields per the Artifact Specifications Phase Definition sub-spec.
pub const REQUIRED_PHASE_FIELDS: [&str; 9] = [
    "phase_id",
    "name",
    "goal",
    "action_list",
    "deliverable",
    "acceptance_criteria",
    "dependencies",
    "hinge_tests",
    "evaluation_metric_impact",
];

/// One phase entry in the Planner Contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlannerPhase {
    pub phase_id: String,
    pub name: String,
    pub goal: String,
    pub action_list: Vec<String>,
    pub deliverable: String,
    pub acceptance_criteria: Vec<String>,
    /// Direct dependency phase IDs.
    pub dependencies: Vec<String>,
    pub hinge_tests: Vec<String>,
    pub evaluation_metric_impact: String,
    /// Optional author estimate of review rounds needed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_rounds: Option<u32>,
}

/// The full Planner Contract as extracted from the Planner model's response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerContract {
    pub plan_version: String,
    /// Cross-reference key for the Charter that was consumed (e.g. `"charter.md:v2.1"`).
    pub charter_ref: String,
    pub phases: Vec<PlannerPhase>,
}

// ── Validation ────────────────────────────────────────────────────────────────

/// Validates that every phase in `contract` has all nine required fields non-empty.
///
/// Returns a `Vec` of errors — one per (phase, field) pair where the field is empty
/// or an empty list/string. The caller decides whether to treat these as fatal.
// hinge_test: pins=planner_contract_required_fields, intended=contract-compliance, phase=P7
#[must_use]
pub fn validate_planner_contract(contract: &PlannerContract) -> Vec<AnvilError> {
    let mut errors = Vec::new();
    for phase in &contract.phases {
        macro_rules! require_str {
            ($field:expr, $name:literal) => {
                if $field.trim().is_empty() {
                    errors.push(AnvilError::PhaseMissingField {
                        phase_id: phase.phase_id.clone(),
                        field: $name,
                    });
                }
            };
        }
        macro_rules! require_vec {
            ($field:expr, $name:literal) => {
                if $field.is_empty() {
                    errors.push(AnvilError::PhaseMissingField {
                        phase_id: phase.phase_id.clone(),
                        field: $name,
                    });
                }
            };
        }
        require_str!(phase.phase_id, "phase_id");
        require_str!(phase.name, "name");
        require_str!(phase.goal, "goal");
        require_vec!(phase.action_list, "action_list");
        require_str!(phase.deliverable, "deliverable");
        require_vec!(phase.acceptance_criteria, "acceptance_criteria");
        // dependencies may legitimately be empty (P0 has none); not required to be non-empty.
        // hinge_tests may be empty for phases with no deferred decisions.
        require_str!(phase.evaluation_metric_impact, "evaluation_metric_impact");
    }
    errors
}

// ── Extraction ────────────────────────────────────────────────────────────────

/// Extracts the content between `<planner_contract>` and `</planner_contract>` tags.
#[must_use]
pub fn extract_planner_contract_json(response: &str) -> Option<&str> {
    let open = "<planner_contract>";
    let close = "</planner_contract>";
    let start = response.find(open)? + open.len();
    let end = response[start..].find(close)?;
    Some(response[start..start + end].trim())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_phase(phase_id: &str) -> PlannerPhase {
        PlannerPhase {
            phase_id: phase_id.to_owned(),
            name: "Bootstrap".to_owned(),
            goal: "Initialize the workspace.".to_owned(),
            action_list: vec!["Create directories.".to_owned()],
            deliverable: "Scaffold present.".to_owned(),
            acceptance_criteria: vec!["anvil init succeeds.".to_owned()],
            dependencies: vec![],
            hinge_tests: vec![],
            evaluation_metric_impact: "None at P0.".to_owned(),
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

    // hinge_test: pins=planner_contract_required_fields, intended=contract-compliance, phase=P7
    #[test]
    fn test_planner_contract_required_fields() {
        // Pins: validate_planner_contract must detect each missing required field and name it.
        // Flipping requires updating the required-field list and this test together.

        // Valid contract passes with no errors.
        let valid = make_contract(vec![make_phase("P0")]);
        assert!(
            validate_planner_contract(&valid).is_empty(),
            "fully populated phase must pass validation"
        );

        // Missing goal.
        let mut bad_goal = make_phase("P1");
        bad_goal.goal = String::new();
        let errs = validate_planner_contract(&make_contract(vec![bad_goal]));
        assert_eq!(errs.len(), 1);
        assert!(
            matches!(&errs[0], AnvilError::PhaseMissingField { phase_id, field }
                if phase_id == "P1" && *field == "goal"),
            "expected PhaseMissingField for goal, got: {:?}",
            errs[0]
        );

        // Missing action_list (empty vec).
        let mut bad_actions = make_phase("P2");
        bad_actions.action_list.clear();
        let errs = validate_planner_contract(&make_contract(vec![bad_actions]));
        assert_eq!(errs.len(), 1);
        assert!(
            matches!(&errs[0], AnvilError::PhaseMissingField { phase_id, field }
                if phase_id == "P2" && *field == "action_list"),
            "expected PhaseMissingField for action_list, got: {:?}",
            errs[0]
        );

        // Missing evaluation_metric_impact.
        let mut bad_metric = make_phase("P3");
        bad_metric.evaluation_metric_impact = "   ".to_owned();
        let errs = validate_planner_contract(&make_contract(vec![bad_metric]));
        assert_eq!(errs.len(), 1);
        assert!(
            matches!(&errs[0], AnvilError::PhaseMissingField { phase_id, field }
                if phase_id == "P3" && *field == "evaluation_metric_impact"),
            "expected PhaseMissingField for evaluation_metric_impact, got: {:?}",
            errs[0]
        );

        // Multiple phases, multiple errors reported.
        let mut bad1 = make_phase("P4");
        bad1.goal = String::new();
        let mut bad2 = make_phase("P5");
        bad2.deliverable = String::new();
        let errs = validate_planner_contract(&make_contract(vec![bad1, bad2]));
        assert_eq!(errs.len(), 2, "two phases with one error each → two errors");
    }

    #[test]
    fn test_extract_planner_contract_json() {
        let response = r#"
Here is the plan.
<planner_contract>
{"plan_version": "1.0.0"}
</planner_contract>
Thanks.
"#;
        assert_eq!(
            extract_planner_contract_json(response),
            Some("{\"plan_version\": \"1.0.0\"}")
        );
        assert!(extract_planner_contract_json("no tags here").is_none());
    }
}
