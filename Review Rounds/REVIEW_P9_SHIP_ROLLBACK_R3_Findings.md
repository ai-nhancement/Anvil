# Anvil — P9 Ship + Rollback R3 Findings

**Source review doc:** `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R3.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo build --workspace` — **passes**
- `cargo test --workspace` — **passes** (158 tests)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **passes**
- `cargo fmt --check` — **passes**
- `go test ./...` from `C:\Anvil\sidecar` — **passes** (unchanged by P9)

---

## 1. High — PhaseDisposition sole authority removes gate from readiness; requires explicit confirmation of intent

**Location:**

- `crates/anvil-ship/src/ship.rs:82` (`is_phase_currently_shipped`)
- `crates/anvil-ship/src/ship.rs:101` (`latest_shipped_disposition_at`)
- `crates/anvil-cli/src/ship.rs:154` and `crates/anvil-cli/src/phase.rs:671` (both gate + disposition still written)
- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R3.md` §"What to Review" item 1

**Problem:**

R3 makes `PhaseDisposition` (disposition == "shipped") the *only* signal used by `check_all_phases_shipped` / `is_phase_currently_shipped`. The `phase-{id}-ship` `GateApproval` continues to be appended in `run_phase_ship` but is completely ignored for the shipped-state decision and for the unresolved-rollback preflight.

The review document explicitly asks whether readiness should require *both* records. Current implementation answers "no — disposition alone is authoritative."

**Impact:**

- If the original design intent was that a successful ship must produce a durable gate *and* disposition (two independent appends), the R3 change weakens the check.
- Future code that inspects only `GateApproval` records for "shipped" history will see a different truth than the readiness logic.
- The regression test `test_check_all_phases_shipped_gate_without_disposition_blocks` proves the new behavior, but does not prove the design choice was reviewed against the original AC1 wording.

**Suggested fix / improvement:**

- Add an explicit design note in `ANVIL_PLAN.md` §P9 and in `anvil-ship/src/ship.rs` module docs stating: "GateApproval records are retained for historical audit only; `PhaseDisposition` is the sole shipped-state authority after R3."
- If both are required, restore a conjunction in `is_phase_currently_shipped`; otherwise close the question as "sole disposition accepted."

---

## 2. Medium — Duplicate `RollbackEvent` on retry advances rotation epoch boundary without dedicated regression test

**Location:**

- `crates/anvil-ship/src/rollback.rs:76` (`execute_rollback`)
- `crates/anvil-ship/src/ship.rs:86` (`is_phase_currently_shipped` uses `latest_rollback_at`)
- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R3.md` §"What to Review" item 2

**Problem:**

The retry path documented in F4 is safe (duplicates are harmless because `max(created_at)` is used), yet no test asserts that after a second `execute_rollback` on the same plan the `rotation_offset_for_phase` still returns 0 using the *newer* rollback timestamp.

Existing hinge tests cover single rollback only.

**Impact:**

- The conservative "more reviews required" behavior after retry is correct but untested at the boundary.
- A future change to `latest_rollback_at` that used `min` or a different aggregation would silently alter retry semantics.

**Suggested fix / improvement:**

- Add a regression test (name it `test_rollback_retry_advances_epoch_boundary`) that performs two `execute_rollback` calls and asserts the second set of records produces a strictly later `created_at` used by rotation logic.
- Document the max-timestamp rule in `rotation_offset_for_phase` docs.

---

## 3. Medium — Invariant test `test_all_record_type_dirs_covered_by_layout_dirs` is a strong contract but couples two modules without shared source of truth

**Location:**

- `crates/anvil-audit/src/store.rs:454`
- `crates/anvil-core/src/project.rs:8` (`LAYOUT_DIRS`)
- `crates/anvil-audit/src/records.rs:124` (`ALL_RECORD_TYPES`)

**Problem:**

The new test is excellent and directly prevents recurrence of the plan-consolidation omission (F1). However, `LAYOUT_DIRS` and `ALL_RECORD_TYPES` + `dir_name()` remain two independent definitions. Adding a `RecordType` variant without a matching layout entry will now fail the test, which is the desired outcome, but the duplication of the 15 directory names is still present.

**Impact:**

- Maintenance burden when extending the record system.
- The hinge test `test_project_layout_directories` also duplicates the list.

**Suggested fix / improvement:**

- Consider deriving the audit-store subset of `LAYOUT_DIRS` from `ALL_RECORD_TYPES` at compile time (or via a const evaluation helper) so the single source of truth is `RecordType`.
- Keep the explicit `LAYOUT_DIRS` array only for the non-record directories (phases, .anvil/*).

---

## 4. Low — `latest_shipped_disposition_at` (and sibling helpers) perform full-list deserialize on every call; no phase_id index

**Location:**

- `crates/anvil-ship/src/ship.rs:105` (`latest_shipped_disposition_at`)
- `crates/anvil-ship/src/ship.rs:124` (`latest_rollback_at`)
- Similar patterns in `anvil-cli/src/phase.rs:57` (gate lookup)

**Problem:**

Every readiness or rotation query walks the entire record list for the type, deserializes every entry, then filters client-side. With current test sizes this is negligible, but the pattern repeats across ship/phase commands.

**Impact:**

- Minor performance cliff once real projects accumulate hundreds of PhaseDisposition / RollbackEvent records.
- No use of the existing `_index.json` beyond id→path mapping.

**Suggested fix / improvement:**

- Note as future optimization: add a lightweight secondary index (phase_id → latest disposition timestamp) or expose a filtered list API on `AuditStore`.
- Not required for P9; acceptable for v1 scale.

---

## 5. Low — `PhaseDisposition.disposition` is an unconstrained string; only "shipped" is ever written

**Location:**

- `crates/anvil-audit/src/records.rs:310`
- `crates/anvil-ship/src/ship.rs:113` (hard-coded filter)

**Problem:**

The field accepts any string at construction and persistence time. The shipped-state check hard-codes `"shipped"`. Future dispositions (e.g. "rolled-back", "superseded") would require coordinated string changes.

**Impact:**

- Low risk for v1, but weak typing.

**Suggested fix / improvement:**

- Add a module-level constant `pub const DISPOSITION_SHIPPED: &str = "shipped";` and use it in both construction sites and the filter. This documents the only valid value today and makes future extension mechanical.

---

## Summary of R3 Code Health

- All R2 findings resolved as documented.
- New invariant test and regression coverage are high quality.
- The three open questions in "What to Review" are addressed above; sole-authority disposition is the correct engineering choice given the partial-failure goal, provided it is explicitly documented.
- No new correctness, security, or immutability violations introduced.
- Test/clippy/fmt gates remain clean.