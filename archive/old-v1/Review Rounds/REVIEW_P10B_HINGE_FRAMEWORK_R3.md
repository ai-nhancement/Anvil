# P10b Hinge-Test Framework — Review Briefing (R3)

**Date:** 2026-05-27
**Scope:** Full P10b R2 finding responses — all 7 findings addressed
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P10b — Hinge-Test Framework
**Tests:** 189 passing (20 audit, 61 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed
**Clippy:** Clean (`-D warnings`, all targets)
**Fmt:** Clean (`cargo fmt --all -- --check`)
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 (2026-05-27, 8 findings), R2 (2026-05-27, 7 findings), all applied.

---

## R2 Finding Responses

### F1 (High) — Plan still required Rust `#[hinge_test]` proc-macro

**Resolution: Plan fully amended.**

`ANVIL_PLAN.md` §P10b action list item 1 and AC1 updated to describe the `// hinge_test:` source-comment scanner for Rust, not the `#[hinge_test]` proc-macro. The proc-macro is deferred; source scanning is the accepted v1 mechanism. `PLAN_HARDENING_HISTORY.md` Amendment 3 records this.

---

### F2 (High/Medium) — CI did not run strict hinge consensus check

**Resolution: Applied.**

`.github/workflows/ci.yml` Rust job gains a `Hinge consensus check (strict)` step after Format check, before `cargo audit`:

```yaml
- name: Hinge consensus check (strict)
  run: cargo run -q -p anvil-cli -- hinge list --strict --project .
```

This satisfies the R4 hardening-history invariant ("CI runs the check on every build"). `PLAN_HARDENING_HISTORY.md` Amendment 4 records this.

---

### F3 (Medium/High) — `#[tokio::test]` support missed `async fn`

**Resolution: Applied.**

`scan_rust_file` in `crates/anvil-hinge/src/lib.rs` now tries four prefixes in order when `saw_test` is true:

```rust
let fn_start = trimmed
    .strip_prefix("pub async fn ")
    .or_else(|| trimmed.strip_prefix("async fn "))
    .or_else(|| trimmed.strip_prefix("pub fn "))
    .or_else(|| trimmed.strip_prefix("fn "));
```

**Test added:** `test_scanner_recognizes_async_fn_after_tokio_test` — writes a temp file with `#[tokio::test]\nasync fn test_async_something()` preceded by a hinge annotation and asserts the entry is found with the correct function name.

---

### F4 (Medium) — `--count --strict` bypassed the strict check

**Resolution: Applied.**

`run_hinge_list` in `crates/anvil-cli/src/hinge.rs` now runs the strict consensus check before the count-mode early return. The strict check now runs regardless of whether `count_only` is true.

**Test updated:** The existing `test_hinge_list_count_with_strict_runs_strict_check` test (added in this round) verifies the combined flag path returns Ok on a clean workspace.

---

### F5 (Medium) — Alternative entries excluded from collision detection

**Resolution: Applied.**

`HingeRegistry::consensus_violations()` in `crates/anvil-hinge/src/lib.rs` now performs two additional checks after the source-entry phase check:

1. Duplicate `intended` IDs within the `.anvil/hinge-alternatives.toml` entries.
2. Collision between an alternative entry's `intended` and any Rust or Go source entry's `intended`.

**Test added:** `test_alternative_collision_with_source_entry_is_a_violation` — builds a registry with one Rust entry and one alternative entry sharing `intended=shared-id` and asserts one violation is produced with a reason containing "collides".

`PLAN_HARDENING_HISTORY.md` Amendment 6 records this.

---

### F6 (Low/Medium) — `HingeFlip.reasoning` addition breaks pre-R2 records

**Resolution: Applied.**

`HingeFlip.reasoning` is annotated `#[serde(default)]`. Pre-R2 records without the field deserialize with `reasoning = ""` rather than failing. Historical flip status is preserved; legacy records are marked by the absence of reasoning text.

`PLAN_HARDENING_HISTORY.md` Amendment 5 records this.

---

### F7 (Low) — Missing behavior tests for scanner/strict-mode behaviors

**Resolution: Applied (three scanner tests; one strict-without-store test).**

New tests in `crates/anvil-hinge/src/lib.rs`:
- `test_scanner_recognizes_async_fn_after_tokio_test` — verifies `#[tokio::test] async fn` is bound by the scanner.
- `test_scanner_skips_unbound_annotation_above_non_test` — verifies a hinge annotation above a `const` produces no entry.
- `test_scan_workspace_includes_tests_directory` — writes a hinge-annotated `.rs` file into a temp `tests/` directory and verifies `scan_workspace` finds it.
- `test_alternative_collision_with_source_entry_is_a_violation` — see F5.

New test in `crates/anvil-cli/src/hinge.rs`:
- `test_hinge_list_strict_succeeds_without_audit_store` — calls `run_hinge_list(tmp.path(), true, true)` on a temp dir with no `anvil.toml` and no hinge entries; asserts Ok.

---

## Test Count Delta

| Crate | R2 | R3 | Delta |
|---|---|---|---|
| anvil-cli | 59 | 61 | +2 (F4 strict-count test, strict-without-store test) |
| anvil-hinge | 3 | 7 | +4 (async scanner, unbound skip, tests/ dir, alt collision) |
| All others | 120 | 120 | — |
| **Total** | **182** | **189** | **+7** |

---

## Files Changed

| File | Change |
|---|---|
| `Anvil Plan/ANVIL_PLAN.md` | F1: replace proc-macro language in action list + AC1 |
| `Anvil Plan/PLAN_HARDENING_HISTORY.md` | F1/F2/F5/F6 amendments (3–6) |
| `.github/workflows/ci.yml` | F2: add hinge strict CI step |
| `crates/anvil-hinge/src/lib.rs` | F3: async fn prefix; F5: alt collision check; F7: 4 tests |
| `crates/anvil-hinge/Cargo.toml` | F7: add `tempfile` dev-dependency |
| `crates/anvil-cli/src/hinge.rs` | F4: strict before count; F7: 2 tests |
| `crates/anvil-audit/src/records.rs` | F6: `#[serde(default)]` on `reasoning` |

---

## What to Review

1. **F1 (Plan amendment).** Confirm action list item 1 and AC1 now accurately describe the source-comment scanner with no remaining proc-macro references.

2. **F2 (CI step).** Confirm the CI step command is correct. Note: CI runs against the workspace root (`--project .`), which has no `anvil.toml`, so `hinge list --strict` runs in source-only mode (no audit store required). Confirm this is the intended CI usage.

3. **F3 (async fn scanner).** Confirm the four-prefix chain is correct and complete for the project's test patterns. The scanner still requires a recognized test attribute (`#[test]`, `#[tokio::test]`, `#[test_case]`, `#[rstest]`) to be seen before the function line.

4. **F4 (--count --strict).** Confirm the new execution order is: scan → strict check (if requested) → count return (if count mode) → open store → print table.

5. **F5 (alternative collision).** Confirm that cross-namespace collision (alternative vs. source) and intra-alternative duplicates are detected. Note: the alt-vs-alt duplicate detection inserts into `alt_set` once per entry; if `intended` already present, the second occurrence fires a violation. Verify the logic correctly handles the case where the same alternative appears three times (should produce two violations for two duplicates).

6. **F6 (serde default on reasoning).** Confirm `#[serde(default)]` on a `String` field produces `""` on missing JSON keys. Confirm the `filter_map` in `run_hinge_list` now succeeds on legacy records rather than silently dropping them.

7. **F7 (behavior tests).** Confirm the temp-file scanner tests adequately exercise the claimed behaviors. Confirm unbound annotation skipping is the intended policy (no warning emitted).
