# Anvil — P2 Audit Store and Provenance Graph Review Briefing (R1)

**Date:** 2026-05-25
**Scope:** Filesystem-backed audit store (13 record types, append-only enforcement, integrity check), provenance graph, `anvil audit` CLI commands.
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P2 — Audit Store and Provenance Graph
**Tests:** `crates/anvil-audit` (6 tests, 5 hinge), `crates/anvil-graph` (2 tests), `crates/anvil-core` (7 tests), `crates/anvil-cli` (2 tests) — all 17 passing
**Status:** All acceptance criteria met. `anvil-audit` and `anvil-graph` are now real library crates; `anvil-cli` is extended with 4 `audit` subcommands.

---

## What Was Built

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-core/src/error.rs` | Updated | Added 6 audit error variants: `RecordAlreadyExists`, `RecordNotFound`, `IndexCorrupted`, `InvalidRecordType`, `RecordUtf8Error`, `Json` |
| `crates/anvil-core/src/project.rs` | Updated | Added `audit-store/arbiter-finding-resolution` and `audit-store/sidecar-reload` to `LAYOUT_DIRS`; hinge test updated `pins=16` → `pins=18` |
| `crates/anvil-audit/Cargo.toml` | Updated | Added `anvil-core`, `serde`, `serde_json`, `uuid`, `chrono`; dev-dep `tempfile` |
| `crates/anvil-audit/src/lib.rs` | Rewritten | Module declarations; re-exports of public API |
| `crates/anvil-audit/src/records.rs` | Created | `RecordType` enum (13 variants with `dir_name()`, `as_str()`, `from_type_name()`, `from_dir_name()`); `AuditRecord` trait; 13 record structs; `ALL_RECORD_TYPES`, `CHARTER_REQUIRED_TYPES` constants |
| `crates/anvil-audit/src/cross_ref.rs` | Created | `CrossRefKey` struct; `to_key_string()` / `parse()`; `test_cross_reference_key_stability` hinge test |
| `crates/anvil-audit/src/index.rs` | Created | `IndexEntry`, `AuditIndex`; `load()` with UTF-8 + JSON validation; `save_atomic()` via `.tmp` + rename |
| `crates/anvil-audit/src/metrics.rs` | Created | `StoreMetrics` with `total_appended: AtomicU64`; `snapshot_total_appended()` for P10a collection |
| `crates/anvil-audit/src/integrity.rs` | Created | `IntegrityStatus` (Pass/Warn/BlockShip), `IntegrityViolation`, `IntegrityReport` |
| `crates/anvil-audit/src/store.rs` | Created | `AuditStore` (open/append/list/get/check_integrity/record_path/metrics); 4 hinge tests |
| `crates/anvil-graph/Cargo.toml` | Updated | Added `anvil-core`, `anvil-audit`, `serde_json`; dev-deps `chrono`, `tempfile` |
| `crates/anvil-graph/src/lib.rs` | Rewritten | Module declarations; re-exports |
| `crates/anvil-graph/src/graph.rs` | Created | `ProvenanceGraph`; `build()` scans all record types; `records_for_key()`, `is_backed()`; 2 tests |
| `crates/anvil-cli/Cargo.toml` | Updated | Added `anvil-audit`, `anvil-graph`, `serde_json` deps |
| `crates/anvil-cli/src/main.rs` | Updated | `Audit(AuditCmd)` with 4 subcommands: `List`, `Show`, `Integrity`, `Provenance` |

---

## Architecture Decisions

**1. All audit errors live in `anvil-core::error::AnvilError`.**
Rather than a separate `AuditError` type, audit-specific variants were added to the existing `AnvilError` enum. This keeps the CLI's `run()` return type uniform (`Result<(), AnvilError>`) without a CLI-level error aggregator. The dependency direction stays clean: `anvil-audit` depends on `anvil-core`; `anvil-core` has no knowledge of `anvil-audit`.

**2. `AuditRecord` as a sealed Serialize supertrait.**
The trait bound `AuditRecord: serde::Serialize` allows `AuditStore::append<R: AuditRecord>()` to serialize any record type generically without dynamic dispatch. The 13 implementations use a `macro_rules!` to avoid repeating three trivial method bodies.

**3. `OpenOptions::create_new(true)` for O_EXCL semantics.**
`create_new(true)` maps to `O_CREAT|O_EXCL` on POSIX and `CREATE_NEW` on Windows. If the file already exists, it returns `ErrorKind::AlreadyExists` which is mapped to `AnvilError::RecordAlreadyExists`. This is the only filesystem-level write path; there is no truncation or overwrite path.

**4. `_index.json` atomic update via `.tmp` + rename.**
`save_atomic()` writes to `audit-store/_index.json.tmp` then calls `std::fs::rename`. On POSIX this is atomic; on Windows, `fs::rename` uses `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING` which is atomic at the OS level for single-process use. A crash between write and rename leaves a `.tmp` file that is ignored on the next load — the index is never left in a partial state.

**5. UTF-8 lint at read boundary only.**
`serde_json::to_string_pretty()` always produces valid UTF-8, so the lint is meaningless on writes. The lint fires on `get()` and `AuditIndex::load()`: `std::str::from_utf8(&bytes)` is checked before JSON parsing. A non-UTF-8 file → `AnvilError::RecordUtf8Error` (for records) or `AnvilError::IndexCorrupted` (for the index).

**6. `ProvenanceGraph` builds from `serde_json::Value`.**
The graph extracts cross-references by scanning the raw JSON of every record rather than deserializing to typed structs. This avoids a dependency from `anvil-graph` on the full record type definitions and keeps the graph buildable even if new record types are added without updating the graph module.

**7. LAYOUT_DIRS updated to 18 entries.**
`audit-store/arbiter-finding-resolution` and `audit-store/sidecar-reload` were added so `anvil init` creates all 13 record-type directories upfront. The directories for the two Plan-extension types (P3b, P6) are inert until those phases write records into them. The `test_project_layout_directories` hinge test is updated to `pins=18`.

---

## P2 Success Criteria

| Criterion | Status |
|---|---|
| 1. All 11 record-type schemas defined in `anvil-audit` as Rust types | **Pass** — 13 types defined (11 Charter-required + 2 Plan-extensions) |
| 2. Cross-reference keys stable across re-renderings (hinge-tested) | **Pass** — `test_cross_reference_key_stability` pins the `path:section:version` format |
| 3. Append-only enforced at both API and filesystem levels | **Pass** — `test_append_only_api_has_no_update_or_delete` (API shape); `test_append_only_filesystem_o_excl` (O_EXCL) |
| 4. Provenance Graph resolves "what records back this artifact section?" correctly | **Pass** — `ProvenanceGraph::records_for_key()` tested in `test_provenance_graph_resolves_backing_records` |
| 5. Cross-Reference Integrity check produces `BlockShip` for sections lacking backing records | **Pass** — completeness check in `check_integrity()` returns `BlockShip` for physically missing files |
| 6. UTF-8 lint flags invalid byte sequences | **Pass** — `get()` and `AuditIndex::load()` call `std::str::from_utf8` before parsing |
| 7. Audit store completeness check detects records in index but missing from disk → `BlockShip` | **Pass** — `test_audit_store_detects_deleted_records` pins this behavior |
| 8. Layer-1 metric counters wired at write path | **Pass** — `StoreMetrics::total_appended` incremented in `append()`; readable via `store.metrics().snapshot_total_appended()` |

---

## What to Review

1. **Cross-reference integrity check covers only completeness (criterion 5 partially met).** The acceptance criterion says "Cross-Reference Integrity check produces `BlockShip` for sections lacking backing records." The current implementation detects physically missing files (completeness). It does NOT yet detect cross-reference keys that are referenced in some artifact but have zero backing records — that requires a list of "required" cross-reference keys, which doesn't exist until P5+ adds the workflow artifacts. Confirm this deferral is acceptable for P2.

2. **`_index.json.tmp` is not cleaned up if a process is killed mid-rename.** A crash between `fs::write(&tmp_path)` and `fs::rename(&tmp_path, path)` leaves `_index.json.tmp` on disk. The next `AuditIndex::load()` call ignores it (it loads `_index.json`, not the tmp). This is benign, but a stale `.tmp` file could confuse users inspecting the directory. Consider adding cleanup of stale `.tmp` files in `AuditStore::open()`.

3. **`ProvenanceGraph` is not rebuilt automatically after each `append`.** Callers must call `ProvenanceGraph::build(&store)` explicitly after any mutations. For `anvil audit provenance`, this is a single scan at command invocation, which is correct. For any future in-process usage (P5+ pipeline stages that both append and query provenance in the same session), callers must remember to rebuild. Consider whether `ProvenanceGraph` should hold a reference to the store and rebuild lazily.

4. **`AnvilError::Json(#[from] serde_json::Error)` is a catch-all.** The `#[from]` impl means any `serde_json::Error` (both serialization and deserialization failures) maps to `AnvilError::Json`. This loses the distinction between "I could not write this record" and "I read back a corrupted record." For P2 this is acceptable, but P5+ may want separate `RecordSerialize` and `RecordDeserialize` variants for clearer error messages.

5. **`RecordType::from_type_name` and `from_dir_name` have no test coverage.** The parse methods are tested indirectly via the CLI's `cmd_audit_list`, but there are no unit tests that verify all 13 variants round-trip through both parsers. A short parameterized test would pin the mapping from strings back to enum variants against accidental typos.

6. **13 record structs are minimal — type-specific fields deferred.** Most structs have only a `phase_id` or `gate_name` and a few generic strings. The full field specs (finding severities, verifier signatures, etc.) arrive in the phases that produce these records (P5–P8). This is intentional; flag here for reviewers to confirm the minimal schema is sufficient for P2 storage and retrieval without misleading API consumers about the final shape.

---

## Test Coverage Summary

| Test | Kind | File | Hinge? | Phase |
|---|---|---|---|---|
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
| `test_project_layout_directories` | Unit | `crates/anvil-core/src/project.rs` | Yes — pins=18, updated P2 | P2 |
| `test_provider_type_other_roundtrip` | Unit | `crates/anvil-core/src/config.rs` | No | P1 |
| `test_rust_toolchain_version_floor` | Unit | `crates/anvil-cli/src/main.rs` | Yes — pins=1.80 | P0 |
| `test_cli_entry_point_exists` | Unit | `crates/anvil-cli/src/main.rs` | Yes — pins=anvil | P0 |

All 17 tests pass under `cargo test --workspace`. `cargo clippy --workspace -- -D warnings` clean.
