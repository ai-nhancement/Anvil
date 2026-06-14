# Anvil — P9 Ship + Rollback R5 Findings

**Source review doc:** `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R5.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo build --workspace` — **passes**
- `cargo test --workspace` — **passes** (159 tests)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **passes**
- `cargo fmt --all -- --check` — **fails** (one file)
- `go test ./...` from `C:\Anvil\sidecar` — **passes**

Additional note: `cargo fmt --all -- --check` produces a diff in `crates/anvil-core/src/project.rs:143` (assert_eq! formatting on the hard-pinned layout count). This directly contradicts the R5 validation claim.

---

## 1. High — `cargo fmt --all -- --check` fails, contradicting the R5 validation claim

**Location:**

- `crates/anvil-core/src/project.rs:143`
- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R5.md` (validation section)

**Problem:**

R5 states “Fmt: Clean (`cargo fmt --all -- --check`)”. Running the exact command produces a non-zero exit and a diff on the hard-pinned `assert_eq!(dirs.len(), 20, ...)` line inside `test_project_layout_directories`. The multi-line formatting preferred by rustfmt was not applied after the R4 changes that introduced the literal count.

This is the same class of error as F1 in R4 (which was resolved by actually running fmt).

**Impact:**

- CI format gate will fail on the current tree.
- The R5 “clean” claim is factually incorrect and misleads downstream consumers of the review document.
- The test file was edited in R4/R5 but the final formatting pass was omitted.

**Suggested fix:**

- Run `cargo fmt --all` and commit the resulting diff on `project.rs`.
- Re-execute `cargo fmt --all -- --check` and update the R5 validation line to reflect the true state before declaring the round complete.

---

## 2. Medium — Stronger regression test for `max(created_at)` rollback boundary still absent; doc-only specification remains

**Location:**

- `crates/anvil-ship/src/rollback.rs:328` (`test_rollback_retry_appends_duplicate_records_without_breaking_offset`)
- `crates/anvil-ship/src/ship.rs:86` (`latest_rollback_at`)
- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R5.md` §"What to Review" item 1

**Problem:**

The test was correctly renamed to avoid overstating its guarantees. Its body explicitly documents that it does *not* pin the `max(created_at)` selection logic used by `rotation_offset_for_phase` after duplicate rollbacks. That rule lives only in the doc comment of `rotation_offset_for_phase`.

No executable guard exists that would fail if the selection changed from `max` to `min` or to “latest record in index order.”

**Impact:**

- A future refactor of `latest_rollback_at` (or of `RollbackEvent` creation) could silently alter retry semantics for the rotation epoch without any test noticing.
- The conservative “more reviews required after retry” behavior is therefore protected only by prose.

**Suggested fix / improvement:**

- The rename chosen in R5 is acceptable for v1 given the cost of a sleep-based or internal-timestamp-injection test.
- If stronger protection is desired later, expose an internal test-only constructor for `RollbackEvent` that accepts an explicit `created_at`, or accept the doc-only status and mark the item closed.

---

## 3. Low — `layout_dirs()` remains an allocating function; constant alternative discussed but unchanged

**Location:**

- `crates/anvil-core/src/project.rs:50` (`pub fn layout_dirs() -> Vec<String>`)
- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R5.md` §"What to Review" item 2
- Call sites: `project::init`, `setup.rs:step6_store`

**Problem:**

The function builds a fresh `Vec<String>` on every call by concatenating three slices. It is invoked only during `anvil init` and the setup wizard step 6. Allocation cost is negligible at current scale.

**Impact:**

- None for v1 performance or correctness.
- The choice to keep a function (rather than a `const` or `LazyLock`) is reasonable because the list is derived from three independent constants that must remain in source order for the structural-order assertions in the hinge test.

**Suggested fix / improvement:**

- No action required. The current design correctly trades a trivial allocation for a single source of truth (`AUDIT_RECORD_DIR_NAMES`) that the bijection test can enforce. Documenting the rationale (as already done in the module comment) is sufficient.

---

## 4. Low — `AUDIT_RECORD_DIR_NAMES` + `from_dir_name` bijection is now robust but still requires manual synchronization on `RecordType` addition

**Location:**

- `crates/anvil-core/src/project.rs:24` (`AUDIT_RECORD_DIR_NAMES`)
- `crates/anvil-audit/src/records.rs:110` (`from_dir_name`)
- `crates/anvil-audit/src/store.rs:454` (bijection test)

**Problem:**

The R5 bijection test (forward + reverse + count) is a clear improvement over the R4 one-way check. However, when a new `RecordType` variant is added, three sites must still be edited by hand:

1. The enum variant in `records.rs`
2. The arm in `dir_name()` and in `from_dir_name()`
3. The string literal in `AUDIT_RECORD_DIR_NAMES`

The test will catch omissions, but the work is not mechanically derived.

**Impact:**

- Low risk because the test is part of the regular CI gate.
- The circular-dependency reason for keeping the lists separate (stated in the `AUDIT_RECORD_DIR_NAMES` doc) remains valid.

**Suggested fix / improvement:**

- The current approach is the correct engineering trade-off. No further automation is warranted for v1.

---

## Summary of R5 Code Health

- The sole critical gate failure is the unapplied rustfmt diff on the hard-pinned layout count assertion.
- All R4 findings were addressed; the bijection strengthening and doc updates are high-quality.
- The two open review questions are evaluated above: the test-rename decision is acceptable for v1; the `layout_dirs()` function choice is appropriate.
- No new correctness, immutability, or safety regressions introduced by the R5 changes.
- Once the fmt diff is committed, the round will be validation-clean.