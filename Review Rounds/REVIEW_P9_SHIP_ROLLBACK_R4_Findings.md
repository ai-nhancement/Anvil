# Anvil — P9 Ship + Rollback R4 Findings

**Source review doc:** `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R4.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Fail**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass** (159 tests)

The R4 test-count claim matches the current workspace output: 19 audit, 54 CLI, 49 core, 9 graph, 17 ship, 11 sidecar-client = 159 tests. Clippy is also clean.

However, the R4 document's fmt-clean claim is not accurate for the current tree.

---

## 1. High — `cargo fmt --all -- --check` fails, contradicting the R4 validation claim

**Location:**

- `crates/anvil-cli/src/phase.rs:667`
- `crates/anvil-cli/src/ship.rs:150`
- `crates/anvil-core/src/project.rs:139,158`
- `crates/anvil-ship/src/rollback.rs:334`
- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R4.md:7-8,113`

**Problem:**

R4 says fmt is clean:

```text
Clippy: Clean (`-D warnings`, all targets)
Fmt: Clean (`cargo fmt --all`)
```

But the actual formatter check fails. The diff includes multiple files. Representative examples:

```text
Diff in crates/anvil-cli/src/phase.rs:667:
-    let disposition =
-        PhaseDisposition::new(phase_id.to_owned(), DISPOSITION_SHIPPED.to_owned(), vec![cross_ref]);
+    let disposition = PhaseDisposition::new(
+        phase_id.to_owned(),
+        DISPOSITION_SHIPPED.to_owned(),
+        vec![cross_ref],
+    );
```

```text
Diff in crates/anvil-core/src/project.rs:139:
-        let expected_total =
-            BEFORE_AUDIT_STORE_DIRS.len() + AUDIT_RECORD_DIR_NAMES.len() + AFTER_AUDIT_STORE_DIRS.len();
-        assert_eq!(dirs.len(), expected_total, "layout_dirs() must have {expected_total} entries");
+        let expected_total = BEFORE_AUDIT_STORE_DIRS.len()
+            + AUDIT_RECORD_DIR_NAMES.len()
+            + AFTER_AUDIT_STORE_DIRS.len();
+        assert_eq!(
+            dirs.len(),
+            expected_total,
+            "layout_dirs() must have {expected_total} entries"
+        );
```

**Impact:**

- CI-equivalent validation would reject this round if fmt is enforced.
- The R4 review briefing overstates readiness.
- This is a release hygiene issue rather than a runtime correctness issue, but it is still a blocking validation mismatch.

**Suggested fix:**

- Run `cargo fmt --all` and commit the resulting formatting changes.
- Re-run `cargo fmt --all -- --check`, clippy, and tests before marking R4 complete.

---

## 2. Medium — Shipped-state docs still describe the obsolete gate-based authority

**Location:**

- `crates/anvil-ship/src/ship.rs:23-30`
- `crates/anvil-ship/src/ship.rs:41-45`
- `crates/anvil-ship/src/ship.rs:98-100`

**Problem:**

R4 correctly adds module-level documentation stating that `PhaseDisposition` is the sole shipped-state authority. But several nearby comments still describe the old gate-based logic.

`ShipReadiness::unshipped_phases` still says:

```rust
/// A phase is "not shipped" if it either has never had a `phase-{id}-ship` gate, or
/// its latest ship gate is older than its latest `RollbackEvent`.
```

`check_all_phases_shipped` still says:

```rust
/// A phase is "currently shipped" when it has a `phase-{id}-ship` `GateApproval` whose
/// `created_at` is strictly newer than the latest `RollbackEvent` that invalidated it
/// (or if no rollback event exists for the phase, any ship gate suffices).
```

And the timestamp comparison comment still refers to a rollback being written after the “ship gate it invalidates”:

```rust
// Strict greater-than is intentional: a RollbackEvent is always written after
// the ship gate it invalidates, so equality is impossible in a well-formed store.
```

After R3/R4, the code uses `PhaseDisposition.created_at`, not ship-gate `created_at`.

**Impact:**

- The implementation and local API docs disagree.
- Future maintainers can incorrectly reintroduce gate-based readiness by following the stale function docs instead of the module-level note.
- The timestamp invariant is less precise than it should be after the authority change; the relevant ordering is rollback event vs. shipped `PhaseDisposition`, not rollback event vs. gate.

**Suggested fix:**

- Update all function/field comments in `ship.rs` to consistently state that shipped state is determined by a `PhaseDisposition` with `disposition == DISPOSITION_SHIPPED` newer than the latest rollback.
- Change the comparison comment to reference the disposition/state record, not the gate.
- Consider adding a doc-test-style comment or inline assertion reference to `test_check_all_phases_shipped_gate_without_disposition_blocks`.

---

## 3. Medium — Layout-dir invariant is one-way; extra/non-record audit directories can still slip in unnoticed

**Location:**

- `crates/anvil-core/src/project.rs:24-40`
- `crates/anvil-audit/src/store.rs:453-466`
- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R4.md:37-49,100`

**Problem:**

R4 describes `AUDIT_RECORD_DIR_NAMES` as the “single source of truth” for record-type directory names, and the invariant test checks:

```rust
for rt in ALL_RECORD_TYPES {
    assert!(AUDIT_RECORD_DIR_NAMES.contains(&rt.dir_name()), ...);
}
```

This only verifies that every `RecordType` has a directory name in `AUDIT_RECORD_DIR_NAMES`. It does **not** verify the reverse: every name in `AUDIT_RECORD_DIR_NAMES` corresponds to a real `RecordType`.

`test_project_layout_directories` also does not catch extras, because its expected count is derived from `AUDIT_RECORD_DIR_NAMES.len()`:

```rust
let expected_total = BEFORE_AUDIT_STORE_DIRS.len()
    + AUDIT_RECORD_DIR_NAMES.len()
    + AFTER_AUDIT_STORE_DIRS.len();
assert_eq!(dirs.len(), expected_total, ...);
```

If someone accidentally adds `"obsolete-record-dir"` to `AUDIT_RECORD_DIR_NAMES`, both tests still pass:

- every real `RecordType` is still covered;
- the derived layout count increases consistently with the derived list;
- `anvil init` creates an extra audit-store subdirectory that has no record type.

This weakens the “single source of truth” claim. It prevents omissions, but not drift in the other direction.

**Impact:**

- The initialized filesystem layout can grow directories that no audit record will ever use.
- The project layout hinge test becomes less meaningful as an exact layout guard because the expected count is derived from the same list under test.
- The R4 question about ordering is less important than the missing bijection check.

**Suggested fix:**

- Strengthen `test_all_record_type_dirs_covered_by_layout_dirs` into a bijection check:
  - every `RecordType::dir_name()` is in `AUDIT_RECORD_DIR_NAMES`; and
  - every `AUDIT_RECORD_DIR_NAMES` entry maps back through `RecordType::from_dir_name`.
- Also assert `AUDIT_RECORD_DIR_NAMES.len() == ALL_RECORD_TYPES.len()`.
- In `test_project_layout_directories`, keep at least a fixed total count or fixed non-record ordering expectation that is not entirely derived from `layout_dirs()` inputs.

---

## 4. Medium / Low — `test_rollback_retry_advances_epoch_boundary` does not actually prove the epoch boundary advances

**Location:**

- `crates/anvil-ship/src/rollback.rs:327-373`
- `crates/anvil-ship/src/rollback.rs:106-111`
- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R4.md:25-35,104`

**Problem:**

R4 adds `test_rollback_retry_advances_epoch_boundary`, but the test itself acknowledges it does not pin the `max(created_at)` behavior:

```rust
// Note: this test verifies the safe-retry property and the resulting 0-offset. It
// does not directly pin the max(created_at) semantics of latest_rollback_at because
// pinning that requires RFPs created strictly between the two rollback timestamps,
// which would need a sleep.
```

The test performs two rollbacks and asserts:

- rollback-event count doubles; and
- `rotation_offset_for_phase` returns 0 because no RFPs exist.

That proves duplicate rollback records do not crash and that an empty current epoch has offset 0. It does **not** prove the newer rollback timestamp is the boundary. If `rotation_offset_for_phase` accidentally used the oldest rollback timestamp instead of the newest one, this test would still pass when no RFP exists between the two rollback attempts.

**Impact:**

- The test name and R4 disposition overstate what is pinned.
- A future regression from `max(created_at)` to `min(created_at)` could still pass this test.
- The conservative “more reviews post-retry” behavior remains documented but not executable as a regression guard.

**Suggested fix:**

- Rename the test to describe what it actually proves, such as `test_rollback_retry_appends_duplicate_records_without_breaking_offset`.
- Or add a stronger test that creates an RFP between two rollback records and confirms it is excluded after the second rollback.
- If avoiding sleeps, consider constructing audit records with controlled timestamps in a test-only helper, or exposing a narrow helper that accepts explicit `created_at` for deterministic tests.

---

## 5. Low — `PhaseDisposition.disposition` remains unconstrained despite the new constant

**Location:**

- `crates/anvil-audit/src/records.rs:6-11`
- `crates/anvil-audit/src/records.rs:596-605`
- `crates/anvil-cli/src/phase.rs:670-671`
- `crates/anvil-ship/src/ship.rs:128-130`

**Problem:**

R4 adds `DISPOSITION_SHIPPED`, which improves call-site consistency. However, `PhaseDisposition::new` still accepts an arbitrary `String`:

```rust
pub fn new(phase_id: String, disposition: String, cross_references: Vec<String>) -> Self
```

So the type still permits invalid state. The constant documents and centralizes the string value but does not enforce it.

This is acceptable for v1 if `PhaseDisposition` is intentionally an open-ended audit record, but the R4 wording says `DISPOSITION_SHIPPED` is “the only valid shipped-state value.” That is accurate for shipped-state queries, not for the `PhaseDisposition` field as a whole.

**Impact:**

- Low runtime risk because the construction site for phase ship now uses the constant.
- Future code can still write arbitrary disposition values without compiler help.
- The documentation could be misread as implying type-level enforcement that does not exist.

**Suggested fix:**

- Either clarify that `DISPOSITION_SHIPPED` is the only value recognized as shipped state, not the only value the field may contain, or introduce a typed disposition enum/newtype when additional states appear.
- If v1 should only ever write shipped dispositions, consider adding `PhaseDisposition::shipped(...)` and making general construction less prominent.

---

## Overall Assessment

R4 resolves the substantive R3 design questions: `PhaseDisposition` is clearly chosen as shipped-state authority, the layout refactor prevents missing record-type directories, and the retry behavior is documented with a duplicate-rollback regression test.

The primary blocker is validation hygiene: fmt is not clean despite the R4 claim. After running `cargo fmt --all`, the remaining findings are mostly documentation/test-contract improvements rather than runtime correctness blockers.
