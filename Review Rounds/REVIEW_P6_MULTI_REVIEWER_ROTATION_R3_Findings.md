# Anvil — P6 Multi-Reviewer Rotation R3 Findings

**Source review doc:** `Review Rounds/REVIEW_P6_MULTI_REVIEWER_ROTATION_R3.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Pass**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass** (91 tests)

R3's stated validation result is accurate. I also verified the claimed R3 changes in the source:

- `DropAdvisory` and `DeferAdvisory` prompts now use `.allow_empty(false)` in `crates/anvil-cli/src/charter.rs`.
- `test_advisory_gate_rejects_drop_advisory_with_empty_annotation` exists and covers Drop/Defer empty annotation failures plus Accept-Advisory pass behavior.
- `run_status` now emits a warning for artifact-scoped reviewer packets lacking `artifact_hash` when a current artifact hash exists.
- `run_resolve_finding` now has tests for `PacketNotFound` and `FindingNotFound` variants and message content.

---

## 1. Medium — Open advisory counts can use the latest curation record from the wrong packet/artifact

**Location:**

- `crates/anvil-cli/src/status.rs:193-215`
- `crates/anvil-cli/src/arbiter.rs:197-220`
- `crates/anvil-audit/src/records.rs:434-441`
- `Anvil Plan/ANVIL_PLAN.md:635,638`

**Problem:**

The R2/R3 work correctly scopes RFPs by artifact for status and convergence declaration counts. However, both advisory-count helpers still load the globally latest `CuratedFindingsRecord` and apply its dispositions to the latest artifact-scoped `ReviewerFindingPacket`:

```rust
let curated_entries = store.list(RecordType::CuratedFindings)?;
let dispositions = if let Some(last) = curated_entries.last() {
    let curated: CuratedFindingsRecord = serde_json::from_value(store.get(&last.id)?)?;
    curated.dispositions
} else {
    vec![]
};

let missing = check_advisory_gate(&dispositions, &rfp.packet.findings);
```

But `CuratedFindingsRecord` has a `packet_id` field. The helpers do not require `curated.packet_id == rfp.packet.packet_id`, and they also do not filter by cross-reference/artifact. This means a later curation from another round or another artifact can be treated as if it disposes the latest packet's advisory findings.

Concrete failure mode:

1. `charter.md` latest RFP has advisory finding `F1`, not curated yet.
2. A later Plan/phase/other-artifact curation record also has a disposition for finding ID `F1`.
3. `anvil status --artifact charter.md` and `anvil arbiter declare-convergence charter.md` load that globally latest curation record.
4. `check_advisory_gate` sees a disposition with matching finding ID `F1` and reports zero open advisory findings, even though the charter packet itself lacks a curation record.

A same-artifact cross-round version is also possible if finding IDs are reused across rounds: R1 curation for `F1` can be applied to R2/R3's latest RFP `F1` if it happens to be the latest curation record.

**Impact:**

- `anvil status` can undercount open advisory findings.
- `ConvergenceDeclaration.advisory_finding_count` can record an incorrect value.
- The P6 rule that advisory findings in the current round require explicit human disposition can be bypassed by unrelated curation records with matching local finding IDs.
- Artifact scoping implemented for R2/R3 is incomplete for advisory disposition accounting.

**Suggested fix:**

- When counting open advisories for a latest RFP, select a `CuratedFindingsRecord` whose `packet_id` exactly matches `rfp.packet.packet_id`.
- If multiple curation records exist for the same packet, use the latest matching one by `created_at` or index order.
- Do not use a global latest curation record for artifact-scoped advisory counts.
- Add regression tests with two RFPs that both contain `F1`, only one curated; assert the other still reports one open advisory finding.
- Add a second test with different artifacts to ensure `status --artifact charter.md` cannot be satisfied by a Plan/phase curation record.

---

## Overall Assessment

P6 R3 resolves all four R2 findings as written. The prompt-level Drop/Defer-Advisory enforcement now matches the gate, the new advisory-gate unit test covers the critical core behavior, the pre-R2 hash limitation is surfaced in status, and the arbiter error-path tests are meaningfully stronger.

I found one remaining medium-severity advisory-accounting issue: advisory counts still pair the latest artifact RFP with the globally latest curation record instead of the curation record for that packet. This should be fixed before treating P6 as fully converged because it affects the accuracy of `anvil status` and `ConvergenceDeclaration.advisory_finding_count`, both called out in the P6 acceptance criteria.
