// P11 — Dogfooding and Documentation phase hinge tests.

#[cfg(test)]
mod tests {
    // hinge_test: pins=pl-all-resolved, intended=test_no_outstanding_provisional_locks_after_dogfooding, phase=P11
    #[test]
    fn test_no_outstanding_provisional_locks_after_dogfooding() {
        // Pins: after P11 dogfooding all 8 Provisional Locks are confirmed Final.
        // The strings below are the canonical choice_key slugs from the Required Choices
        // table in ANVIL_PLAN.md; each slug appears in parentheses in that table's
        // "Choice" column. This test is a naming-convention and count assertion — it
        // enforces that someone deliberately edits this list whenever a PL is added,
        // reopened, or re-keyed. It does not read the Plan or audit store at runtime.
        // "Final" means the v1 decision is locked; v1.1 may independently differ.
        let confirmed_final: &[&str] = &[
            "plan-consolidation-triggers",     // trigger: P7 done
            "per-metric-numeric-thresholds",   // trigger: P10a done, baselines established
            "file-system-layout",              // trigger: P0 done, no structural issues found
            "deferred-decision-tracking",      // trigger: P10b done, hinge registry operational
            "ship-transport-actions",          // trigger: P9 done, configurable transport works
            "runtime-alert-response-policies", // trigger: P10a done, warning-mode confirmed
            "cli-setup-wizard-step-ordering",  // v1.1-prep trigger reached; v1 wizard Final
            "cli-command-structure",           // v1.1-prep trigger reached; v1 surface Final
        ];

        assert_eq!(
            confirmed_final.len(),
            8,
            "all 8 Provisional Locks must be confirmed Final at P11 ship"
        );
    }

    // hinge_test: pins=manual-sync, intended=test_contract_doc_sync_method, phase=P11
    #[test]
    fn test_contract_doc_sync_method() {
        // Pins: docs/contract.md is manually synced from proto/anvil/v1/sidecar.proto in v1.
        // No automated CI check exists to detect drift between the doc and the proto.
        // Flipping to "ci-enforced" requires adding a CI step that extracts service/RPC/
        // message definitions from the proto or generated descriptors and fails on mismatch.
        // That step is explicitly a v1.1 task (noted in docs/contract.md maintenance note).
        let contract_doc = include_str!("../../../docs/contract.md");
        assert!(
            contract_doc.contains("Automated drift detection is a v1.1 task"),
            "docs/contract.md must retain the maintenance note; if it was removed, \
             update this test to reflect the new sync approach"
        );
    }
}
