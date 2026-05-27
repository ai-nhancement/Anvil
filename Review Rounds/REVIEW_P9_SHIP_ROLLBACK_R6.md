# P9 Ship + Rollback — Review Briefing (R6)

**Date:** 2026-05-26
**Scope:** R5 F1 resolution — fmt pass on `project.rs` `assert_eq!` reformatted by rustfmt; F2/F3/F4 closed as v1-accepted
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P9 — Ship + Rollback (Cascading Invalidation)
**Tests:** 159 passing (19 audit, 54 cli, 49 core, 9 graph, 17 ship, 11 sidecar-client), 0 failed
**Clippy:** Clean (`-D warnings`, all targets)
**Fmt:** Clean (`cargo fmt --all -- --check`)

---

## R5 Findings Disposition

### F1 (High) — `cargo fmt --all -- --check` fails on `project.rs`
**Status: Resolved**

The `assert_eq!(dirs.len(), 20, ...)` line introduced in R5 was not reformatted by rustfmt before the R5 briefing was written. Running `cargo fmt --all` produced one diff in `crates/anvil-core/src/project.rs:146–150`: rustfmt broke the long single-line `assert_eq!` into its multi-argument form:

```rust
assert_eq!(
    dirs.len(),
    20,
    "layout_dirs() must have exactly 20 entries (2 pre + 15 record + 3 post)"
);
```

`cargo fmt --all -- --check` now exits 0.

### F2 (Medium) — Stronger `max(created_at)` regression test absent
**Status: Closed (v1-accepted)**

R5 reviewer confirmed: "The rename chosen in R5 is acceptable for v1 given the cost of a sleep-based or internal-timestamp-injection test." The `max(created_at)` rule remains specified in the `rotation_offset_for_phase` doc comment. No code change.

### F3 (Low) — `layout_dirs()` is an allocating function
**Status: Closed (no action)**

R5 reviewer: "No action required. The current design correctly trades a trivial allocation for a single source of truth." No code change.

### F4 (Low) — `AUDIT_RECORD_DIR_NAMES` + `from_dir_name` requires manual sync on `RecordType` addition
**Status: Closed (no action)**

R5 reviewer: "No further automation is warranted for v1." The bijection test enforces correctness at CI time; the circular-dependency constraint rules out compile-time derivation. No code change.

---

## P9 Acceptance Criteria (post-R6, all satisfied)

| Criterion | Status |
|---|---|
| AC1: `anvil ship` succeeds only when all phases shipped | ✓ `PhaseDisposition` authority documented consistently in module, function, and inline docs |
| AC2: Transport actions execute in order; typed failure | ✓ |
| AC3: Blast radius shown before commit | ✓ |
| AC4: Explicit confirmation / `--yes` CI path | ✓ |
| AC5: One `RollbackEvent` per invalidated phase | ✓ Retry safety pinned by `test_rollback_retry_appends_duplicate_records_without_breaking_offset` |
| AC6: Ship blocked on unresolved rollbacks | ✓ |
| AC7: Audit store immutable through rollback | ✓ |
| AC8: Charter/Plan amendment triggered (v1: instruction only) | ✓ |
| Plan cross-cutting: `--yes` non-interactive path | ✓ |

---

## Hinge Tests (final state)

| Test | Location | What It Pins |
|---|---|---|
| `test_rollback_transitive_invalidation` | `rollback.rs` | Cascade + sibling records |
| `test_audit_store_immutable_through_rollback` | `ship.rs` (anvil-ship) | Append-only invariant |
| `test_rollback_resets_rotation_on_dependents` | `rollback.rs` | Rotation offset 0 post-rollback |
| `test_project_layout_directories` | `project.rs` | Hard-pinned 20-entry count, structural order |
| `test_rollback_retry_appends_duplicate_records_without_breaking_offset` | `rollback.rs` | Retry appends without error; offset 0 with no RFPs |

---

## What to Review

No open questions. All prior "What to Review" items are either resolved or closed as v1-accepted.

---

## Test Coverage Summary (R6 — no new tests)

Single change: `crates/anvil-core/src/project.rs` — `assert_eq!` reformatted by rustfmt (no logic change).

**Total: 159 tests passing, 0 failed, clippy clean, fmt clean.**
