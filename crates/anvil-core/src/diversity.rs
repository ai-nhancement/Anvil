/// Model family classification for the adversarial-diversity policy.
///
/// Family membership is determined by the model identity string prefix.
/// The family-floor invariant requires that Reviewer-1 and Reviewer-2
/// come from different families, and both differ from the Coder's family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelFamily {
    Anthropic,
    OpenAi,
    Google,
    XAi,
    /// Unrecognized prefix — treated as its own opaque family.
    Unknown(String),
}

impl ModelFamily {
    /// Returns a human-readable name for error messages.
    #[must_use]
    pub fn display_name(&self) -> &str {
        match self {
            Self::Anthropic => "Anthropic (Claude)",
            Self::OpenAi => "OpenAI (GPT/O-series)",
            Self::Google => "Google (Gemini)",
            Self::XAi => "xAI (Grok)",
            Self::Unknown(s) => s,
        }
    }
}

/// Infers the model family from a model identity string.
///
/// Model identity strings are free-form (e.g., `claude-opus-4-7`, `gpt-4o-mini`,
/// `gemini-1.5-flash`). Family classification is prefix-based and intentionally
/// conservative: unrecognized prefixes are `Unknown`, not assumed to be from any
/// known family. This prevents accidental diversity-floor bypass via novel model names.
#[must_use]
pub fn model_family(identity: &str) -> ModelFamily {
    let lower = identity.to_lowercase();
    if lower.starts_with("claude") {
        ModelFamily::Anthropic
    } else if lower.starts_with("gpt")
        || lower.starts_with("o1")
        || lower.starts_with("o3")
        || lower.starts_with("o4")
    {
        ModelFamily::OpenAi
    } else if lower.starts_with("gemini") {
        ModelFamily::Google
    } else if lower.starts_with("grok") {
        ModelFamily::XAi
    } else {
        ModelFamily::Unknown(identity.to_owned())
    }
}

/// A diversity policy violation: two roles assigned to models from the same family.
#[derive(Debug, Clone)]
pub struct DiversityViolation {
    pub role_a: &'static str,
    pub role_b: &'static str,
    pub shared_family: ModelFamily,
}

impl std::fmt::Display for DiversityViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} and {} both use the {} family — a different family is required for adversarial independence",
            self.role_a,
            self.role_b,
            self.shared_family.display_name()
        )
    }
}

/// Validates the adversarial diversity policy for the three key roles.
///
/// Invariant: all three families must be pairwise distinct.
/// Returns `Ok(())` if the policy is satisfied; returns a non-empty `Vec` of violations
/// otherwise. The caller should surface every violation, not just the first.
#[must_use]
pub fn validate_diversity(
    coder: &str,
    reviewer1: &str,
    reviewer2: &str,
) -> Vec<DiversityViolation> {
    let cf = model_family(coder);
    let r1f = model_family(reviewer1);
    let r2f = model_family(reviewer2);

    let mut violations = Vec::new();

    if cf == r1f {
        violations.push(DiversityViolation {
            role_a: "Coder",
            role_b: "Reviewer-1",
            shared_family: cf.clone(),
        });
    }
    if cf == r2f {
        violations.push(DiversityViolation {
            role_a: "Coder",
            role_b: "Reviewer-2",
            shared_family: cf.clone(),
        });
    }
    if r1f == r2f {
        violations.push(DiversityViolation {
            role_a: "Reviewer-1",
            role_b: "Reviewer-2",
            shared_family: r1f.clone(),
        });
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    // hinge_test: pins=diversity-rejects-same-family, intended=adversarial-diversity-enforcement, phase=P4
    #[test]
    fn test_diversity_policy_validation_rejects_same_family() {
        // Coder = Claude, Reviewer-1 = Claude → family floor violated.
        let violations = validate_diversity("claude-opus-4-7", "claude-haiku-4-5", "gpt-4o");
        assert!(
            !violations.is_empty(),
            "Coder+Reviewer-1 both Anthropic must produce a violation"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.role_a == "Coder" && v.role_b == "Reviewer-1"),
            "violation must name Coder and Reviewer-1"
        );
    }

    #[test]
    fn test_diversity_policy_valid_pool() {
        let violations = validate_diversity("claude-opus-4-7", "gpt-4o", "gemini-1.5-pro");
        assert!(
            violations.is_empty(),
            "Claude+GPT+Gemini must pass: {violations:?}"
        );
    }

    #[test]
    fn test_diversity_all_same_family() {
        let violations =
            validate_diversity("claude-opus-4-7", "claude-haiku-4-5", "claude-sonnet-4-6");
        assert_eq!(
            violations.len(),
            3,
            "all three Claude must produce 3 violations"
        );
    }

    #[test]
    fn test_model_family_classification() {
        assert_eq!(model_family("claude-opus-4-7"), ModelFamily::Anthropic);
        assert_eq!(model_family("gpt-4o"), ModelFamily::OpenAi);
        assert_eq!(model_family("o1-preview"), ModelFamily::OpenAi);
        assert_eq!(model_family("o3-mini"), ModelFamily::OpenAi);
        assert_eq!(model_family("gemini-1.5-pro"), ModelFamily::Google);
        assert_eq!(model_family("grok-3"), ModelFamily::XAi);
        assert!(matches!(
            model_family("unknown-model-x"),
            ModelFamily::Unknown(_)
        ));
    }
}
