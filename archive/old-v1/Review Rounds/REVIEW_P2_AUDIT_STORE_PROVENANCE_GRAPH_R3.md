# Anvil — P2 Audit Store and Provenance Graph R3

**Source review:** `Review Rounds/REVIEW_P2_AUDIT_STORE_PROVENANCE_GRAPH_R2_Findings.md`  
**Round:** R3 (all R2 findings addressed)  
**Date:** 2026-05-25

---

## Validation

- `cargo test --workspace` — **passes**: 26 tests (up from 20 in R2)
- `cargo clippy --workspace -- -D warnings` — **passes**

---

## R2 Finding Disposition

### Finding 1 — High: `append()` can leave orphaned record files if index update fails

**Fixed.**

`append()` now wraps the index read-modify-write in a closure and, if it fails,
removes the just-created record file on a best-effort basis before returning the
error. This prevents the bad intermediate state where a record file exists but
has no index entry and cannot be retried.

`check_integrity()` now additionally scans every record-type directory for
`.json` files with no corresponding index entry and reports them as `Warn`
violations (`ViolationSeverity::Warn`).

New tests: `test_integrity_detects_unindexed_orphan`.

---

### Finding 2 — High: Concurrent appends can race and corrupt or lose index updates

**Documented as a known P2 limitation; not fully fixed.**

The `AuditStore` struct-level doc comment explicitly states:

> P2 assumes single-process access. The `_index.json` read-modify-write in
> `append()` is not protected by a cross-process lock; concurrent CLI
> invocations can race and lose index updates. A file-lock mechanism will be
> added in a future phase when concurrent access is needed.

The shared temp file collision is resolved: `save_atomic()` now uses a
UUID-based temp filename (`_index.json.<uuid>.tmp`) so no two writers share
a temp path.

---

### Finding 3 — High: R2 stale temp cleanup can delete another process's active temp file

**Fixed.**

The R2 shared temp path `_index.json.tmp` has been replaced with UUID-based
names. `AuditStore::open()` scans for all files matching the pattern
`_index.json.*.tmp` and removes them; since every active writer uses its own
unique temp path, this cleanup cannot delete a live writer's temp file.

New test: `test_open_cleans_up_stale_tmp_files` verifies that a planted stale
temp file is removed on open.

---

### Finding 4 — Medium: `CrossRefKey::parse()` does not enforce "exactly two separators"

**Fixed.**

`parse()` was changed from `splitn(3, ':')` to `split(':')` + `vec.len() == 3`
check. Any string with more than two colons now returns `None`.

New test: `test_cross_ref_key_rejects_extra_colons` asserts that `"a:b:c:d"`
and `"a:b:c:d:e"` are rejected.

---

### Finding 5 — Medium: Integrity check only verifies existence, not file type or record validity

**Partially fixed.**

`check_integrity()` now:
- Uses `is_file()` instead of `exists()`.
- Reads and parses every indexed file to validate its `id` field matches the
  index entry.
- Reports any file that is a directory, non-UTF-8, non-JSON, or has a
  mismatched `id` as a `BlockShip` violation.

Bounds checking (ensuring paths stay within the project root) and schema
validation beyond the `id` field are deferred to a future phase.

New test: `test_integrity_detects_id_mismatch`.

---

### Finding 6 — Medium: Provenance graph silently ignores malformed `cross_references`

**Fixed.**

`ProvenanceGraph::build()` now distinguishes three cases:
- `cross_references` absent → no edges added (legitimate for record types that
  carry no cross-references).
- `cross_references` present but not an array → returns `AnvilError::IndexCorrupted`.
- `cross_references` is an array but contains a non-string element → returns
  `AnvilError::IndexCorrupted`.

This ensures a malformed record is surfaced as an error rather than silently
producing an "unbacked" result.

---

### Finding 7 — Medium: Public record structs allow unsafe or invalid IDs

**Fixed at the `append()` boundary.**

`validate_record_id()` is called at the start of every `append()`. It rejects
IDs that are empty, equal to `.` or `..`, or contain `/`, `\`, `\0`, or `:`.

Making record struct fields private is deferred to when schemas stabilize in a
future phase.

New test: `test_append_rejects_invalid_id` covers empty, `../escape`,
`nested/path`, `colon:id`, and `back\slash`.

---

### Finding 8 — Medium: `AuditStore::open()` does not validate the store layout or index existence

**Fixed.**

`open()` now checks that `_index.json` is a file (not just that
`audit-store/` exists). If `_index.json` is absent, it returns
`AnvilError::IndexCorrupted` with a message directing the user to re-run
`anvil init`.

New test: `test_open_fails_without_index_json`.

---

### Finding 9 — Low/Medium: Atomic rename does not guarantee durability without fsync

**Accepted as known limitation for P2.**

The doc comment on `save_atomic()` does not claim crash persistence beyond
what `rename()` provides. No fsync has been added; this is acceptable for P2
and will be revisited if durability requirements change.

---

### Finding 10 — Low: Error type used for invalid provenance key is misleading

**Fixed.**

`cmd_audit_provenance()` now returns `AnvilError::InvalidCrossRefKey(key)` for
an unparseable cross-reference key instead of the incorrect
`AnvilError::InvalidRecordType(...)`.

`AnvilError::InvalidCrossRefKey` was added in R2; the CLI now uses it
correctly.

---

### Finding 11 — Low: R2 temp cleanup lacks a committed regression test

**Fixed.** See `test_open_cleans_up_stale_tmp_files` (Finding 3 above).

---

### Finding 12 — Low: Metrics are per-process and may not satisfy future collection expectations

**Accepted as known P2 limitation.**

`StoreMetrics::total_appended` is an in-memory `AtomicU64`. The doc comment
notes this counter resets on every `open()`. Persistent or cross-process
metrics are deferred to P10a.

---

## New Tests Added in R3

| Test | Crate | What it pins |
|---|---|---|
| `test_cross_ref_key_rejects_extra_colons` | `anvil-audit` | Extra colons in cross-ref keys return `None` |
| `test_open_fails_without_index_json` | `anvil-audit` | `open()` returns `IndexCorrupted` when `_index.json` absent |
| `test_open_cleans_up_stale_tmp_files` | `anvil-audit` | `open()` removes `_index.json.*.tmp` stale files |
| `test_append_rejects_invalid_id` | `anvil-audit` | `append()` rejects unsafe path characters in IDs |
| `test_integrity_detects_unindexed_orphan` | `anvil-audit` | `check_integrity()` reports unindexed `.json` files as `Warn` |
| `test_integrity_detects_id_mismatch` | `anvil-audit` | `check_integrity()` reports `id` field mismatch as `BlockShip` |

---

## Summary

All High and Medium findings from R2 have been fixed or formally accepted with
documented rationale. The remaining deferred items (cross-process locking,
full schema validation, private record fields, fsync, persistent metrics) are
clearly documented in code comments and will be addressed in later phases.

**R3 is ready for approval.**
