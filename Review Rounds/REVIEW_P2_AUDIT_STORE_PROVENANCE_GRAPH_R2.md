# Anvil — P2 Audit Store and Provenance Graph Review Briefing (R2)

**Date:** 2026-05-25
**Scope:** Applied both actionable R1 findings: `RecordType` parser roundtrip tests and stale `.tmp` cleanup.
**Note:** A first external review pass was received but all five findings referenced implementation details that do not exist in the P2 code (hash-chained records, JSONL index format, `anvil graph export` command). Each was verified against the source and found to be based on incorrect premises. A second pass was requested; the second pass correctly identified the two real actionable items below.
**Tests:** 20 tests across all crates — all passing. `cargo clippy --workspace -- -D warnings` clean.

---

## Findings from R1

| Finding | Severity | Fix Applied | Test |
|---|---|---|---|
| `RecordType::from_type_name` / `from_dir_name` lack direct unit test coverage | Medium | Added three tests in `records.rs`: `test_record_type_name_roundtrip` (all 13 variants via `ALL_RECORD_TYPES`), `test_record_type_dir_name_roundtrip` (same via `dir_name()`), `test_record_type_parsers_reject_invalid_input` (6 invalid cases) | All three pass |
| Stale `_index.json.tmp` left on disk after crash | Low (polish) | `AuditStore::open()` now removes `_index.json.tmp` on startup if present (best-effort `remove_file`, ignores failure) | Behavior verified: the file is removed when present; non-presence is a no-op |

---

## Accepted deferred items (no change)

| Item | Decision |
|---|---|
| Cross-reference integrity (completeness only, not "zero-backing-records" check) | Deferred to P5+ when required cross-ref key lists exist |
| `ProvenanceGraph` requires explicit `build()` call | Correct design for CLI usage; lazy rebuild deferred if in-process need emerges in P5+ |
| `AnvilError::Json` is a catch-all for serde_json errors | Acceptable for P2; `RecordSerialize`/`RecordDeserialize` split deferred to P5+ |
| Minimal record struct fields | Intentional; full field specs land per-phase in P5–P8 |

---

## Test Coverage Summary

| Test | Kind | File | Hinge? | Phase |
|---|---|---|---|---|
| `test_record_type_name_roundtrip` | Unit | `crates/anvil-audit/src/records.rs` | No | P2 |
| `test_record_type_dir_name_roundtrip` | Unit | `crates/anvil-audit/src/records.rs` | No | P2 |
| `test_record_type_parsers_reject_invalid_input` | Unit | `crates/anvil-audit/src/records.rs` | No | P2 |
| `test_audit_store_required_types_present` | Unit | `crates/anvil-audit/src/store.rs` | Yes — pins=11, intended=charter-required-audit-types | P2 |
| `test_append_only_api_has_no_update_or_delete` | Unit | `crates/anvil-audit/src/store.rs` | Yes — pins=append-only, intended=audit-api-shape | P2 |
| `test_append_only_filesystem_o_excl` | Unit | `crates/anvil-audit/src/store.rs` | Yes — pins=o_excl, intended=filesystem-append-only | P2 |
| `test_audit_store_detects_deleted_records` | Unit | `crates/anvil-audit/src/store.rs` | Yes — pins=integrity-check, intended=audit-completeness-check | P2 |
| `test_cross_reference_key_stability` | Unit | `crates/anvil-audit/src/cross_ref.rs` | Yes — pins=cross-ref-format, intended=cross-reference-key-format | P2 |
| `test_cross_ref_key_rejects_incomplete_input` | Unit | `crates/anvil-audit/src/cross_ref.rs` | No | P2 |
| `test_provenance_graph_resolves_backing_records` | Unit | `crates/anvil-graph/src/graph.rs` | No | P2 |
| `test_provenance_graph_returns_empty_for_unbacked_key` | Unit | `crates/anvil-graph/src/graph.rs` | No | P2 |
| `test_load_charter_rejects_empty_file` | Unit | `crates/anvil-core/src/charter.rs` | No | P1 |
| `test_load_charter_returns_hash_for_nonempty_file` | Unit | `crates/anvil-core/src/charter.rs` | No | P1 |
| `test_provisional_validate_rejects_blank_hypothesis` | Unit | `crates/anvil-core/src/choices.rs` | No | P1 |
| `test_provisional_validate_rejects_blank_revision_trigger` | Unit | `crates/anvil-core/src/choices.rs` | No | P1 |
| `test_required_choices_count` | Unit | `crates/anvil-core/src/choices.rs` | Yes — pins=17 | P1 |
| `test_project_layout_directories` | Unit | `crates/anvil-core/src/project.rs` | Yes — pins=18 | P2 |
| `test_provider_type_other_roundtrip` | Unit | `crates/anvil-core/src/config.rs` | No | P1 |
| `test_rust_toolchain_version_floor` | Unit | `crates/anvil-cli/src/main.rs` | Yes — pins=1.80 | P0 |
| `test_cli_entry_point_exists` | Unit | `crates/anvil-cli/src/main.rs` | Yes — pins=anvil | P0 |

All 20 tests pass under `cargo test --workspace`. `cargo clippy --workspace -- -D warnings` clean.
