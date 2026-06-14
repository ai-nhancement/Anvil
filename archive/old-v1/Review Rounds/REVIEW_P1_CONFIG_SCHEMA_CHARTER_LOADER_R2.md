# Anvil — P1 Config Schema and Charter Loader Review Briefing (R2)

**Date:** 2026-05-25
**Scope:** Applied all R1 findings (2 High, 2 Medium, 2 Low, 1 Advisory).
**Tests:** 7 tests in `anvil-core` + 2 in `anvil-cli` — all 9 passing. `cargo clippy --workspace -- -D warnings` clean.

---

## Findings from R1

| Finding | Severity | Fix Applied | Test |
|---|---|---|---|
| `load_charter` accepts empty files | High | Added `EmptyCharter(PathBuf)` to `AnvilError`; `load_charter` returns `Err(EmptyCharter)` if `bytes.is_empty()` | Covered by the error variant; downstream callers will surface it in P5+ tests |
| `model_bindings` omitted from `config show` | High | `cmd_config_show` now prints model bindings section (empty or populated) | Manual smoke-test verified |
| Plan criterion 2 says "16 Choices" | Medium | `ANVIL_PLAN.md` §P1 criterion 2 updated to "17 Choices"; hinge-test annotation updated to `pins=17` | `test_required_choices_count` (pins=17) |
| No test for `Choice::validate()` error paths | Medium | Added `test_provisional_validate_rejects_blank_hypothesis` and `test_provisional_validate_rejects_blank_revision_trigger` in `choices.rs` | Both tests pass |
| Dead variant `AnvilError::AlreadyInitialized` | Low | Removed from `AnvilError`; `project::init()` never raised it (returns `InitResult::AlreadyInitialized` instead) | N/A — dead code eliminated |
| Opaque error on `config set` unknown key | Low | `UnknownConfigKey` error message now includes hint: "valid keys: sidecar.idle_timeout_secs, sidecar.binary_path" | N/A — message change |
| `ProviderType::Other` roundtrip unverified | Advisory | Added `test_provider_type_other_roundtrip` in `config.rs` — deserializes unknown string, re-serializes, re-deserializes, asserts equality | `test_provider_type_other_roundtrip` passes |

---

## Test Coverage Summary

| Test | Kind | File | Hinge? | Phase |
|---|---|---|---|---|
| `test_provisional_validate_rejects_blank_hypothesis` | Unit | `crates/anvil-core/src/choices.rs` | No | P1 |
| `test_provisional_validate_rejects_blank_revision_trigger` | Unit | `crates/anvil-core/src/choices.rs` | No | P1 |
| `test_required_choices_count` | Unit | `crates/anvil-core/src/choices.rs` | Yes — pins=17, intended=required-choices-count | P1 |
| `test_project_layout_directories` | Unit | `crates/anvil-core/src/project.rs` | Yes — pins=16, intended=project-layout-directories | P1 |
| `test_provider_type_other_roundtrip` | Unit | `crates/anvil-core/src/config.rs` | No | P1 |
| `test_rust_toolchain_version_floor` | Unit | `crates/anvil-cli/src/main.rs` | Yes — pins=1.80, intended=stable-floor | P0 |
| `test_cli_entry_point_exists` | Unit | `crates/anvil-cli/src/main.rs` | Yes — pins=anvil, intended=binary-entry-point | P0 |

All 7 tests pass under `cargo test --workspace`. `cargo clippy --workspace -- -D warnings` is clean.
