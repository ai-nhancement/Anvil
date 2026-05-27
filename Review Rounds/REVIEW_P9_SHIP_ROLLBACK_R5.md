# P9 Ship + Rollback — Review Briefing (R5)

**Date:** 2026-05-26
**Scope:** R4 findings resolution — fmt pass, stale gate-based docs updated, layout-dir bijection check, test rename (scope clarification), DISPOSITION_SHIPPED doc clarification
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P9 — Ship + Rollback (Cascading Invalidation)
**Tests:** 159 passing (19 audit, 54 cli, 49 core, 9 graph, 17 ship, 11 sidecar-client), 0 failed
**Clippy:** Clean (`-D warnings`, all targets)
**Fmt:** Clean (`cargo fmt --all -- --check`)

---

## R4 Findings Disposition

### F1 (High) — `cargo fmt --all -- --check` fails, contradicting the R4 validation claim
**Status: Resolved**

Ran `cargo fmt --all`. Four files reformatted:
- `crates/anvil-cli/src/phase.rs` — `PhaseDisposition::new(...)` call broken to trailing-comma style
- `crates/anvil-cli/src/ship.rs` — `GateApproval::new(...)` call in test
- `crates/anvil-core/src/project.rs` — `expected_total` multi-line addition and `assert_eq!` macro formatting
- `crates/anvil-ship/src/rollback.rs` — `make_contract(vec![...])` inline argument

`cargo fmt --all -- --check` now passes. The R5 validation claim reflects the actual tree state.

### F2 (Medium) — Shipped-state docs still describe obsolete gate-based authority
**Status: Resolved**

Three comment sites in `crates/anvil-ship/src/ship.rs` updated:

1. **`ShipReadiness::unshipped_phases` field doc** — changed from "has never had a `phase-{id}-ship` gate, or its latest ship gate is older than its latest `RollbackEvent`" to reference `PhaseDisposition` with `disposition == DISPOSITION_SHIPPED`.

2. **`check_all_phases_shipped` function doc** — replaced "has a `phase-{id}-ship` `GateApproval` whose `created_at` is strictly newer than..." with "has a `PhaseDisposition` record with `disposition == DISPOSITION_SHIPPED` whose `created_at` is strictly newer than...". Also added: "The `phase-{id}-ship` `GateApproval` is not used for this determination; see the module-level docs for the rationale."

3. **Timestamp comparison comment in `is_phase_currently_shipped`** — changed "a `RollbackEvent` is always written after the ship gate it invalidates" to "a `RollbackEvent` is always written after the shipped `PhaseDisposition` it invalidates".

The module-level docs (added in R3), the function docs, and the inline comment now all consistently describe `PhaseDisposition` as the authority.

### F3 (Medium) — Layout-dir invariant is one-way; extra directories can slip in unnoticed
**Status: Resolved**

`test_all_record_type_dirs_covered_by_layout_dirs` in `crates/anvil-audit/src/store.rs` is now a full bijection check:

- **Forward** (was already present): every `RecordType::dir_name()` is in `AUDIT_RECORD_DIR_NAMES`.
- **Reverse** (new): every `AUDIT_RECORD_DIR_NAMES` entry resolves to `Some` via `RecordType::from_dir_name`.
- **Count** (new): `AUDIT_RECORD_DIR_NAMES.len() == ALL_RECORD_TYPES.len()`.

If a name is added to `AUDIT_RECORD_DIR_NAMES` without a matching `RecordType` variant (or vice versa), the test now fails.

`test_project_layout_directories` in `crates/anvil-core/src/project.rs` also updated: the expected count is now the hard-coded literal `20` (with an explanatory comment) rather than being derived from `BEFORE_AUDIT_STORE_DIRS.len() + AUDIT_RECORD_DIR_NAMES.len() + AFTER_AUDIT_STORE_DIRS.len()`. This means the assert is no longer tautological — adding an entry to any of the three constants will produce a failing assertion until the literal is manually updated.

### F4 (Medium/Low) — Test name overstates what is pinned
**Status: Resolved (rename)**

`test_rollback_retry_advances_epoch_boundary` renamed to
`test_rollback_retry_appends_duplicate_records_without_breaking_offset`.

The test body comment was also updated to clearly state:
- What IS pinned: duplicate rollback records do not crash; a zero-RFP epoch always produces offset 0.
- What IS NOT pinned: the `max(created_at)` boundary semantics — that requires an RFP created strictly between two rollback timestamps, which would need a sleep. The specification lives in the `rotation_offset_for_phase` doc comment.

The rename option from the R4 suggestion was chosen over a sleep-based or timestamp-injection test. See "What to Review" item 1 below if the reviewer prefers the stronger test.

### F5 (Low) — `DISPOSITION_SHIPPED` doc implied type-level enforcement that doesn't exist
**Status: Resolved**

`DISPOSITION_SHIPPED` doc comment in `crates/anvil-audit/src/records.rs` reworded:

Before: "The only valid value for `PhaseDisposition.disposition` that signals a shipped phase."

After: "The only value of `PhaseDisposition.disposition` recognized as shipped state by the shipped-state query (`latest_shipped_disposition_at`)."

The new wording also explicitly notes that `PhaseDisposition` is an open audit record — future disposition values are possible but will not satisfy the shipped-state check. The field itself is not type-constrained.

---

## P9 Acceptance Criteria (post-R5, unchanged)

| Criterion | Status |
|---|---|
| AC1: `anvil ship` succeeds only when all phases shipped | ✓ `PhaseDisposition` authority documented consistently in module, function, and inline docs |
| AC2: Transport actions execute in order; typed failure | ✓ Unchanged |
| AC3: Blast radius shown before commit | ✓ Unchanged |
| AC4: Explicit confirmation / `--yes` CI path | ✓ Unchanged |
| AC5: One `RollbackEvent` per invalidated phase | ✓ Retry safety pinned by renamed test |
| AC6: Ship blocked on unresolved rollbacks | ✓ Unchanged |
| AC7: Audit store immutable through rollback | ✓ Unchanged |
| AC8: Charter/Plan amendment triggered (v1: instruction only) | ✓ Unchanged |
| Plan cross-cutting: `--yes` non-interactive path | ✓ Unchanged |

---

## Hinge Tests (one rename from R4)

| Test | Location | What It Pins |
|---|---|---|
| `test_rollback_transitive_invalidation` | `rollback.rs` | Cascade + sibling records |
| `test_audit_store_immutable_through_rollback` | `ship.rs` (anvil-ship) | Append-only invariant |
| `test_rollback_resets_rotation_on_dependents` | `rollback.rs` | Rotation offset 0 post-rollback |
| `test_project_layout_directories` | `project.rs` | Hard-pinned 20-entry count, structural order |
| `test_rollback_retry_appends_duplicate_records_without_breaking_offset` | `rollback.rs` | Retry appends without error; offset 0 with no RFPs |

---

## What to Review

1. **F4 rename vs. stronger test.** The test was renamed rather than strengthened with explicit timestamp injection. The `max(created_at)` boundary rule is specified only in the doc comment of `rotation_offset_for_phase`. Is a rename sufficient for v1, or is an executable regression guard needed? A controlled-timestamp test would require either a sleep or an internal helper that accepts `created_at` directly (currently `RollbackEvent::new` uses `Utc::now()`).

2. **`layout_dirs()` function vs. constant.** Unchanged from R4. Allocates `Vec<String>` per call; called once from `project::init` and once from `setup.rs` `step6_store`. Not a performance concern at v1 scale.

---

## Test Coverage Summary (R5 — no new tests, one rename)

**`crates/anvil-ship/src/rollback.rs`**:
- `test_rollback_retry_appends_duplicate_records_without_breaking_offset` (renamed from `test_rollback_retry_advances_epoch_boundary`)

**`crates/anvil-audit/src/store.rs`**:
- `test_all_record_type_dirs_covered_by_layout_dirs` — strengthened to full bijection check (no rename)

**Total: 159 tests passing, 0 failed, clippy clean, fmt clean.**
