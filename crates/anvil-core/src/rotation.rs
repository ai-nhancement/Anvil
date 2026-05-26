//! Multi-reviewer rotation arithmetic (P6).
//!
//! Provides deterministic round-robin selection, advisory-round detection,
//! and the data types for full-pool clean checks.

// hinge_test: pins=round_limit_default_is_5, intended=advisory-threshold, phase=P6
/// Rounds after which P2/P3 findings become advisory.
/// Rounds 1–5 are blocking; round 6+ are advisory.
/// Flipping this constant requires updating the reviewer prompt and this annotation together.
pub const ADVISORY_THRESHOLD_ROUND: u32 = 5;

/// Returns the reviewer binding name for a 1-indexed round using round-robin selection.
///
/// - `round_count = 1` → `pool[0]`
/// - `round_count = 2` → `pool[1]`
/// - `round_count > pool.len()` → wraps back to the start
///
/// Returns `None` if `pool` is empty.
#[must_use]
pub fn rotation_select(pool: &[String], round_count: u32) -> Option<&str> {
    if pool.is_empty() {
        return None;
    }
    let idx = (round_count.saturating_sub(1) as usize) % pool.len();
    Some(&pool[idx])
}

/// Returns `true` if `round_count` is in advisory mode (`round_count > ADVISORY_THRESHOLD_ROUND`).
///
/// Rounds 6+ are advisory: P2/P3 findings become advisory rather than blocking.
#[must_use]
pub fn is_advisory_round(round_count: u32) -> bool {
    round_count > ADVISORY_THRESHOLD_ROUND
}

/// Result of a full-pool clean termination check.
#[derive(Debug, Clone)]
pub struct FullPoolCheckResult {
    /// `true` if all pool members have submitted a clean pass (or single-clean-pass override active).
    pub all_clean: bool,
    /// Reviewer binding names that have NOT yet submitted a clean pass.
    pub not_clean: Vec<String>,
    /// Whether the result was driven by the single-clean-pass override.
    pub override_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    // hinge_test: pins=round_limit_default_is_5, intended=advisory-threshold, phase=P6
    #[test]
    fn test_round_limit_default_is_5() {
        // Pins: the advisory threshold is 5 (rounds 6+ are advisory).
        // Flipping requires changing the threshold constant AND updating all round-counting logic.
        assert_eq!(ADVISORY_THRESHOLD_ROUND, 5);
        assert!(!is_advisory_round(5), "round 5 must still be blocking");
        assert!(is_advisory_round(6), "round 6 must be advisory");
    }

    // hinge_test: pins=severity_tiering_at_round_6, intended=advisory-tiering, phase=P6
    #[test]
    fn test_severity_tiering_at_round_6() {
        // Pins: is_advisory_round returns false for rounds 1-5, true for rounds 6+.
        // Flipping requires also updating apply_severity_tiering and the reviewer system prompt.
        for r in 1..=5 {
            assert!(
                !is_advisory_round(r),
                "round {r} should not be advisory (rounds 1–5 are blocking)"
            );
        }
        for r in 6..=10 {
            assert!(
                is_advisory_round(r),
                "round {r} should be advisory (rounds 6+ are advisory)"
            );
        }
    }

    #[test]
    fn test_rotation_select_round_robin() {
        let pool = vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()];
        assert_eq!(rotation_select(&pool, 1), Some("r1"));
        assert_eq!(rotation_select(&pool, 2), Some("r2"));
        assert_eq!(rotation_select(&pool, 3), Some("r3"));
        assert_eq!(rotation_select(&pool, 4), Some("r1")); // wraps
        assert_eq!(rotation_select(&pool, 5), Some("r2"));
    }

    #[test]
    fn test_rotation_select_empty_pool() {
        assert_eq!(rotation_select(&[], 1), None);
    }

    #[test]
    fn test_rotation_select_single_reviewer() {
        let pool = vec!["reviewer-1".to_owned()];
        for round in 1..=5 {
            assert_eq!(rotation_select(&pool, round), Some("reviewer-1"));
        }
    }
}
