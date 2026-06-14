# Anvil — P2 Audit Store and Provenance Graph R2 Findings

**Source review doc:** `Review Rounds/REVIEW_P2_AUDIT_STORE_PROVENANCE_GRAPH_R2.md`  
**Review date:** 2026-05-25  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo test --workspace` — **passes**: 20 tests
- `cargo clippy --workspace -- -D warnings` — **passes**
- Confirmed the two R1 fixes exist:
  - `RecordType::{from_type_name, from_dir_name}` roundtrip/rejection tests are present in `crates/anvil-audit/src/records.rs`
  - `AuditStore::open()` removes `audit-store/_index.json.tmp` best-effort in `crates/anvil-audit/src/store.rs`

---

## 1. High — `append()` can leave orphaned record files if index update fails

**Location:**  
`crates/anvil-audit/src/store.rs`, especially `AuditStore::append()`

**Problem:**  
`append()` writes the record file first using `create_new(true)`, then loads and rewrites `_index.json`.

Flow:

1. Serialize record
2. Create/write `audit-store/<type>/<id>.json`
3. Load `_index.json`
4. Append index entry
5. Save index atomically

If anything fails after the record file is created—index is corrupted, index write fails, rename fails, process crashes—the store can contain a physical record file that is **not indexed**.

Consequences:

- `list()` will not show the record.
- `get(id)` will not find the record because it searches only the index.
- A retry with the same record ID fails with `RecordAlreadyExists`, because the file already exists.
- `check_integrity()` does not detect this because it only checks “indexed record missing from disk,” not “disk record missing from index.”

This creates a bad intermediate state: the record exists, append-only semantics prevent retry, but the record is invisible to normal audit APIs.

**Suggested fix:**

- Add integrity detection for unindexed `*.json` files under audit-store record-type directories.
- Consider one of these recovery policies:
  - Treat unindexed record files as `BlockShip`.
  - Reconcile them into the index if they parse and match their directory/type.
  - Move them to a quarantine/recovery directory.
- For `append()` failure handling, consider best-effort cleanup of a newly-created record file if the index update fails before the append is committed.
- Add tests for:
  - Record file exists but index entry is absent.
  - Retry after partial append failure.
  - Corrupt index after record file write.

---

## 2. High — Concurrent appends can race and corrupt or lose index updates

**Location:**  
`crates/anvil-audit/src/index.rs`  
`crates/anvil-audit/src/store.rs`

**Problem:**  
`append()` performs a read-modify-write cycle on `_index.json` without any inter-process or intra-process lock.

Two concurrent appends can do this:

1. Process A loads index with records `[X]`.
2. Process B loads index with records `[X]`.
3. A appends `A`, saves `[X, A]`.
4. B appends `B`, saves `[X, B]`.

Result: record file `A.json` exists, but its index entry is lost.

This is closely related to Finding 1, but concurrency makes it likely even without crashes.

There is also a shared temp file path:

```text
_index.json.tmp
```

Multiple writers can collide on that path during `save_atomic()`.

**Suggested fix:**

- Add a store-level lock around index read-modify-write.
  - Cross-process locking is preferable because CLI invocations can overlap.
  - A lock file such as `audit-store/_index.lock` would be appropriate.
- Use a unique temp file per writer, not a shared `_index.json.tmp`.
- Keep the write protocol documented: lock → load → validate → write temp → rename → release lock.
- Add tests using multiple threads/processes appending different records and assert all entries survive.

---

## 3. High — R2 stale temp cleanup can delete another process’s active temp file

**Location:**  
`crates/anvil-audit/src/store.rs`, `AuditStore::open()`

**Problem:**  
The R2 fix removes `_index.json.tmp` whenever the store is opened:

```text
if tmp_path.exists() {
    remove_file(tmp_path)
}
```

This is safe only if there is guaranteed single-process access. If one process is currently in the middle of `AuditIndex::save_atomic()` and another process opens the store, the second process may delete the first process’s active temp file.

That can cause the first writer’s rename to fail and can create the orphan-record problem from Finding 1.

**Suggested fix:**

- Perform stale temp cleanup only while holding the same index/store lock used for append.
- Prefer unique temp names so cleanup can target only clearly stale files.
- Consider age-based cleanup, e.g. only delete temp files older than a conservative threshold.
- Add a regression test for temp cleanup if retained. The review doc says behavior was verified, but I did not see a dedicated test in the committed test suite.

---

## 4. Medium — `CrossRefKey::parse()` does not enforce “exactly two separators”

**Location:**  
`crates/anvil-audit/src/cross_ref.rs`

**Problem:**  
The documentation says parsing requires exactly two `:` separators:

```text
<artifact-path>:<section-id>:<version>
```

But the implementation uses `splitn(3, ':')`, which accepts extra colons by folding them into the `version` field.

Example behavior:

```text
a:b:c:d
```

parses as:

```text
artifact_path = "a"
section_id = "b"
version = "c:d"
```

This contradicts the doc comment and test description.

There is also a broader design concern: on Windows, absolute paths commonly contain a colon, e.g. `C:\...`. Since this environment is Windows, colon-delimited cross-reference keys are fragile if artifact paths are ever absolute or contain drive letters.

**Suggested fix:**

- Decide whether colons are forbidden inside all fields or escaped/encoded.
- If forbidden, validate `artifact_path`, `section_id`, and `version` reject `:`.
- If allowed, switch to an unambiguous encoding, such as structured JSON, URL escaping, or length-prefixed components.
- Add tests for:
  - Extra separators.
  - Windows-like paths.
  - Colons in section IDs or versions, if those should be rejected.

---

## 5. Medium — Integrity check only verifies existence, not file type or record validity

**Location:**  
`crates/anvil-audit/src/store.rs`, `check_integrity()`

**Problem:**  
`check_integrity()` currently checks only:

```text
path.exists()
```

This means the integrity check passes if:

- The expected path is a directory, not a file.
- The file exists but is not valid UTF-8.
- The file exists but is invalid JSON.
- The file JSON has the wrong `id`.
- The file JSON’s type does not match the indexed `record_type`.
- The indexed path points outside the project due to index tampering.

Some of these are outside the stated P2 threat model, but even for local corruption detection, “path exists” is a very weak completeness check.

**Suggested fix:**

- At minimum use `is_file()` instead of `exists()`.
- Consider validating that every indexed file:
  - Is within the project root.
  - Is valid UTF-8.
  - Is valid JSON.
  - Has an `id` field matching the index entry.
  - Has the expected `cross_references` shape.
- Add separate integrity violation reasons for missing file, non-file path, invalid UTF-8, invalid JSON, and metadata mismatch.

---

## 6. Medium — Provenance graph silently ignores malformed `cross_references`

**Location:**  
`crates/anvil-graph/src/graph.rs`

**Problem:**  
`ProvenanceGraph::build()` reads each record as generic JSON and then does:

```text
value.get("cross_references").and_then(|v| v.as_array())
```

If `cross_references` is missing, not an array, or contains non-string elements, the graph silently ignores it.

That can produce a false “unbacked” result even though the underlying audit record is malformed rather than genuinely missing backing records.

**Suggested fix:**

- Treat malformed `cross_references` as an error during graph build.
- Or move validation into `AuditStore::get()` / integrity checks.
- Add tests for:
  - Missing `cross_references`.
  - `cross_references` as a string/object.
  - Arrays containing non-string values.
- If future phases introduce typed record deserialization, use that instead of raw JSON field extraction.

---

## 7. Medium — Public record structs allow unsafe or invalid IDs

**Location:**  
`crates/anvil-audit/src/records.rs`  
`crates/anvil-audit/src/store.rs`

**Problem:**  
Most record structs have public fields, including `id`, and only three record types currently provide constructors. Callers can construct records with IDs containing path separators or traversal-like values.

`append()` uses the ID directly in a filename:

```text
audit-store/<record-type-dir>/<id>.json
```

A bad ID could cause unexpected paths, write failures, or platform-specific behavior.

Examples of risky IDs:

```text
../x
nested/path
C:\temp\x
name:with:colon
```

**Suggested fix:**

- Define and enforce an ID format, preferably UUID v4 or another safe filename subset.
- Validate IDs in `append()` before computing the file path.
- Consider making fields private and requiring constructors once schemas stabilize.
- Add tests for invalid IDs with slashes, backslashes, `..`, empty strings, and reserved filename characters.

---

## 8. Medium — `AuditStore::open()` does not validate the store layout or index existence

**Location:**  
`crates/anvil-audit/src/store.rs`

**Problem:**  
`AuditStore::open()` only checks that `audit-store/` exists. It does not verify:

- `_index.json` exists.
- Required record-type directories exist.
- `_index.json` is valid JSON.
- The directory layout matches `ALL_RECORD_TYPES`.

This means opening can succeed, but later calls fail in less direct ways.

**Suggested fix:**

- Either make `open()` validate the minimal layout or add a distinct `validate_layout()` path used by CLI commands.
- Return a more specific error if `_index.json` is missing.
- Add tests for missing index and missing record-type subdirectory.

---

## 9. Low / Medium — Atomic rename does not guarantee durability without fsync

**Location:**  
`crates/anvil-audit/src/index.rs`, `AuditIndex::save_atomic()`

**Problem:**  
The implementation writes a temp file and renames it, which protects against many partial-write states. However, it does not fsync the temp file or parent directory.

A crash or power loss may still lose the temp contents or directory entry depending on filesystem behavior.

For P2 this may be acceptable, but the review doc’s wording around atomicity could be read as stronger than what is guaranteed.

**Suggested fix:**

- If stronger crash consistency is desired:
  - Write temp file.
  - Flush/sync the temp file.
  - Rename.
  - Sync the parent directory where supported.
- Clarify documentation if P2 only promises logical atomic replacement, not durable crash persistence.

---

## 10. Low — Error type used for invalid provenance key is misleading

**Location:**  
`crates/anvil-cli/src/main.rs`, `cmd_audit_provenance()`

**Problem:**  
Invalid cross-reference keys are reported using:

```text
AnvilError::InvalidRecordType(...)
```

This produces an error category that does not match the problem.

**Suggested fix:**

- Add a dedicated `InvalidCrossRefKey` error variant.
- Use it in `cmd_audit_provenance()`.
- This will improve CLI clarity and future API consumers.

---

## 11. Low — R2 temp cleanup lacks a committed regression test

**Location:**  
`Review Rounds/REVIEW_P2_AUDIT_STORE_PROVENANCE_GRAPH_R2.md`  
`crates/anvil-audit/src/store.rs`

**Problem:**  
The review doc says stale `_index.json.tmp` cleanup behavior was verified. I confirmed the implementation exists, but I did not find a dedicated unit test asserting:

- `_index.json.tmp` is removed when present.
- Opening without `_index.json.tmp` is a no-op.
- Cleanup failure is ignored, if that remains intended.

**Suggested fix:**

- Add a small unit test around `AuditStore::open()` temp cleanup.
- If concurrency locking is added, test cleanup under the locking policy instead.

---

## 12. Low — Metrics are per-process and may not satisfy future collection expectations

**Location:**  
`crates/anvil-audit/src/metrics.rs`  
`crates/anvil-audit/src/store.rs`

**Problem:**  
`StoreMetrics::total_appended` is an in-memory counter attached to an `AuditStore` instance. It resets every time the store is opened.

The comment says P10a collection infrastructure reads from these counters. That may not be sufficient if metrics need to survive separate CLI invocations or process restarts.

**Suggested fix:**

- Clarify whether P2 only needs transient instrumentation hooks.
- If P10a needs durable counters, derive counts from `_index.json` or persist metrics separately.
- Consider exposing index-derived counts by record type.

---

## Overall Assessment

The two R2 fixes are present and the current test/clippy suite is clean. The most important remaining risks are not in the parser tests themselves, but in the audit store’s write protocol:

1. Non-atomic relationship between record files and `_index.json`
2. Lack of locking around index updates
3. Unsafe stale temp cleanup under concurrent access

If P2’s intended scope is strictly single-process, non-adversarial, and low-concurrency, these can be documented as assumptions. If the CLI may be run concurrently or interrupted during appends, the store should add locking, orphan detection, and stronger recovery/integrity checks before relying on it as a durable audit source.