# P9 Ship + Rollback — Review Briefing (R3)

**Date:** 2026-05-26
**Scope:** R2 findings resolution — plan-consolidation layout fix, PhaseDisposition as shipped authority, empty-reason validation, partial-rollback recovery message, AC8 v1 scope documentation
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P9 — Ship + Rollback (Cascading Invalidation)
**Tests:** 158 passing (19 audit, 54 cli, 49 core, 9 graph, 16 ship, 11 sidecar-client), 0 failed
**Clippy:** Clean (`-D warnings`, all targets)
**Fmt:** Clean (`cargo fmt --all`)

---

## R2 Findings Disposition

### F1 (High) — Initialized projects missing `plan-consolidation` audit directory
**Status: Resolved**

Added `"audit-store/plan-consolidation"` to `LAYOUT_DIRS` in `crates/anvil-core/src/project.rs`. The hinge test `test_project_layout_directories` is updated to pin 20 entries (was 19).

Two new tests in `crates/anvil-audit/src/store.rs`:

- **`test_all_record_type_dirs_covered_by_layout_dirs`** — invariant test: for every `RecordType` in `ALL_RECORD_TYPES`, asserts that `"audit-store/{dir_name}"` exists in `LAYOUT_DIRS`. Prevents a future omission from going undetected.
- **`test_plan_consolidation_record_appended_after_project_init`** — regression test: calls `project::init` (not `init_test_store` which creates dirs manually), opens the store, appends a `PlanConsolidationRecord`, and asserts it succeeds. Passes now that the directory is in `LAYOUT_DIRS`.

### F2 (High/Medium) — `check_all_phases_shipped` used `GateApproval` as authority; partial ship-failure gap
**Status: Resolved**

`is_phase_currently_shipped` now uses `PhaseDisposition` (with `disposition == "shipped"`) as the authoritative shipped-state record instead of the `phase-{id}-ship` `GateApproval`. The old `latest_gate_at` helper is replaced by `latest_shipped_disposition_at`, which queries `RecordType::PhaseDisposition` filtered by `phase_id` and `disposition == "shipped"`.

This eliminates the partial-failure gap: if `run_phase_ship` succeeds writing the gate but fails writing the disposition, `check_all_phases_shipped` correctly treats the phase as unshipped.

Updated tests (3 files):
- `anvil-ship/src/ship.rs` — `test_check_all_phases_shipped_all_shipped` and `test_check_all_phases_shipped_after_rollback_blocks` now create `PhaseDisposition` records alongside the gate.
- `anvil-cli/src/ship.rs` — `test_project_ship_succeeds_when_all_shipped` likewise creates both.
- New regression: `test_check_all_phases_shipped_gate_without_disposition_blocks` creates a `phase-{id}-ship` gate without a disposition and asserts the phase is still unshipped.

The rollback timestamp comparison (`rollback_at > ship_at`) is unchanged; `ship_at` is now the disposition's `created_at` rather than the gate's.

### F3 (Medium) — Empty/whitespace `--reason` accepted by `phase reopen`
**Status: Resolved**

`run_phase_reopen` now validates `reason` before any IO:

```rust
if reason.trim().is_empty() {
    return Err(AnvilError::EmptyReasoning { command: "phase reopen --reason" });
}
```

Two new tests in `crates/anvil-cli/src/phase.rs`:
- `test_phase_reopen_empty_reason_rejected`
- `test_phase_reopen_whitespace_reason_rejected`

Both confirm `EmptyReasoning` is returned without requiring a project to be initialized (the check runs before `AuditStore::open`).

### F4 (Medium) — Partial rollback write left no recovery guidance
**Status: Resolved**

`execute_rollback` failure in `run_phase_reopen` is now wrapped with retry instructions:

```rust
anvil_ship::execute_rollback(&plan, &store, reason).map_err(|e| {
    AnvilError::Io(std::io::Error::other(format!(
        "rollback write failed — partial invalidation may exist; \
         re-run `anvil phase reopen {phase_id} --reason <reason>` to complete: {e}"
    )))
})?;
```

The error printed to stderr instructs the user to re-run the same command. Duplicate `RollbackEvent` records from a retry are harmless — `is_phase_currently_shipped` uses `max(created_at)` on disposition records and `rotation_offset_for_phase` counts RFPs after the latest rollback timestamp, so extra rollback records only advance the epoch boundary.

A preflight-availability check before the append loop was considered but not added: `project::init` now creates all record-type directories (F1), so the only remaining failure modes are disk-full or permission errors that `AuditStore::append` cannot prevent regardless.

### F5 (Medium/Low) — AC8 "triggered by re-open" implemented as print-only
**Status: Accepted (documented as v1 behavior)**

Added a code comment in `run_phase_reopen` explicitly scoping AC8:

```rust
// AC8 (v1): Amendment triage is a human judgment call. The CLI surfaces the decision
// point; enforcing it via a gate or audit record is out of scope for v1.
```

The reviewer's option of implementing a lightweight `AmendmentTriageRecord` is deferred. The print-only behavior is correct for v1 given that amendment necessity is context-dependent and the coordinator commands to amend (`anvil charter review`, `anvil plan invoke`) already exist.

### F6 (Low) — R2 test count was stale
**Status: Resolved by this document**

R3 reports 158 total tests, matching the current workspace output.

---

## P9 Acceptance Criteria (post-R3)

| Criterion | Status |
|---|---|
| AC1: `anvil ship` succeeds only when all phases shipped; exits non-zero with named list | ✓ Now uses `PhaseDisposition` as authority; combined error names "never shipped" and "rolled back without re-ship" |
| AC2: `anvil ship` executes configured transport actions in order; failure is typed error | ✓ Unchanged |
| AC3: `anvil phase reopen <id>` shows full blast radius before committing | ✓ Unchanged |
| AC4: User must explicitly confirm blast radius (or pass `--yes` for CI) | ✓ Unchanged |
| AC5: Re-opening creates `RollbackEvent` records for re-opened phase and all dependents | ✓ Partial-write failure now surfaces retry instructions |
| AC6: `anvil ship` blocked if any `RollbackEvent` lacks re-shipped resolution | ✓ Now unreachable path eliminated (both checks always run) |
| AC7: Audit store records remain immutable through rollback | ✓ Unchanged; pinned by hinge test |
| AC8: Charter/Plan amendment workflow triggered by re-open | ✓ v1: Coordinator instruction displayed; gate enforcement deferred (documented) |
| Plan cross-cutting: non-interactive `--yes` path | ✓ Unchanged from R2 |

---

## Hinge Tests (unchanged)

| Test | Location | What It Pins |
|---|---|---|
| `test_rollback_transitive_invalidation` | `rollback.rs` | One `RollbackEvent` per phase in transitive closure; `rotation_reset_phases` identical across siblings |
| `test_audit_store_immutable_through_rollback` | `ship.rs` (anvil-ship) | `execute_rollback` never modifies/deletes; only appends new `RollbackEvent` records |
| `test_rollback_resets_rotation_on_dependents` | `rollback.rs` | `rotation_offset_for_phase` returns 0 post-rollback; `rotation_reset_phases` includes all invalidated phases |
| `test_project_layout_directories` | `project.rs` | Layout has exactly 20 entries including `audit-store/plan-consolidation` |

---

## What to Review

1. **`PhaseDisposition` as sole authority vs. requiring both.** The fix makes `PhaseDisposition` the sole shipped-state signal and drops the `GateApproval` check entirely. The gate is still written by `run_phase_ship` and is still used for preflight checks in that command. Is removing the gate from the ship readiness check correct, or should readiness require both gate AND disposition to be present?

2. **Retry semantics under duplicate `RollbackEvent`.** A re-run of `anvil phase reopen` after a partial write creates a second set of `RollbackEvent` records with a newer `created_at`. `rotation_offset_for_phase` uses `max(rollback_at)` so the epoch boundary advances to the second run — any RFPs between run 1 and run 2 are excluded from the offset count. This is conservative (requires more reviews post-retry) but correct. Confirm this is acceptable.

3. **`test_all_record_type_dirs_covered_by_layout_dirs` as a contract.** This test now enforces that every `RecordType` has a layout dir. Adding a new `RecordType` without updating `LAYOUT_DIRS` will fail this test. Confirm this is the intended coupling.

---

## Test Coverage Summary (R3 additions)

**`anvil-audit/src/store.rs`** (2 new tests):
- `test_all_record_type_dirs_covered_by_layout_dirs` — layout/record-type invariant
- `test_plan_consolidation_record_appended_after_project_init` — regression for plan-consolidation gap

**`anvil-ship/src/ship.rs`** (1 new test):
- `test_check_all_phases_shipped_gate_without_disposition_blocks` — F2 regression

**`anvil-cli/src/phase.rs`** (2 new tests):
- `test_phase_reopen_empty_reason_rejected` — F3
- `test_phase_reopen_whitespace_reason_rejected` — F3

**Total: 158 tests passing, 0 failed, clippy clean, fmt clean.**
