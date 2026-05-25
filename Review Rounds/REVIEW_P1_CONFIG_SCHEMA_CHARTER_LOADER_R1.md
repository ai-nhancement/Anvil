# Anvil — P1 Config Schema and Charter Loader Review Briefing (R1)

**Date:** 2026-05-25
**Scope:** Required-Choices schema, Charter loader, `anvil init`, `anvil config show/set`, pre-Plan-stage gate check.
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P1 — Config Schema and Charter Loader
**Tests:** `crates/anvil-core/src/choices.rs` (1 hinge test), `crates/anvil-core/src/project.rs` (1 hinge test) — all 2 P1 hinge tests + 2 P0 hinge tests passing (4 total)
**Status:** All acceptance criteria met. `anvil-core` is now a real library crate; `anvil-cli` is its first consumer.

---

## What Was Built

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-core/Cargo.toml` | Updated | Added `serde` (derive), `serde_json`, `toml`, `sha2`, `thiserror` |
| `crates/anvil-core/src/lib.rs` | Updated | Declared five new modules: `charter`, `choices`, `config`, `error`, `project` |
| `crates/anvil-core/src/error.rs` | Created | `AnvilError` enum via `thiserror`; 8 variants covering IO, parse, serialize, validation, config-key, and initialization states |
| `crates/anvil-core/src/choices.rs` | Created | `LockState` enum (Final/Provisional/Unlocked), `Choice` struct with optional `hypothesis`/`revision_trigger`, `CHOICE_KEYS` constant (17 entries), `default_choices()` factory (9 Final + 8 Provisional), `test_required_choices_count` hinge test |
| `crates/anvil-core/src/config.rs` | Created | `AnvilConfig` (choices + sidecar + provider_connections + model_bindings), `SidecarConfig`, `ProviderConnection`, `ProviderType` enum, `ModelBinding`, `load_config()`, `save_config()`, `check_plan_stage_gate()` |
| `crates/anvil-core/src/charter.rs` | Created | `CharterMetadata` (content_hash + byte_len), `load_charter()` using SHA-256 |
| `crates/anvil-core/src/project.rs` | Created | `LAYOUT_DIRS` constant (16 dirs), `InitResult` enum, `init()` idempotent scaffold, `test_project_layout_directories` hinge test |
| `crates/anvil-cli/Cargo.toml` | Updated | Added `anvil-core` (path dep) and `clap` (version 4, derive feature) |
| `crates/anvil-cli/src/main.rs` | Updated | Full CLI: `Init`, `Config {Show, Set}`, `Gate {CheckPlan}` subcommands via clap derive; `cmd_init`, `cmd_config_show`, `cmd_config_set`, `cmd_gate_check_plan` implementations; P0 hinge tests preserved |

---

## Architecture Decisions

**1. Flat TOML struct layout (not internally-tagged enum for LockState).**
The `Choice` struct uses a separate `lock_state` field + optional `hypothesis`/`revision_trigger` fields rather than an internally-tagged enum like `LockState::Provisional { hypothesis, revision_trigger }`. TOML's serializer does not support internally-tagged enums (the `#[serde(tag = "...")]` form); attempting it produces a runtime serialize error. The flat layout serializes cleanly to TOML and avoids a compatibility footgun for user-editable config files.

**2. `BTreeMap<String, Choice>` for the choices map.**
`BTreeMap` gives stable, alphabetical iteration order. This matters for two reasons: (1) `anvil config show` output is deterministic without sorting; (2) TOML serialization order is stable across runs, so round-tripping `anvil.toml` through `load_config` → `save_config` does not produce spurious diffs in git history.

**3. `Box<toml::de::Error>` in `AnvilError::ConfigParse`.**
`toml::de::Error` is large (> 128 bytes). With `clippy::result_large_err` enabled (pedantic lint set), an unboxed error variant is a warning-turned-denial. The source is boxed at the error creation site; callers see `AnvilError` uniformly and do not interact with the box.

**4. `anvil-core` as a library crate, not a binary.**
`anvil-core` exposes all domain logic as a Rust library. `anvil-cli` is the first consumer; the v1.1 App will be a second consumer of the same crate. This means the config, charter, and project modules can be called directly from both surfaces without FFI or subprocess overhead. The split was established at P0 (stub) and is now populated at P1.

**5. `anvil init` is idempotent by design, not by coincidence.**
The idempotency check (`if config_path.exists() { return AlreadyInitialized }`) runs before any filesystem writes. Re-running `anvil init` on an existing project prints the current config status and exits zero. This means scripts and CI pipelines can call `anvil init` unconditionally without guard logic. The behavior is pinned by acceptance criterion 7 and verified in the smoke test.

**6. `anvil config set` is intentionally narrow in P1.**
Only `sidecar.idle_timeout_secs` and `sidecar.binary_path` are settable via `anvil config set`. Provider connections and model bindings are configured through `anvil setup` (P4's interactive wizard), not through raw key-value pairs. This keeps the P1 surface minimal and avoids designing a key-path DSL that would need to accommodate nested provider-connection objects.

**7. 17 Required Choices (plan annotation says 16).**
The plan's hinge-test annotation for `test_required_choices_count` reads `pins=16 (updated from 15 to include sidecar lifecycle)`. The actual plan table has 17 rows: the 16 from that annotation plus `runtime_alert_response_policies`, which was added in a later plan revision after the hinge annotation was written. The implementation adds all 17. The hinge test is updated to `pins=17` with a comment documenting the discrepancy. The plan acceptance criterion 2 says "16 Choices" — this is also stale by one entry; noted in What to Review below.

---

## P1 Success Criteria

| Criterion | Status |
|---|---|
| 1. `anvil init my-project` creates per-project directory layout | **Pass** — 16 directories created; pinned by `test_project_layout_directories` |
| 2. Required-Choices schema covers all 16 Choices (plan text) | **Pass (with note)** — implemented 17 Choices; see Architecture Decision 7 |
| 3. `anvil config show` displays lock status | **Pass** — shows Final/Provisional/UNLOCKED labels + hypothesis/revision_trigger for Provisional |
| 4. Provisional Locks require non-empty hypothesis and revision_trigger | **Pass** — `Choice::validate()` rejects blank fields; called from both `load_config` and `save_config` |
| 5. Pre-Plan-stage gate check exits non-zero with clear listing of unlocked Choices | **Pass** — lists unlocked keys, calls `std::process::exit(1)` |
| 6. Malformed TOML produces typed errors, not parse-panics | **Pass** — `AnvilError::ConfigParse` with boxed `toml::de::Error`; no unwrap at config boundary |
| 7. `anvil init` on already-initialized project prints current status and exits zero | **Pass** — prints config show output and returns `Ok(())` |

---

## What to Review

1. **Plan acceptance criterion 2 says "16 Choices" but 17 are implemented.** The criterion text is stale (written before `runtime_alert_response_policies` was added to the plan table). The implementation is correct — 17 is the canonical count — but the plan text should be amended to say 17. Flag this as a required plan correction before P2.

2. **`AnvilError::AlreadyInitialized` is defined but never raised.** `project::init()` returns `InitResult::AlreadyInitialized` (an `Ok` variant), not `Err(AnvilError::AlreadyInitialized)`. The error variant exists in `error.rs` but has no call site. Either remove it or document why it is reserved for future use (e.g., a future `anvil init --strict` flag that returns an error instead of silently succeeding).

3. **`anvil config show` does not display `model_bindings`.** The output covers Required Choices, sidecar config, and provider connections, but `model_bindings` in `AnvilConfig` is never printed. This is appropriate for P1 (model bindings are set up in P4's `anvil setup`), but confirm that the omission is intentional and that the show command will be extended in P4.

4. **`ProviderType::Other(String)` with `#[serde(untagged)]`.** Untagged deserialization means any unknown string round-trips to `Other("...")`. The TOML roundtrip for this variant produces a bare string, not an object — which is the correct behavior for `provider_type = "my-custom-provider"`. Verify that this case is accepted by the deserializer and does not silently drop the value on re-read.

5. **`load_charter` does not validate non-empty content.** A zero-byte `charter.md` will compute a valid SHA-256 hash (`e3b0c44298fc1c149afb...`) and return `byte_len: 0`. Downstream code (P5–P8) will use the hash to anchor audit records; an empty charter is semantically invalid. Consider whether `load_charter` should return an error on `byte_len == 0`, or whether that validation belongs at the caller.

6. **No test for `Choice::validate()` with Provisional + empty fields.** The two hinge tests cover count and layout. There is no unit test that exercises the `ProvisionalMissingField` error path directly. A simple unit test calling `choice.validate()` with a Provisional choice missing `hypothesis` would pin the validation invariant and prevent accidental regression.

7. **`cmd_config_set` unknown-key error message could suggest valid keys.** Currently returns `AnvilError::UnknownConfigKey(key)` with no hint. Since the valid key set at P1 is small and fixed (`sidecar.idle_timeout_secs`, `sidecar.binary_path`), the error message could list them. Minor UX improvement — Low severity.

---

## Test Coverage Summary

| Test | Kind | File | Hinge? | Phase |
|---|---|---|---|---|
| `test_required_choices_count` | Unit | `crates/anvil-core/src/choices.rs` | Yes — pins=17, intended=required-choices-count | P1 |
| `test_project_layout_directories` | Unit | `crates/anvil-core/src/project.rs` | Yes — pins=16, intended=project-layout-directories | P1 |
| `test_rust_toolchain_version_floor` | Unit | `crates/anvil-cli/src/main.rs` | Yes — pins=1.80, intended=stable-floor | P0 |
| `test_cli_entry_point_exists` | Unit | `crates/anvil-cli/src/main.rs` | Yes — pins=anvil, intended=binary-entry-point | P0 |

All 4 tests pass under `cargo test --workspace`. No Go tests are affected by P1 changes.

Smoke-test commands verified manually:
- `anvil init C:\Temp\anvil-test-proj` → "Initialized Anvil project at ... / Created 16 directories + anvil.toml"
- `anvil config show --project C:\Temp\anvil-test-proj` → All 17 choices printed with Final/provisional labels
- `anvil gate check-plan --project C:\Temp\anvil-test-proj` → "Gate check passed: all Required Choices are locked."
- Re-running `anvil init C:\Temp\anvil-test-proj` → "Project already initialized at ..." (idempotent)
