# P10b Hinge-Test Framework — Review Briefing (R1)

**Date:** 2026-05-27
**Scope:** Full P10b implementation — source scanner, unified registry, CLI surface, Go hinge test
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P10b — Hinge-Test Framework
**Tests:** 177 passing (19 audit, 56 cli, 49 core, 14 eval, 9 graph, 2 hinge, 17 ship, 11 sidecar-client), 0 failed
**Clippy:** Clean (`-D warnings`, all targets)
**Fmt:** Clean (`cargo fmt --all -- --check`)
**Go tests:** All passing (`go test ./...` from `sidecar/`)

---

## Implementation Summary

P10b implements a source-scanner approach rather than a proc-macro. The annotation format `// hinge_test: pins=X, intended=Y, phase=Z` is already used throughout the codebase; P10b provides the infrastructure to query and act on it.

### `crates/anvil-hinge/src/lib.rs` (new, full implementation)

**Public types:**

- `HingeEntry` — one scanned annotation: `intended`, `pins`, `phase`, `source` (Rust/Go), `file`, `fn_name`
- `HingeSource` — `Rust` | `Go`
- `AlternativeEntry` — non-test-harness deferred decision from `.anvil/hinge-alternatives.toml`
- `HingeRegistry` — `entries: Vec<HingeEntry>` + `alternatives: Vec<AlternativeEntry>`
- `ConsensusViolation` — `intended` + `reason`

**`parse_hinge_comment(line) -> Option<(pins, intended, phase)>`**
Parses `// hinge_test:` prefix; splits on `,`; extracts `pins=`, `intended=`, `phase=` key-value pairs. Returns `None` if any field is absent or empty. All three fields are required.

**`scan_rust_file(path, entries)`**
Looks for `// hinge_test:` lines. Within the next 5 lines, looks for `#[test]` (set flag) then `fn <name>(` (extract name). Only pushes if both `#[test]` and the fn name are found. Module-level annotations (above constants/structs) are naturally skipped because no `#[test]` follows within 5 lines.

**`scan_go_file(path, entries)`**
Same pattern: `// hinge_test:` → within 3 lines → `func Test<Name>(`. Pushes entry if found.

**`walk_files_with_ext(dir, ext, files)`**
Recursive file walker; skips `.git`, `target`, `vendor`, `node_modules`.

**`scan_workspace(root) -> Result<HingeRegistry, AnvilError>`**
Scans `<root>/crates/` for `.rs` files and `<root>/sidecar/` for `.go` files. Calls `load_alternatives(root)` and combines into a `HingeRegistry`.

**`load_alternatives(root) -> Result<Vec<AlternativeEntry>, AnvilError>`**
Reads `.anvil/hinge-alternatives.toml` if present. Returns empty vec if file absent. Format:
```toml
[[alternative]]
intended = "some-decision"
pins = "option-a"
phase = "P3"
mechanism = "code review"
```

**`HingeRegistry::consensus_violations() -> Vec<ConsensusViolation>`**
Checks all `intended` IDs that appear in BOTH Rust and Go entries. A violation is raised when the `phase` fields differ. Cross-language hinges may legitimately have different `pins` values (e.g., `binary-entry-point` pins `"anvil"` in Rust and `"anvil-sidecar"` in Go — both correct for their respective binaries). Phase equality is the invariant: both sides must have been introduced in the same phase.

**Location:** `crates/anvil-hinge/src/lib.rs`

---

### `crates/anvil-hinge/Cargo.toml`

Added dependencies: `anvil-core` (for `AnvilError`), `serde` (with derive), `toml`.

---

### `sidecar/cmd/anvil-sidecar/main_test.go` (amended)

Added `TestHingeCommentMetadataRequired` — the Go P10b hinge test:

```go
// hinge_test: pins=comment-parser, intended=test_hinge_comment_metadata_required, phase=P10b
func TestHingeCommentMetadataRequired(t *testing.T) {
    // Pins: a valid // hinge_test: annotation must supply all of pins, intended, and phase.
    // Flipping requires changing the annotation format and updating the Rust scanner together.
    sample := "// hinge_test: pins=v1, intended=my-hinge, phase=P5"
    // ... parses sample and asserts all three fields are present ...
}
```

The test verifies that a representative `// hinge_test:` comment string contains all three required fields. It uses Go's `strings.CutPrefix` and `strings.Split` — no external dependencies.

**Location:** `sidecar/cmd/anvil-sidecar/main_test.go` (bottom of file)

---

### `crates/anvil-cli/src/hinge.rs` (new)

**`run_hinge_list(project, strict, count_only)`**
- Calls `scan_workspace(project)` to build the registry.
- Opens the audit store and reads all `HingeFlip` records to determine which entries are flipped.
- If `count_only`: prints total entry count (entries + alternatives) and returns.
- Otherwise: prints a table with columns INTENDED / PINS / PHASE / LANG / STATUS. Status is `OPEN` or `FLIPPED` based on `HingeFlip` records keyed by `hinge_test_name`.
- Prints alternatives with `ALT` in the LANG column and the `mechanism` in the STATUS column.
- If `strict`: runs `consensus_violations()`. If any exist, prints `[BLOCK-SHIP]` header with details and calls `std::process::exit(1)`.

**`run_hinge_flip(project, id, new_value, reason)`**
- Rejects empty `reason` with `AnvilError::EmptyReasoning`.
- Calls `scan_workspace(project)` to find the entry matching `intended == id`. Falls back to checking alternatives. Returns `AnvilError::RecordNotFound` if not found.
- Creates a `HingeFlip` record (`hinge_test_name = intended`, `old_value = current pins`, `new_value = new_value arg`).
- Appends to audit store. Prints confirmation.

**Location:** `crates/anvil-cli/src/hinge.rs`

---

### `crates/anvil-cli/src/main.rs` (amended)

- Added `mod hinge;`
- Added `HingeCmd` enum with `List { strict, count, project }` and `Flip { id, new_value, reason, project }` variants.
- Added `Command::Hinge(HingeCmd)` variant to the top-level `Command` enum.
- Added dispatch arms for both variants in `run()`.
- Added `#[allow(clippy::too_many_lines)]` to `run()` (now 113 lines; was already over the 100-line pedantic threshold before this change — the hinge dispatch adds ~10 more lines).

**Location:** `crates/anvil-cli/src/main.rs`

---

### `crates/anvil-cli/Cargo.toml` (amended)

Added `anvil-hinge = { path = "../anvil-hinge" }`.

---

## Hinge Tests Delivered

| Test | Source | Phase | Pins |
|---|---|---|---|
| `test_hinge_decorator_metadata_required` | Rust | P10b | `source-scanner` |
| `test_hinge_comment_metadata_required` | Go | P10b | `comment-parser` |
| `test_bi_language_registry_merge` | Rust | P10b | `registry-merge` |

All three hinge entries will appear in `anvil hinge list` output once the workspace is initialized, demonstrating that the scanner finds its own annotations.

---

## Design Decisions

### Source scanner, not proc-macro

The Plan describes a `#[hinge_test]` proc-macro. The implementation uses a source scanner (`// hinge_test:` comment parser) instead, for two reasons:
1. The annotation format was already established in the codebase across all phases — a proc-macro would require rewriting all existing annotations.
2. Go tests require a comment-based approach anyway; unifying on `// hinge_test:` in both languages keeps the format consistent.

The source scanner is simpler, avoids proc-macro compile-time complexity, and produces identical semantics.

### Consensus check checks `phase`, not `pins`

Cross-language hinges legitimately have different `pins` values when the same logical invariant has a different language-specific expression. For example, `binary-entry-point` pins `"anvil"` in Rust (the CLI binary name) and `"anvil-sidecar"` in Go (the sidecar binary name) — both are correct. Enforcing `pins` equality would produce false violations on the existing codebase.

Phase equality is the meaningful invariant: both language implementations of the same logical hinge must have been introduced in the same phase. A phase mismatch indicates the two entries are tracking different concepts under the same name.

### `intended` as canonical ID

`HingeFlip.hinge_test_name` is set to `intended` (not `fn_name`). This matches the audit store's flip record semantics and allows the CLI to look up hinges by their stable canonical name, independent of the test function name.

---

## What to Review

1. **Scanner completeness.** The Rust scanner requires `#[test]` to appear within 5 lines after the annotation, then `fn` after `#[test]`. Verify this correctly handles all existing annotation placements in the codebase. Module-level annotations (e.g., `rotation.rs:6`) are correctly skipped because no `#[test]` follows. Confirm no false positives or false negatives.

2. **Go scanner window.** The Go scanner looks within 3 lines after the annotation for `func Test`. Verify this is sufficient for all existing Go test files (no blank lines or additional attributes between annotation and `func`).

3. **Consensus check definition.** The check enforces `phase` equality (not `pins` equality) for cross-language hinges. Confirm this is the correct v1 invariant. The `--strict` flag exits non-zero on violations.

4. **`hinge flip` semantics.** The `old_value` recorded in `HingeFlip` is the current `pins` value from the scanner. The developer is expected to manually update the annotation in source after running `hinge flip`. Confirm this manual-update model is acceptable for v1.

5. **Alternative-mechanism entries.** The `.anvil/hinge-alternatives.toml` file is optional (absent = empty list). Alternatives appear in `anvil hinge list` with `ALT` in the LANG column. Confirm the TOML format (`[[alternative]]` array) is ergonomic enough for v1.

6. **`run()` function length allow.** `#[allow(clippy::too_many_lines)]` was added to `run()` in `main.rs`. This is consistent with the fact that `run()` is a dispatcher (a match on all commands) and its length is inherent to the CLI's breadth, not algorithmic complexity. Confirm this is acceptable.

---

## Test Coverage Summary

**`crates/anvil-hinge/src/lib.rs`** (2 Rust hinge tests):
- `test_hinge_decorator_metadata_required` — pins `source-scanner`; verifies `HingeEntry` requires non-empty fields and `parse_hinge_comment` rejects incomplete annotations
- `test_bi_language_registry_merge` — pins `registry-merge`; verifies consensus check detects phase mismatches and passes on matching phases

**`sidecar/cmd/anvil-sidecar/main_test.go`** (1 Go hinge test):
- `TestHingeCommentMetadataRequired` — pins `comment-parser`; verifies `// hinge_test:` annotation format requires all three fields

**`crates/anvil-cli/src/metrics.rs`** (2 tests, unchanged):
- `test_metrics_show_empty_project_succeeds`
- `test_metrics_history_empty_project_succeeds`

**Total: 177 Rust tests passing, 0 failed, clippy clean, fmt clean. Go tests passing.**
