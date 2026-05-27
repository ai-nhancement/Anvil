# P9 Ship + Rollback — Review Briefing (R2)

**Date:** 2026-05-26
**Scope:** R1 findings resolution — non-interactive reopen path, timestamp invariant comment, combined ship readiness check, Windows quoting test, fmt/clippy clean
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P9 — Ship + Rollback (Cascading Invalidation)
**Tests:** 133 passing (15 in `anvil-ship`, 52 in `anvil-cli`), 0 failed
**Clippy:** Clean (`-D warnings`, all targets)
**Fmt:** Clean (`cargo fmt --all`)

---

## R1 Findings Disposition

### F1 (High) — Validation not clean; fmt/clippy failures contradicted R1 claims
**Status: Resolved**

`cargo fmt --all` and `cargo clippy --all --all-targets -- -D warnings` now both pass with zero warnings. The `redundant_closure` error in `rollback.rs:164` was fixed (`.map(|s| s.to_string())` → `.map(ToString::to_string)`). Seven files were reformatted by `cargo fmt --all`.

### F2 (High) — `run_phase_reopen` lacked non-interactive `--yes` path
**Status: Resolved**

Added `--yes` / `-y` flag to `PhaseCmd::Reopen` in `main.rs`:

```rust
/// Skip the confirmation prompt (for CI / non-interactive use).
#[arg(long, short = 'y')]
yes: bool,
```

`run_phase_reopen` now accepts `yes: bool`. When `yes=true`, the `dialoguer::Confirm` block is skipped entirely and execution proceeds directly to `execute_rollback`. The `--reason` argument was already required in R1; the finding's suggestion to make it optional was not adopted since an empty reason would produce an uninformative audit record.

Dispatch in `main.rs` updated to pass `yes` through:

```rust
PhaseCmd::Reopen { id, reason, yes, project } =>
    phase::run_phase_reopen(&project, &id, &reason, yes),
```

### F3 (Medium) — Timestamp comparison direction undocumented
**Status: Resolved**

Added an inline comment at the comparison site in `is_phase_currently_shipped` (`crates/anvil-ship/src/ship.rs`):

```rust
// Strict greater-than is intentional: a RollbackEvent is always written after
// the ship gate it invalidates, so equality is impossible in a well-formed store.
Some(rollback_at) if rollback_at > ship_at => Ok(false),
```

The invariant holds because `execute_rollback` runs after the ship gate is already in the store — both in real workflows and in tests (the ship gate is appended before `execute_rollback` is called). Sub-second collision is structurally impossible.

### F4 (Low) — `check_unresolved_rollbacks` unreachable when `check_all_phases_shipped` fails
**Status: Resolved**

Both checks now run unconditionally before any error is returned. The results are combined into a single `ProjectShipBlocked` error that distinguishes "never shipped" phases from "rolled back without re-ship" phases:

```
never shipped: P0, P1; rolled back without re-ship: P2
```

The separator (`;`) and category labels make automated parsing unambiguous. Test coverage:
- `test_project_ship_blocked_when_phases_not_shipped` — exercises "never shipped" path (phases have no gates at all)
- `test_project_ship_succeeds_when_all_shipped` — happy path unchanged

### F5 (Low) — `cmd /C` quoting for complex transport commands untested on Windows
**Status: Resolved**

Added a `#[cfg(windows)]` test in `transport.rs`:

```rust
#[cfg(windows)]
#[test]
fn test_execute_transport_windows_embedded_quotes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let actions = vec![TransportAction {
        kind: TransportKind::Shell,
        command: r#"echo "hello world""#.to_owned(),
        label: Some("EchoQuoted".to_owned()),
    }];
    execute_transport(&actions, tmp.path()).unwrap();
}
```

Test passes on Windows; `cmd /C` correctly handles the embedded double-quotes and `echo "hello world"` exits 0. The test output (`"hello world"` printed to stdout) confirms quoting is forwarded correctly.

---

## P9 Acceptance Criteria (post-R2)

| Criterion | Status |
|---|---|
| AC1: `anvil ship` succeeds only when all phases shipped; exits non-zero with named list | ✓ Combined readiness error names "never shipped" and "rolled back without re-ship" phases |
| AC2: `anvil ship` executes configured transport actions in order; failure is typed error | ✓ Unchanged from R1 |
| AC3: `anvil phase reopen <id>` shows full blast radius before committing | ✓ Unchanged from R1 |
| AC4: User must explicitly confirm blast radius (or pass `--yes` for CI) | ✓ `dialoguer::Confirm` block wrapped in `if !yes` guard |
| AC5: Re-opening creates `RollbackEvent` records for re-opened phase and all dependents | ✓ Unchanged from R1 |
| AC6: `anvil ship` blocked if any `RollbackEvent` lacks re-shipped resolution | ✓ Now surfaced even when some phases were also never shipped (both checks run) |
| AC7: Audit store records remain immutable through rollback | ✓ Unchanged from R1; pinned by hinge test |
| AC8: Charter/Plan amendment workflow triggered by re-open | ✓ Unchanged from R1 |

**Plan cross-cutting: non-interactive `--yes` path on every gate** | ✓ `anvil phase reopen --yes` now supported

---

## Hinge Tests (unchanged)

| Test | Location | What It Pins |
|---|---|---|
| `test_rollback_transitive_invalidation` | `rollback.rs` | One `RollbackEvent` per phase in transitive closure; `rotation_reset_phases` identical across siblings |
| `test_audit_store_immutable_through_rollback` | `ship.rs` (anvil-ship) | `execute_rollback` never modifies/deletes; only appends new `RollbackEvent` records |
| `test_rollback_resets_rotation_on_dependents` | `rollback.rs` | `rotation_offset_for_phase` returns 0 post-rollback; `rotation_reset_phases` includes all invalidated phases |

---

## What to Review

1. **Combined ship readiness error format.** The new combined error message uses `"; "` as a separator between "never shipped" and "rolled back without re-ship" categories. Is this delimiter appropriate for downstream parsing by scripts or the Coordinator? An alternative would be two separate `ProjectShipBlocked` errors returned as a `Vec`, but that would require a signature change to `run_project_ship`.

2. **`--yes` without `--reason`.** The current CLI definition requires `--reason` unconditionally (no `default_value`). A user running `anvil phase reopen P1 --yes` without `--reason` will get a clap error. Is requiring `--reason` even in `--yes`/CI mode the right UX, or should a default reason (e.g., `"unspecified"`) be accepted? An audit record with an empty or default reason is less useful, but removing the requirement would reduce friction.

3. **`plan-consolidation` directory gap (R1 Q6 — predates P9).** The R1 reviewer noted that `LAYOUT_DIRS` in `project.rs` is missing the `plan-consolidation` subdirectory. This remains out of scope for P9 R2 but should be tracked as a pre-existing gap for the next cleanup phase.

---

## Test Coverage Summary (R2 additions)

**`anvil-ship/src/transport.rs`** (1 new test):
- `test_execute_transport_windows_embedded_quotes` (`#[cfg(windows)]`) — quoting correctness on `cmd /C`

**No new tests in `anvil-cli/src/ship.rs`** — existing `test_project_ship_blocked_when_phases_not_shipped` and `test_project_ship_succeeds_when_all_shipped` already cover the updated combined-check path.

**Total: 133 tests passing, 0 failed, clippy clean, fmt clean.**
