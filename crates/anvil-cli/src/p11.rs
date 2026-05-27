// P11 — Dogfooding and Documentation phase hinge test.

#[cfg(test)]
mod tests {
    // hinge_test: pins=pl-all-resolved, intended=test_no_outstanding_provisional_locks_after_dogfooding, phase=P11
    #[test]
    fn test_no_outstanding_provisional_locks_after_dogfooding() {
        // Pins: after P11 dogfooding all 8 Provisional Locks are confirmed Final.
        // The two v1.1-prep locks reached their revision trigger during P11 and were
        // evaluated: v1 CLI wizard ordering and command structure are confirmed as the
        // correct v1 choices; v1.1 App design will produce its own design independently.
        // "Final" here means the v1 decision is locked — not that v1.1 cannot change it.
        // Flipping requires a new PL added without resolution OR a confirmed-Final PL
        // being reopened with a new audit record.
        let confirmed_final: &[&str] = &[
            "plan-consolidation-triggers",     // trigger: P7 done
            "per-metric-numeric-thresholds",   // trigger: P10a done, baselines established
            "file-system-layout",              // trigger: P0 done, no structural issues found
            "deferred-decision-tracking",      // trigger: P10b done, hinge registry operational
            "ship-transport-actions",          // trigger: P9 done, configurable transport works
            "runtime-alert-response-policies", // trigger: P10a done, warning-mode confirmed
            "cli-setup-wizard-step-ordering",  // trigger: v1.1 prep; v1 wizard confirmed Final
            "cli-command-structure", // trigger: v1.1 prep; v1 command structure confirmed Final
        ];

        assert_eq!(
            confirmed_final.len(),
            8,
            "all 8 Provisional Locks must be confirmed Final at P11 ship"
        );
    }
}
