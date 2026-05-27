# P9 Ship + Rollback — Review Briefing (R4)

**Date:** 2026-05-26
**Scope:** R3 findings resolution — PhaseDisposition authority documented, layout-dir single-source-of-truth refactor, rollback-retry test, DISPOSITION_SHIPPED constant, performance note
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P9 — Ship + Rollback (Cascading Invalidation)
**Tests:** 159 passing (19 audit, 54 cli, 49 core, 9 graph, 17 ship, 11 sidecar-client), 0 failed
**Clippy:** Clean (`-D warnings`, all targets)
**Fmt:** Clean (`cargo fmt --all`)

---

## R3 Findings Disposition

### F1 (High) — PhaseDisposition authority not explicitly documented
**Status: Resolved**

Two documentation sites added:

1. **Module-level docs in `crates/anvil-ship/src/ship.rs`** — explains why `PhaseDisposition` is the sole shipped-state authority, why `GateApproval` is retained as audit provenance only, and references the partial-failure rationale.

2. **`Anvil Plan/ANVIL_PLAN.md` §P9** — added an "Implementation note (P9 R3)" block before AC1 documenting the design decision and the reasoning (disposition is written after the gate; gate-only check falsely reports shipped after a partial failure).

The design choice is now auditable from both the code and the plan document.

### F2 (Medium) — Rollback-retry semantics lacked a regression test
**Status: Resolved**

Added `test_rollback_retry_advances_epoch_boundary` in `crates/anvil-ship/src/rollback.rs`:
- Calls `execute_rollback` twice on the same plan (simulating a retry after partial write).
- Asserts the second call appends a second set of records (store is append-only — count doubles).
- Asserts `rotation_offset_for_phase` returns 0 for all affected phases after both rollbacks.

Also added to the `rotation_offset_for_phase` doc comment: explicit documentation of the `max(created_at)` rule and the conservative "more reviews on retry" behavior.

The test is annotated with a note explaining that it does not directly pin the `max` vs. `min` aggregation (that would require RFPs created strictly between two rollback timestamps, which needs a sleep). The doc comment carries that specification.

### F3 (Medium) — `LAYOUT_DIRS` and `ALL_RECORD_TYPES` duplicated; no single source of truth
**Status: Resolved**

`LAYOUT_DIRS` (static const) is removed. `crates/anvil-core/src/project.rs` now exposes:

- **`AUDIT_RECORD_DIR_NAMES: &[&str]`** (pub) — the 15 record-type dir names without the `"audit-store/"` prefix. This is the single source of truth for record-type directory names.
- **`layout_dirs() -> Vec<String>`** (pub) — derives the full 20-entry list by concatenating two small private constants (`BEFORE_AUDIT_STORE_DIRS`, `AFTER_AUDIT_STORE_DIRS`) with prefixed `AUDIT_RECORD_DIR_NAMES` entries.

`project::init` now iterates `layout_dirs()` instead of the former static array.

The invariant test in `crates/anvil-audit/src/store.rs` (`test_all_record_type_dirs_covered_by_layout_dirs`) is updated to check `AUDIT_RECORD_DIR_NAMES` against `ALL_RECORD_TYPES::dir_name()` — this is the cross-crate coupling point that enforces consistency.

The hinge test `test_project_layout_directories` now tests `layout_dirs()` output for count (20), structural ordering, and membership — without repeating the full 15-entry list (the coupling to record types is handled by the invariant test).

Two additional call sites updated: `step6_store` in `setup.rs` and `test_workspace_runtime_dir_in_layout`.

### F4 (Low) — No performance note on full-scan helpers
**Status: Resolved**

Added a "Performance note" paragraph to the `latest_shipped_disposition_at` doc comment:
> Acceptable at v1 scale; a secondary index (`phase_id` → latest disposition timestamp) would eliminate the linear scan.

### F5 (Low) — `PhaseDisposition.disposition` is an unconstrained string
**Status: Resolved**

Added `pub const DISPOSITION_SHIPPED: &str = "shipped";` to `crates/anvil-audit/src/records.rs` with a doc comment explaining it is the only valid shipped-state value. Used in:
- `latest_shipped_disposition_at` in `anvil-ship/src/ship.rs` (the filter site)
- `run_phase_ship` in `anvil-cli/src/phase.rs` (the construction site)

Any future disposition string (e.g., `"superseded"`) will require its own named constant, making future extension mechanical.

---

## P9 Acceptance Criteria (post-R4, unchanged)

| Criterion | Status |
|---|---|
| AC1: `anvil ship` succeeds only when all phases shipped | ✓ `PhaseDisposition` authority documented in module docs and ANVIL_PLAN.md |
| AC2: Transport actions execute in order; typed failure | ✓ Unchanged |
| AC3: Blast radius shown before commit | ✓ Unchanged |
| AC4: Explicit confirmation / `--yes` CI path | ✓ Unchanged |
| AC5: One `RollbackEvent` per invalidated phase | ✓ Retry safety pinned by `test_rollback_retry_advances_epoch_boundary` |
| AC6: Ship blocked on unresolved rollbacks | ✓ Unchanged |
| AC7: Audit store immutable through rollback | ✓ Unchanged; hinge test covers retry path too |
| AC8: Charter/Plan amendment triggered (v1: instruction only) | ✓ Unchanged |
| Plan cross-cutting: `--yes` non-interactive path | ✓ Unchanged |

---

## Hinge Tests (unchanged, one new)

| Test | Location | What It Pins |
|---|---|---|
| `test_rollback_transitive_invalidation` | `rollback.rs` | Cascade + sibling records |
| `test_audit_store_immutable_through_rollback` | `ship.rs` (anvil-ship) | Append-only invariant |
| `test_rollback_resets_rotation_on_dependents` | `rollback.rs` | Rotation offset 0 post-rollback |
| `test_project_layout_directories` | `project.rs` | 20-entry layout, structural order |
| `test_rollback_retry_advances_epoch_boundary` | `rollback.rs` | Retry is safe; offset 0 after duplicate rollback |

---

## What to Review

1. **`AUDIT_RECORD_DIR_NAMES` ordering.** The 15 names in `AUDIT_RECORD_DIR_NAMES` follow the same order as `ALL_RECORD_TYPES` in `records.rs` (declaration order). Is this ordering load-bearing anywhere, or is it purely cosmetic? Currently the invariant test uses `contains`, not positional checks, so order doesn't matter for correctness.

2. **`layout_dirs()` is a function, not a constant.** Every call allocates a `Vec<String>`. This is called from `project::init` (once per project) and from `step6_store` in setup (once per wizard step preview). Not performance-sensitive at this scale; flagging in case there is a preference for a `lazy_static!` or `OnceLock` version.

3. **`test_rollback_retry_advances_epoch_boundary` does not test `max` vs `min` aggregation.** The test note documents this gap. Is a sleep-based test needed to close it, or is the doc comment specification sufficient for v1?

---

## Test Coverage Summary (R4 additions)

**`crates/anvil-ship/src/rollback.rs`** (1 new test):
- `test_rollback_retry_advances_epoch_boundary` — retry safety and offset-0 post-retry

**Total: 159 tests passing, 0 failed, clippy clean, fmt clean.**
