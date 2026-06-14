# Anvil — P1 Config Schema and Charter Loader Review Briefing (R3)

**Date:** 2026-05-25
**Scope:** Applied both R2 findings: empty-charter unit test and CHOICE_KEYS doc comment cleanup.
**Tests:** 9 tests in `anvil-core` + 2 in `anvil-cli` — all 9 passing. `cargo clippy --workspace -- -D warnings` clean.

---

## Findings from R2

| Finding | Severity | Fix Applied | Test |
|---|---|---|---|
| `load_charter` empty-file fix lacks a unit test | Medium (coverage gap) | Added `test_load_charter_rejects_empty_file` (writes a zero-byte temp file, asserts `Err(AnvilError::EmptyCharter(_))`) and `test_load_charter_returns_hash_for_nonempty_file` (asserts non-zero `byte_len` and 64-char hex hash) in `charter.rs` | Both tests pass |
| `CHOICE_KEYS` doc comment carries stale 16→17 history | Improvement | Replaced 4-line historical comment with a single sentence: "17 canonical Required Choice keys (9 Final + 8 Provisional). Changing this set requires a Charter/Plan amendment." | N/A — comment change |

---

## Test Coverage Summary

| Test | Kind | File | Hinge? | Phase |
|---|---|---|---|---|
| `test_load_charter_rejects_empty_file` | Unit | `crates/anvil-core/src/charter.rs` | No | P1 |
| `test_load_charter_returns_hash_for_nonempty_file` | Unit | `crates/anvil-core/src/charter.rs` | No | P1 |
| `test_provisional_validate_rejects_blank_hypothesis` | Unit | `crates/anvil-core/src/choices.rs` | No | P1 |
| `test_provisional_validate_rejects_blank_revision_trigger` | Unit | `crates/anvil-core/src/choices.rs` | No | P1 |
| `test_required_choices_count` | Unit | `crates/anvil-core/src/choices.rs` | Yes — pins=17, intended=required-choices-count | P1 |
| `test_project_layout_directories` | Unit | `crates/anvil-core/src/project.rs` | Yes — pins=16, intended=project-layout-directories | P1 |
| `test_provider_type_other_roundtrip` | Unit | `crates/anvil-core/src/config.rs` | No | P1 |
| `test_rust_toolchain_version_floor` | Unit | `crates/anvil-cli/src/main.rs` | Yes — pins=1.80, intended=stable-floor | P0 |
| `test_cli_entry_point_exists` | Unit | `crates/anvil-cli/src/main.rs` | Yes — pins=anvil, intended=binary-entry-point | P0 |

All 9 tests pass under `cargo test --workspace`. `cargo clippy --workspace -- -D warnings` clean.
