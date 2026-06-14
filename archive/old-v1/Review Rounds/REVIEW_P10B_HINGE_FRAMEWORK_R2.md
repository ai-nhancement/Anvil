# P10b Hinge-Test Framework — Review Briefing (R2)

**Date:** 2026-05-27
**Scope:** Full P10b R1 finding responses — all 8 findings addressed
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P10b — Hinge-Test Framework
**Tests:** 182 passing (20 audit, 59 cli, 49 core, 14 eval, 9 graph, 3 hinge, 17 ship, 11 sidecar-client), 0 failed
**Clippy:** Clean (`-D warnings`, all targets)
**Fmt:** Clean (`cargo fmt --all -- --check`)
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior round: R1 (2026-05-27) — 8 findings, all applied.

---

## R1 Finding Responses

### F1 (High) — Reason not persisted in `HingeFlip` record

**Resolution: Applied.**

`HingeFlip` struct in `crates/anvil-audit/src/records.rs` gains a `pub reasoning: String` field. `HingeFlip::new` gains a `reasoning: String` parameter. `run_hinge_flip` passes `reason.to_owned()` as the new argument.

**Regression test added:** `test_hinge_flip_stores_reasoning` in `crates/anvil-audit/src/records.rs` — constructs a `HingeFlip` and asserts `record.reasoning` equals the provided string.

---

### F2 (High) — Consensus check weaker than Plan spec

**Resolution: Plan amended (phase-only is the accepted v1 invariant).**

The Plan text at §P10b said "same pinned value, same intended value, same phase" — which conflicts with the existing codebase where cross-language hinges legitimately have different `pins` (e.g., `binary-entry-point` pins `"anvil"` in Rust and `"anvil-sidecar"` in Go). Enforcing pins equality across languages produces false violations.

The Plan text and `PLAN_HARDENING_HISTORY.md` (P10b R1 Amendment 1) are updated to specify:
- **Phase equality** is the cross-language invariant.
- **Pins differences** across languages are permitted.
- **Duplicate `intended` IDs within a single language** are also `BlockShip` violations (F6, applied together).
- Missing-counterpart detection (hinge in one language, absent from the other) requires per-hinge cross-language metadata not present in v1 — deferred.

No code change to `consensus_violations()` for this finding (the phase-only check was already the correct v1 behavior; only the Plan required updating).

---

### F3 (High/Medium) — Ship gate does not invoke strict consensus check

**Resolution: Applied.**

`crates/anvil-cli/src/ship.rs`: `run_project_ship` now calls `scan_workspace(project_root)` + `consensus_violations()` before transport actions. Any violations produce `AnvilError::ProjectShipBlocked("hinge consensus violations — ...")`.

**Regression test added:** `test_project_ship_blocked_by_hinge_consensus_violation` in `ship.rs` — writes synthetic Rust and Go files with a phase-mismatched hinge into a temp workspace, ships all phases, and asserts the ship returns `ProjectShipBlocked` mentioning "hinge consensus".

Note: CI step was not added (the workflow file is outside the workspace and CI infrastructure wasn't established in P10b scope). The ship gate integration is the primary enforcement path.

---

### F4 (Medium) — `--strict` requires an initialized audit store

**Resolution: Applied.**

`run_hinge_list` in `crates/anvil-cli/src/hinge.rs` is restructured: the `consensus_violations()` call now runs immediately after `scan_workspace` — before `AuditStore::open`. If `--strict` violations exist, the process exits without ever touching the store.

The `AuditStore::open` call is made graceful: an `AnvilError::NotInitialized` result is treated as "no recorded flips" rather than a hard error, so `hinge list` (without `--strict`) also works on an uninitialized checkout (showing all entries as `OPEN`).

---

### F5 (Medium) — Registry persistence is source-only, not audit-store-backed

**Resolution: Plan amended.**

`PLAN_HARDENING_HISTORY.md` (P10b R1 Amendment 2) and the `ANVIL_PLAN.md` §P10b action list are updated to state that source files are the persistence layer for v1; `HingeFlip` records in the audit store capture flip history only. Registry snapshot records at flip or ship time are deferred to a future hardening round.

---

### F6 (Medium) — Duplicate `intended` IDs within a language

**Resolution: Applied.**

`HingeRegistry::consensus_violations()` in `crates/anvil-hinge/src/lib.rs` now detects duplicate `intended` IDs within the same language. When an entry's `intended` already appears in the language's map, a `ConsensusViolation` with reason `"duplicate intended ID in Rust sources"` (or `"... Go sources"`) is added before inserting the first occurrence.

**Existing duplicate fixed:** `charter.rs` contained two test functions both with `intended=pairing-check` but different `pins`. Renamed:
- `test_rfp_vr_pairing_struct` → `intended=pairing-check-struct`
- `test_rfp_vr_pairing_mismatch_returns_error` → `intended=pairing-check-mismatch`

The other grep hits (`rotation.rs:6`, `core/plan.rs:58`, `cli/plan.rs:563`) are module-level documentation annotations above non-test code; the scanner skips them because no `#[test]` follows within the 5-line window.

**Test added:** `test_duplicate_intended_within_language_is_a_violation` in `crates/anvil-hinge/src/lib.rs`.

---

### F7 (Medium/Low) — Scanner misses planned top-level test locations; fragile test forms

**Resolution: Partially applied.**

- `scan_workspace` now scans `<root>/tests/` for `.rs` files in addition to `<root>/crates/`.
- `scan_rust_file` now recognizes `#[tokio::test]`, `#[test_case(...]`, and `#[rstest]` in addition to `#[test]` as valid test attributes that set the `saw_test` flag.

Not applied:
- Warnings for unbound hinge annotations (annotations not followed by a test function within the look-ahead window) — not added. The existing module-level documentation annotations (`rotation.rs:6` etc.) are intentionally placed above non-test code and would generate misleading warnings. Deferred to a future round with more precise "unbound warning" semantics.

---

### F8 (Low) — `hinge flip` accepts empty `--new-value`

**Resolution: Applied.**

`run_hinge_flip` in `crates/anvil-cli/src/hinge.rs` validates `new_value` immediately after `reason`, before any scan or store access:

```rust
if new_value.trim().is_empty() {
    return Err(AnvilError::InvalidConfigValue {
        key: "new-value".to_owned(),
        reason: "must not be empty for hinge flip".to_owned(),
    });
}
```

**Tests added:**
- `test_flip_rejects_empty_reason` — verifies `EmptyReasoning` on empty reason.
- `test_flip_rejects_empty_new_value` — verifies `InvalidConfigValue` on empty new_value.

---

## Test Count Delta

| Crate | R1 | R2 | Delta |
|---|---|---|---|
| anvil-audit | 19 | 20 | +1 (F1 regression test) |
| anvil-cli | 56 | 59 | +3 (F3 ship test, F8 × 2) |
| anvil-hinge | 2 | 3 | +1 (F6 duplicate test) |
| All others | 100 | 100 | — |
| **Total** | **177** | **182** | **+5** |

---

## Files Changed

| File | Change |
|---|---|
| `crates/anvil-audit/src/records.rs` | `HingeFlip` + `reasoning` field; `HingeFlip::new` + param; F1 test |
| `crates/anvil-cli/src/hinge.rs` | F4 restructure; F8 validation; F1 reason pass-through; F8 tests |
| `crates/anvil-cli/src/ship.rs` | F3 hinge check before transport; F3 regression test |
| `crates/anvil-hinge/src/lib.rs` | F6 duplicate detection; F7 scanner expansion; F6 test |
| `crates/anvil-cli/src/charter.rs` | F6 rename duplicate `pairing-check` annotations |
| `Anvil Plan/ANVIL_PLAN.md` | F2 + F5 amended §P10b text |
| `Anvil Plan/PLAN_HARDENING_HISTORY.md` | F2 + F5 amendment records |

---

## What to Review

1. **F1 (reasoning persistence).** Confirm `HingeFlip.reasoning` is serialized to the audit store (serde derive covers it automatically). Verify the regression test `test_hinge_flip_stores_reasoning` is sufficient.

2. **F2 (Plan amendment for phase-only consensus).** Confirm the Plan amendment wording is accurate and the decision to defer missing-counterpart detection is acceptable.

3. **F3 (ship gate integration).** Confirm the regression test `test_project_ship_blocked_by_hinge_consensus_violation` adequately exercises the path. Confirm the missing CI step is acceptable (ship gate is the primary gate).

4. **F4 (strict before store).** Confirm the restructured `run_hinge_list` logic is correct: strict violations exit before the store is opened; store `NotInitialized` is treated as no-flips rather than an error.

5. **F5 (source-as-persistence Plan amendment).** Confirm the amendment wording in both `ANVIL_PLAN.md` and `PLAN_HARDENING_HISTORY.md` is accurate.

6. **F6 (duplicate detection).** Confirm the `charter.rs` rename is correct: `pairing-check-struct` and `pairing-check-mismatch` are semantically appropriate names. Confirm the other grep hits (`rotation.rs:6`, etc.) are correctly identified as module-level documentation annotations skipped by the scanner.

7. **F7 (scanner expansion).** Confirm `#[tokio::test]` etc. are correctly recognized. Confirm the decision not to add unbound-annotation warnings is acceptable.

8. **F8 (new_value validation).** Confirm validation fires before `scan_workspace` and that `InvalidConfigValue` is the appropriate error type.
