// P11 — Dogfooding and Documentation phase hinge tests.

#[cfg(test)]
mod tests {
    // hinge_test: pins=pl-all-resolved, intended=test_no_outstanding_provisional_locks_after_dogfooding, phase=P11
    #[test]
    fn test_no_outstanding_provisional_locks_after_dogfooding() {
        // The strings below are the canonical choice_key slugs from the Required Choices
        // table in ANVIL_PLAN.md; each slug appears in parentheses in that table's
        // "Choice" column. This list is a deliberate governance artifact: any addition,
        // removal, or rename requires a code change here AND a Plan-table update.
        // "Final" means the v1 decision is locked; v1.1 may independently differ.
        // The runtime check below verifies this list stays in sync with the Plan table.
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

        // Runtime verification: extract Final-at-P11 slugs from the Plan table and assert
        // the hard-coded list matches exactly. Fails if the Plan table is updated without
        // updating this list, or vice versa.
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap() // crates/
            .parent()
            .unwrap(); // workspace root

        let plan_doc =
            std::fs::read_to_string(workspace_root.join("Anvil Plan").join("ANVIL_PLAN.md"))
                .expect("Anvil Plan/ANVIL_PLAN.md not found; check workspace layout");

        // Scope extraction to the "## Locked Required Project-Level Choices" section so
        // unrelated table rows elsewhere in the Plan cannot match. Fail fast with a clear
        // message if the section header is ever renamed.
        let lines: Vec<&str> = plan_doc.lines().collect();
        let section_start = lines
            .iter()
            .position(|line| line.trim() == "## Locked Required Project-Level Choices")
            .expect(
                "Section '## Locked Required Project-Level Choices' not found in ANVIL_PLAN.md; \
                 check section header",
            );
        let section_end = lines[section_start + 1..]
            .iter()
            .position(|line| line.trim().starts_with("## "))
            .map_or(lines.len(), |rel| section_start + 1 + rel);

        // Each Required Choices table row for a Final-at-P11 PL looks like:
        //   | Choice text (`slug`) | **Final (P11)** | ...
        // Bold markers are stripped before matching so minor formatting changes
        // (e.g. "**Final** (P11)" vs "**Final (P11)**") do not silently break extraction.
        // Split by | to get the Choice column, then extract the backtick-enclosed slug.
        let plan_slugs: Vec<String> = lines[section_start..section_end]
            .iter()
            .copied()
            .filter(|line| line.replace("**", "").contains("Final (P11)"))
            .filter_map(|line| {
                let cols: Vec<&str> = line.split('|').collect();
                cols.get(1).and_then(|col| {
                    let mut parts = col.split('`');
                    parts.next(); // text before first backtick
                    parts.next().map(std::string::ToString::to_string) // the slug
                })
            })
            .collect();

        assert_eq!(
            plan_slugs.len(),
            confirmed_final.len(),
            "ANVIL_PLAN.md Required Choices table has {} Final-at-P11 PLs but this list has {}; \
             update whichever is stale",
            plan_slugs.len(),
            confirmed_final.len()
        );

        // Forward check: every hard-coded slug must appear in the Plan table.
        for slug in confirmed_final {
            assert!(
                plan_slugs.iter().any(|s| s == slug),
                "slug '{slug}' is in this list but not in ANVIL_PLAN.md Required Choices table; \
                 update the Plan table or remove this slug"
            );
        }

        // Reverse check: every Plan-table slug must appear in the hard-coded list.
        // Together with the forward check and the count assertion, this is a full
        // bidirectional synchronization — neither side can add a slug without the other.
        for slug in &plan_slugs {
            assert!(
                confirmed_final.contains(&slug.as_str()),
                "slug '{slug}' is in ANVIL_PLAN.md Required Choices table but not in this list; \
                 add it here or update the Plan table"
            );
        }
    }

    // hinge_test: pins=manual-sync, intended=test_contract_doc_sync_method, phase=P11
    #[test]
    fn test_contract_doc_sync_method() {
        // Pins: docs/contract.md is manually synced from proto/anvil/v1/sidecar.proto in v1.
        // Smoke test: verifies that (1) every service name and (2) every RPC name from the
        // proto appear as substrings in the contract doc. Does NOT check request/response
        // types, message fields, field numbers, oneof variants, enum values, or package.
        // Full schema-level CI enforcement is explicitly a v1.1 task.
        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap() // crates/
            .parent()
            .unwrap(); // workspace root

        let contract_doc = std::fs::read_to_string(workspace_root.join("docs").join("contract.md"))
            .expect("docs/contract.md not found; check workspace layout");

        let proto = std::fs::read_to_string(
            workspace_root
                .join("proto")
                .join("anvil")
                .join("v1")
                .join("sidecar.proto"),
        )
        .expect("proto/anvil/v1/sidecar.proto not found; check workspace layout");

        // Every service name from the proto must appear in the contract doc.
        let service_names: Vec<&str> = proto
            .lines()
            .filter_map(|line| {
                let t = line.trim();
                if t.starts_with("service ") {
                    t.strip_prefix("service ")
                        .and_then(|r| r.split_whitespace().next())
                } else {
                    None
                }
            })
            .collect();

        assert!(
            !service_names.is_empty(),
            "sidecar.proto defines no services — check parse logic or proto path"
        );

        for service in &service_names {
            assert!(
                contract_doc.contains(service),
                "docs/contract.md is missing proto service name '{service}'; \
                 manual sync may have drifted — update contract.md"
            );
        }

        // All RPC names from the proto must appear in the contract doc.
        let rpc_names: Vec<&str> = proto
            .lines()
            .filter_map(|line| {
                let t = line.trim();
                t.strip_prefix("rpc ")
                    .and_then(|r| r.split('(').next())
                    .map(str::trim)
            })
            .filter(|s| !s.is_empty())
            .collect();

        assert!(
            !rpc_names.is_empty(),
            "sidecar.proto defines no RPCs — check parse logic or proto path"
        );

        for rpc in &rpc_names {
            assert!(
                contract_doc.contains(rpc),
                "docs/contract.md is missing proto RPC '{rpc}'; \
                 manual sync may have drifted — update contract.md"
            );
        }

        assert!(
            contract_doc.contains("Automated drift detection is a v1.1 task"),
            "docs/contract.md must retain the maintenance note; \
             if removed, update this test to reflect the new sync approach"
        );
    }
}
