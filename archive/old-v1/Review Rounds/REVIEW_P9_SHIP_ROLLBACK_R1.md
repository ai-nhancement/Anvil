# P9 Ship + Rollback — Review Briefing (R1)

**Date:** 2026-05-26
**Scope:** Project-level ship gate, cascading rollback with transitive invalidation, rotation reset, configurable transport actions
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P9 — Ship + Rollback (Cascading Invalidation)
**Tests:** 152 passing (17 new in `anvil-ship`, 2 new in `anvil-cli/src/ship.rs`), 0 failed
**Status:** Additive — existing P8 phase-ship gate and `run_phase_review` behaviour unchanged for projects without rollbacks; rotation-reset path activates only when `RollbackEvent` records exist

---

## What Was Built

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-ship/src/rollback.rs` | Created | `RollbackPlan`, `compute_rollback_plan`, `execute_rollback`, `rotation_offset_for_phase` |
| `crates/anvil-ship/src/ship.rs` | Created | `ShipReadiness`, `check_all_phases_shipped`, `check_unresolved_rollbacks` |
| `crates/anvil-ship/src/transport.rs` | Created | `TransportAction` re-export, `parse_transport_actions`, `execute_transport` |
| `crates/anvil-cli/src/ship.rs` | Created | `run_project_ship` — `anvil ship` command implementation |
| `crates/anvil-core/src/config.rs` | Modified | Added `TransportKind`, `TransportAction`, `transport_actions: Vec<TransportAction>` to `AnvilConfig` |
| `crates/anvil-cli/Cargo.toml` | Modified | Added `anvil-ship` dependency |
| `crates/anvil-cli/src/main.rs` | Modified | Added `Command::Ship`, `PhaseCmd::Reopen`, module `ship`, dispatch for both |
| `crates/anvil-cli/src/phase.rs` | Modified | Added `run_phase_reopen`; updated `run_phase_review` to use epoch-based `rotation_round` from `anvil_ship::rotation_offset_for_phase` |

---

## Architecture Decisions

### Timestamp-based shipped/rolled-back detection
`is_phase_currently_shipped` compares `created_at` timestamps of the latest ship gate vs. the latest `RollbackEvent`. This is correct under the append-only constraint: we cannot mark records invalid, so we use recency ordering. The `DateTime<Utc>` fields are sub-second precision (chrono), so even rapid test sequences are correctly ordered.

### Epoch-based rotation in `run_phase_review`
`rotation_offset_for_phase` counts RFPs created **after** the latest `RollbackEvent` for the phase, returning 0 for a just-rolled-back phase. This makes `rotation_select(&pool, rotation_round)` select pool[0] again — the full pool reviews the fix from the start. The pre-P9 behaviour (no rollback → count all RFPs) is preserved as the base case.

Two "round" concepts now coexist in `run_phase_review`:
- `round_number` — global total RFP count + 1, used for briefing file path (never resets)
- `rotation_round` — epoch-based count + 1, used for reviewer selection and `RotationLog.round_number`

### `TransportAction` defined in `anvil-core`, re-exported from `anvil-ship`
`AnvilConfig` (in `anvil-core`) needs to store transport actions. Placing `TransportAction` in `anvil-ship` would create a circular dependency. So it lives in `anvil-core::config` and `anvil-ship::transport` re-exports it (`pub use anvil_core::config::TransportAction`). The `lib.rs` public API (`pub use transport::{..., TransportAction}`) is preserved.

### No Charter/Plan amendment automation
AC8 ("Charter/Plan amendment workflow triggered by re-open") is surfaced as a user-facing instruction in `run_phase_reopen`'s output ("Amend Charter or Plan if the root cause requires it"). The amendment commands (`anvil charter review`, `anvil plan invoke`, etc.) already exist; `phase reopen` just gates the re-build path and leaves the amendment decision to the Coordinator. Automating it would be premature for v1.

---

## P9 Acceptance Criteria

| Criterion | Status |
|---|---|
| AC1: `anvil ship` succeeds only when all phases shipped; exits non-zero with named list | ✓ `check_all_phases_shipped` + `ProjectShipBlocked` error with phase list |
| AC2: `anvil ship` executes configured transport actions in order; failure is typed error | ✓ `execute_transport` with `TransportFailed` on non-zero exit |
| AC3: `anvil phase reopen <id>` shows full blast radius before committing | ✓ `compute_rollback_plan` + display loop in `run_phase_reopen` |
| AC4: User must explicitly confirm blast radius | ✓ `dialoguer::Confirm` with `default(false)` |
| AC5: Re-opening creates `RollbackEvent` records for re-opened phase and all dependents | ✓ `execute_rollback` writes one record per `all_reset_phases` entry |
| AC6: `anvil ship` blocked if any `RollbackEvent` lacks re-shipped resolution | ✓ `check_unresolved_rollbacks` checked before transport |
| AC7: Audit store records remain immutable through rollback | ✓ `execute_rollback` only appends; pinned by `test_audit_store_immutable_through_rollback` |
| AC8: Charter/Plan amendment workflow triggered by re-open | ✓ Surfaced as user instruction in `run_phase_reopen` output |

## Hinge Tests

| Test | Location | What It Pins |
|---|---|---|
| `test_rollback_transitive_invalidation` | `rollback.rs` | One `RollbackEvent` per phase in transitive closure; `rotation_reset_phases` identical across siblings |
| `test_audit_store_immutable_through_rollback` | `ship.rs` | `execute_rollback` never modifies/deletes; only appends new `RollbackEvent` records |
| `test_rollback_resets_rotation_on_dependents` | `rollback.rs` | `rotation_offset_for_phase` returns 0 post-rollback; `rotation_reset_phases` includes all invalidated phases |

---

## What to Review

1. **Timestamp comparison correctness.** `is_phase_currently_shipped` uses `rollback_at > ship_at` (strictly greater) to detect a post-ship rollback. Is this the right comparison direction? Could a ship and rollback in the same millisecond cause incorrect ordering? (In practice this can't happen in a real workflow, but worth confirming the direction is defensible.)

2. **Rotation reset scope.** After `anvil phase reopen P1`, `rotation_offset_for_phase("P2")` returns 0 only if no RFPs for P2 exist after the rollback event timestamp. If the user somehow ran `anvil phase review P2` between the rollback events being written, would the offset be incorrectly non-zero? The current implementation counts all P2 RFPs created after the rollback event — this seems correct, but review the ordering guarantee.

3. **`check_unresolved_rollbacks` vs. `check_all_phases_shipped` overlap.** The CLI calls both, but `check_unresolved_rollbacks` is a strict subset of what `check_all_phases_shipped` catches (phases that have rollback events but no subsequent ship). Is calling both redundant, or is the distinction valuable for error messaging? Currently both are called sequentially; if `check_all_phases_shipped` already blocks, `check_unresolved_rollbacks` is never surfaced. Consider whether the order should be reversed or if they should be merged.

4. **Transport action shell compatibility.** `execute_transport` uses `cmd /C` on Windows, `sh -c` on non-Windows. The test uses `cd .` as a portable no-op — this works on both platforms. Confirm the `cmd /C` path handles commands with quoted arguments correctly (e.g., `git commit -m "message with spaces"`).

5. **`run_phase_reopen` non-interactive path.** The plan's cross-cutting concern requires every gate to support a non-interactive `--yes` path. `run_phase_reopen` uses `dialoguer::Confirm` which blocks on a terminal prompt. There is no `--yes`/`--reason` flag on `phase reopen` in this R1. Is this a High or Medium gap for v1?

6. **`plan-consolidation` directory missing from `LAYOUT_DIRS`.** Noticed while reading `project.rs`: `PlanConsolidation` records (added in P7) have a `RecordType` entry and a subdir (`plan-consolidation`) but `LAYOUT_DIRS` in `project.rs` does not include that directory. This predates P9, but was noticed during this review pass. Is this a known gap?

---

## Test Coverage Summary

**`anvil-ship`** (14 new tests across 3 modules):
- `rollback::test_rollback_transitive_invalidation` — hinge: cascade + sibling records
- `rollback::test_rollback_resets_rotation_on_dependents` — hinge: rotation offset post-rollback
- `rollback::test_compute_rollback_plan_unknown_phase` — error path
- `rollback::test_rotation_offset_for_phase_no_rollback` — baseline (pre-P9) behaviour preserved
- `ship::test_audit_store_immutable_through_rollback` — hinge: append-only
- `ship::test_check_all_phases_shipped_empty_store` — all unshipped
- `ship::test_check_all_phases_shipped_all_shipped` — happy path
- `ship::test_check_all_phases_shipped_after_rollback_blocks` — post-rollback detection
- `ship::test_check_unresolved_rollbacks_empty` — no rollbacks case
- `transport::test_parse_transport_actions_empty` — default config
- `transport::test_parse_transport_actions_returns_all` — list extraction
- `transport::test_execute_transport_empty_succeeds` — no-op path
- `transport::test_execute_transport_failing_command_errors` — `TransportFailed` path
- `transport::test_execute_transport_succeeding_command_ok` — happy path

**`anvil-cli/src/ship.rs`** (2 new tests):
- `test_project_ship_blocked_when_phases_not_shipped` — AC1
- `test_project_ship_succeeds_when_all_shipped` — AC1 + AC2 (empty transport)
