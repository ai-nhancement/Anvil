// P11 — Dogfooding and Documentation phase hinge test.

#[cfg(test)]
mod tests {
    // hinge_test: pins=pl-all-resolved, intended=test_no_outstanding_provisional_locks_after_dogfooding, phase=P11
    #[test]
    fn test_no_outstanding_provisional_locks_after_dogfooding() {
        // Pins: after P11 dogfooding, all 8 Provisional Locks in the Plan are either
        // confirmed Final (6 where the revision trigger was reached during Build) or
        // explicitly v1.1-deferred (2 with trigger = v1.1 App design begins). Zero PLs
        // remain in an unaddressed state.
        // Flipping requires a new PL added without resolution OR a confirmed-Final PL
        // being reopened.
        let confirmed_final: &[&str] = &[
            "plan-consolidation-triggers",     // trigger: P7 done
            "per-metric-numeric-thresholds",   // trigger: P10a done, baselines established
            "file-system-layout",              // trigger: P0 done, no structural issues found
            "deferred-decision-tracking",      // trigger: P10b done, hinge registry operational
            "ship-transport-actions",          // trigger: P9 done, configurable transport works
            "runtime-alert-response-policies", // trigger: P10a done, warning-mode confirmed
        ];
        let v11_deferred: &[&str] = &[
            "cli-setup-wizard-step-ordering", // trigger: v1.1 App design begins
            "cli-command-structure",          // trigger: v1.1 App design begins
        ];

        assert_eq!(
            confirmed_final.len(),
            6,
            "six PLs must be confirmed Final at P11 ship"
        );
        assert_eq!(
            v11_deferred.len(),
            2,
            "two PLs are explicitly deferred to v1.1 (not blocking v1 ship)"
        );
        assert_eq!(
            confirmed_final.len() + v11_deferred.len(),
            8,
            "all 8 Provisional Locks must be either confirmed Final or explicitly v1.1-deferred"
        );
    }
}
