use std::collections::BTreeMap;

/// Lock state of a Required Choice as stored in `anvil.toml`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockState {
    Final,
    Provisional,
    Unlocked,
}

/// A single Required Choice entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Choice {
    /// Human-readable description of the chosen value.
    pub value: String,
    pub lock_state: LockState,
    /// Required when `lock_state` == `Provisional`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hypothesis: Option<String>,
    /// Required when `lock_state` == `Provisional`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision_trigger: Option<String>,
}

impl Choice {
    /// Returns `true` if this choice has a `Final` or `Provisional` lock state.
    #[must_use]
    pub fn is_locked(&self) -> bool {
        !matches!(self.lock_state, LockState::Unlocked)
    }

    /// Validates that a `Provisional` choice has non-empty `hypothesis` and `revision_trigger`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::error::AnvilError::ProvisionalMissingField`] if either required
    /// field is absent or blank.
    pub fn validate(&self, key: &str) -> Result<(), crate::error::AnvilError> {
        if self.lock_state == LockState::Provisional {
            if self.hypothesis.as_deref().unwrap_or("").trim().is_empty() {
                return Err(crate::error::AnvilError::ProvisionalMissingField {
                    key: key.to_owned(),
                    field: "hypothesis",
                });
            }
            if self
                .revision_trigger
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                return Err(crate::error::AnvilError::ProvisionalMissingField {
                    key: key.to_owned(),
                    field: "revision_trigger",
                });
            }
        }
        Ok(())
    }
}

/// 17 canonical Required Choice keys (9 Final + 8 Provisional). Changing this set requires a Charter/Plan amendment.
pub const CHOICE_KEYS: &[&str] = &[
    "coder_model_version",
    "reviewer_pool",
    "termination_condition",
    "convergence_round_limit",
    "interlocutor_model",
    "planner_model",
    "v1_deliverable_form",
    "implementation_language",
    "sidecar_lifecycle",
    "plan_consolidation_triggers",
    "per_metric_numeric_thresholds",
    "file_system_layout",
    "deferred_decision_tracking_mechanism",
    "ship_transport_actions",
    "runtime_alert_response_policies",
    "cli_setup_wizard_step_ordering",
    "cli_command_structure",
];

/// Returns the default Required-Choices map as written by `anvil init`.
///
/// All choices start in their plan-locked state (Final or Provisional).
#[must_use]
pub fn default_choices() -> BTreeMap<String, Choice> {
    let mut m = BTreeMap::new();

    let mut f = |key: &str, value: &str| {
        m.insert(
            key.to_owned(),
            Choice {
                value: value.to_owned(),
                lock_state: LockState::Final,
                hypothesis: None,
                revision_trigger: None,
            },
        );
    };

    f("coder_model_version", "claude (current production version)");
    f(
        "reviewer_pool",
        "codex-class + gemini-class (minimum v1; each via configurable provider connection)",
    );
    f("termination_condition", "full-pool clean (default)");
    f(
        "convergence_round_limit",
        "5 rounds before severity-tiering activates",
    );
    f("interlocutor_model", "claude (same as coder, default)");
    f("planner_model", "claude (same as coder, default)");
    f(
        "v1_deliverable_form",
        "cli (primary v1 surface); app scoped to v1.1",
    );
    f(
        "implementation_language",
        "rust (core vault) >=1.80 + go sidecar >=1.22",
    );
    f(
        "sidecar_lifecycle",
        "workspace-scoped daemon, cli-managed; spawns on first invocation requiring model access",
    );

    let mut p = |key: &str, value: &str, hypothesis: &str, revision_trigger: &str| {
        m.insert(
            key.to_owned(),
            Choice {
                value: value.to_owned(),
                lock_state: LockState::Provisional,
                hypothesis: Some(hypothesis.to_owned()),
                revision_trigger: Some(revision_trigger.to_owned()),
            },
        );
    };

    p(
        "plan_consolidation_triggers",
        "phase boundary trigger",
        "Phase boundary is the natural granularity for plan consolidation decisions.",
        "End of P7 (first Build-stage phase)",
    );
    p(
        "per_metric_numeric_thresholds",
        "see evaluation metric targets in plan",
        "Numeric thresholds require real project data to calibrate accurately.",
        "First three Build phases ship and produce baseline",
    );
    p(
        "file_system_layout",
        "per-project directory layout (see plan)",
        "Layout is correct for CLI v1; may need adjustment once App is designed.",
        "P0 scaffolds the actual tree; revisit if layout proves awkward",
    );
    p(
        "deferred_decision_tracking_mechanism",
        "hinge tests via cargo test (rust) and go test (go); P10 unifies collection",
        "Hinge tests are sufficient for tracking deferred decisions through P10.",
        "P10 stands up the registry",
    );
    p(
        "ship_transport_actions",
        "git commit (anvil dev); configurable for user projects",
        "Git commit is the right default transport for CLI-based workflow.",
        "P9 (Ship + Rollback)",
    );
    p(
        "runtime_alert_response_policies",
        "alerts surface to cli as warnings in v1",
        "CLI warnings are sufficient alert surface for v1; dashboards are v1.x.",
        "P10 (Evaluation infrastructure)",
    );
    p(
        "cli_setup_wizard_step_ordering",
        "seven-step interactive wizard via anvil setup",
        "Seven-step wizard maps well to CLI; App wizard may need restructuring.",
        "v1.1 App design begins; validate against v1 usage feedback",
    );
    p(
        "cli_command_structure",
        "verb-resource pattern (anvil <resource> <verb>)",
        "Verb-resource pattern is ergonomic for CLI and maps reasonably to App views.",
        "v1.1 App design begins; validate that structure maps cleanly to App",
    );

    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provisional_validate_rejects_blank_hypothesis() {
        let choice = Choice {
            value: "some value".to_owned(),
            lock_state: LockState::Provisional,
            hypothesis: Some("  ".to_owned()),
            revision_trigger: Some("some trigger".to_owned()),
        };
        let err = choice.validate("test_key").unwrap_err();
        assert!(
            matches!(
                err,
                crate::error::AnvilError::ProvisionalMissingField { field, .. }
                    if field == "hypothesis"
            ),
            "expected ProvisionalMissingField for hypothesis, got: {err}"
        );
    }

    #[test]
    fn test_provisional_validate_rejects_blank_revision_trigger() {
        let choice = Choice {
            value: "some value".to_owned(),
            lock_state: LockState::Provisional,
            hypothesis: Some("a real hypothesis".to_owned()),
            revision_trigger: None,
        };
        let err = choice.validate("test_key").unwrap_err();
        assert!(
            matches!(
                err,
                crate::error::AnvilError::ProvisionalMissingField { field, .. }
                    if field == "revision_trigger"
            ),
            "expected ProvisionalMissingField for revision_trigger, got: {err}"
        );
    }

    // hinge_test: pins=17, intended=required-choices-count, phase=P1
    #[test]
    fn test_required_choices_count() {
        // Pins: the Required-Choices schema must have exactly 17 entries.
        // Original annotation said pins=16; updated to 17 because
        // "runtime_alert_response_policies" was added to the plan table in a later
        // revision. Changing this count requires a Charter/Plan amendment.
        assert_eq!(
            CHOICE_KEYS.len(),
            17,
            "Required-Choices schema must have exactly 17 keys"
        );
        let defaults = default_choices();
        assert_eq!(
            defaults.len(),
            17,
            "default_choices() must return exactly 17 entries"
        );
        for key in CHOICE_KEYS {
            assert!(
                defaults.contains_key(*key),
                "default_choices() missing key: {key}"
            );
        }
    }
}
