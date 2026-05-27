# Anvil — P10b Hinge-Test Framework R1 Findings

**Source review doc:** `Review Rounds/REVIEW_P10B_HINGE_FRAMEWORK_R1.md`  
**Review date:** 2026-05-27  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Pass**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass** (177 Rust tests)
- `go test ./...` from `C:\Anvil\sidecar` — **Pass**

The R1 validation claims match the current workspace.

---

## 1. High — `hinge flip --reason` is required but the reason is not persisted in the `HingeFlip` audit record

**Location:**

- `crates/anvil-cli/src/hinge.rs:84-123` (`run_hinge_flip`)
- `crates/anvil-audit/src/records.rs:323-330` (`HingeFlip`)
- `crates/anvil-audit/src/records.rs:611-628` (`HingeFlip::new`)
- `Anvil Plan/ANVIL_PLAN.md:787,797`

**Problem:**

`run_hinge_flip` requires a non-empty `reason`:

```rust
if reason.trim().is_empty() {
    return Err(AnvilError::EmptyReasoning {
        command: "hinge flip",
    });
}
```

But the reason is only printed to stdout:

```rust
println!("Reason: {reason}");
```

The persisted audit record contains only:

```rust
pub struct HingeFlip {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub cross_references: Vec<String>,
    pub hinge_test_name: String,
    pub old_value: String,
    pub new_value: String,
}
```

There is no `reason` / `reasoning` field, so the human justification required by the CLI is lost immediately after command execution.

**Impact:**

- The audit store cannot answer why a hinge was flipped.
- P10b AC5 says `anvil hinge flip <id>` creates a `HingeFlip` audit record with non-empty reasoning; current implementation enforces the CLI argument but does not audit it.
- P10a’s deferred-decision metric can count flips but cannot support review of whether the resolution was justified.

**Suggested fix:**

- Add a non-empty `reasoning` field to `HingeFlip` and populate it from `run_hinge_flip`.
- Preserve the existing empty-reason validation before any I/O.
- Add a regression test that runs or directly constructs a flip and asserts the persisted `HingeFlip` contains the reason.

---

## 2. High — Consensus check is weaker than the Plan: it checks only phase equality, not pinned-value drift or asymmetric cross-language hinges

**Location:**

- `crates/anvil-hinge/src/lib.rs:79-117` (`HingeRegistry::consensus_violations`)
- `Review Rounds/REVIEW_P10B_HINGE_FRAMEWORK_R1.md:52-53,145-149,163`
- `Anvil Plan/ANVIL_PLAN.md:789,796`
- `Anvil Plan/PLAN_HARDENING_HISTORY.md:326-330`

**Problem:**

The Plan’s P10b consensus requirement is explicit:

```text
same hinge name → same pinned value, same intended value, same phase.
Asymmetric states are BlockShip violations.
```

It also names failure modes:

- same hinge in both languages with different pinned values;
- same hinge with different intended states;
- hinge present in one language but missing from the other when metadata flags it as cross-language.

The implementation only checks `phase` equality for `intended` IDs that already appear in both Rust and Go:

```rust
if let Some(go_entry) = go_map.get(intended) {
    if rust_entry.phase != go_entry.phase {
        violations.push(...);
    }
}
```

It intentionally allows `pins` differences and has no cross-language-required metadata, so it cannot detect missing counterparts.

**Impact:**

- Schema drift in cross-language contract hinges can pass strict mode if phases match.
- Missing Go/Rust counterparts for cross-language hinges cannot be reported at all.
- The implementation under-satisfies AC4a and the R4 hardening note unless the Plan is amended to weaken the consensus invariant to phase-only.

**Suggested fix:**

- Either update the Plan to explicitly accept phase-only consensus for v1, or strengthen implementation to match the current Plan.
- If strengthening, add metadata for “cross-language required” hinges and compare all required fields, including `pins` where the hinge declares that pins should match.
- Add tests for pin mismatch, phase mismatch, and missing-counterpart cases.

---

## 3. High / Medium — Ship gate and CI do not invoke the strict hinge consensus check

**Location:**

- `crates/anvil-cli/src/ship.rs:21-67` (`run_project_ship`)
- `.github/workflows/ci.yml:35-50` and `52-111`
- `Anvil Plan/ANVIL_PLAN.md:789`

**Problem:**

The Plan says:

```text
The check runs as part of `anvil hinge list --strict` and is invoked automatically by the Ship gate; CI runs it on every build.
```

Current `anvil ship` only checks phase ship readiness and unresolved rollbacks, then executes transport actions. It does not call `scan_workspace`, `consensus_violations`, or `anvil hinge list --strict`.

The GitHub Actions workflow runs Rust build/test/clippy/fmt/audit and Go build/test/fmt/lint/vulncheck, but contains no hinge strict step.

**Impact:**

- Hinge consensus drift does not block project ship.
- CI does not enforce the P10b R4 hardening invariant.
- `anvil hinge list --strict` exists as an opt-in command, but the Plan requires automatic enforcement.

**Suggested fix:**

- Add a reusable hinge strict check that returns `Result` instead of exiting the process, then call it from `run_project_ship` before transport actions.
- Add a CI step that runs the strict check against the repository workspace.
- Add a ship-gate regression test where a synthetic consensus violation blocks ship.

---

## 4. Medium — `anvil hinge list --strict` requires an initialized audit store before it can run the consensus check

**Location:**

- `crates/anvil-cli/src/hinge.rs:8-82` (`run_hinge_list`)
- `crates/anvil-cli/src/hinge.rs:18-30` (audit store opened before strict check)

**Problem:**

`run_hinge_list` scans the source registry, then opens the audit store to compute flip status before evaluating `strict`:

```rust
let registry = scan_workspace(project)?;
...
let store = AuditStore::open(project)?;
...
if strict {
    let violations = registry.consensus_violations();
    ...
}
```

This means `anvil hinge list --strict --project C:\Anvil` fails in the repository root if it has not been initialized as an Anvil project:

```text
error: no anvil.toml found in C:\Anvil — run `anvil init` first
```

By contrast, `anvil hinge list --count --project C:\Anvil` succeeds and reports `72`, because count mode returns before opening the audit store.

**Impact:**

- The strict source-code consensus check cannot be run on a normal checked-out repository unless it is first initialized as an Anvil project.
- This makes CI integration harder: CI should be able to run the source scanner without requiring project audit-store state.
- It also prevents `--strict` from serving as a pure source consistency check.

**Suggested fix:**

- Run `consensus_violations()` before opening `AuditStore`, or make audit-store flip status optional when `--strict` is requested in source-only contexts.
- Consider separate modes: source registry scan/strict check vs. project flip-status view.
- Add a test or smoke check that `hinge list --strict` can run in a checkout without `anvil.toml`.

---

## 5. Medium — Registry is source-scanned on demand but not persisted to the audit store as specified

**Location:**

- `crates/anvil-hinge/src/lib.rs:259-285` (`scan_workspace`)
- `crates/anvil-cli/src/hinge.rs:8-82` (`run_hinge_list`)
- `Anvil Plan/ANVIL_PLAN.md:781,785,794`

**Problem:**

P10b’s goal and action list specify a unified registry “persisted to the audit store” and AC3 says the bi-language registry “persists across runs.”

The implementation builds a transient in-memory `HingeRegistry` every time by scanning source files:

```rust
pub fn scan_workspace(root: &Path) -> Result<HingeRegistry, AnvilError> { ... }
```

Only flip events are written to the audit store. There is no registry snapshot record, registry index, or audit-store representation of the discovered hinge set.

**Impact:**

- The registry state at the time of a flip or release is not auditable as a snapshot.
- If source annotations later change, historical registry composition cannot be reconstructed from audit records alone.
- The implementation may be acceptable as “persisted in source,” but that is weaker than the Plan’s audit-store persistence wording.

**Suggested fix:**

- Either amend the Plan to define source files as the persistent registry and audit store as flip-history only, or add an audit record/snapshot for registry scans.
- If adding persistence, store registry snapshots at `hinge list --strict`, `hinge flip`, or ship-gate time with enough metadata to reproduce the registry state.

---

## 6. Medium — Duplicate `intended` IDs make flip status and old-value selection ambiguous

**Location:**

- `crates/anvil-cli/src/hinge.rs:20-30` (flipped status keyed only by `hinge_test_name` / `intended`)
- `crates/anvil-cli/src/hinge.rs:98-110` (`run_hinge_flip` selects first matching entry)
- Existing duplicate annotations:
  - `pairing-check` in `crates/anvil-cli/src/charter.rs`
  - `version-provenance` in `crates/anvil-cli/src/plan.rs`
  - `contract-compliance` in `crates/anvil-core/src/plan.rs`
  - `advisory-threshold` in `crates/anvil-core/src/rotation.rs`

**Problem:**

The implementation treats `intended` as the canonical ID. This is fine only if `intended` is unique across the registry. The current source tree contains repeated `intended` values for distinct tests.

`hinge flip <id>` chooses the first matching entry:

```rust
.entries.iter().find(|e| e.intended == id)
```

`hinge list` marks all entries with that same `intended` as `FLIPPED` if any `HingeFlip` exists for the ID:

```rust
let status = if flipped.contains(&entry.intended) { "FLIPPED" } else { "OPEN" };
```

**Impact:**

- Flipping one logical hinge ID can mark multiple tests as flipped, even when their `pins` values differ.
- The `old_value` recorded in `HingeFlip` may come from an arbitrary first matching entry rather than the specific test the user intended.
- Registry merge “without collision” is not enforced.

**Suggested fix:**

- Decide whether duplicate `intended` IDs are allowed.
- If not allowed, make registry scanning report duplicates as strict violations.
- If allowed, use a compound identity such as `(intended, source, fn_name)` or add a separate stable `id` field distinct from semantic `intended`.
- Add tests for duplicate IDs and flip behavior.

---

## 7. Medium / Low — Scanner misses planned top-level test locations and is fragile around common Rust/Go test forms

**Location:**

- `crates/anvil-hinge/src/lib.rs:152-229` (`scan_rust_file`, `scan_go_file`)
- `crates/anvil-hinge/src/lib.rs:259-278` (`scan_workspace`)
- `Anvil Plan/ANVIL_PLAN.md:244` (`tests/hinge` planned layout)

**Problem:**

`scan_workspace` scans only:

- `<root>/crates/**/*.rs`
- `<root>/sidecar/**/*.go`

The Plan’s file layout includes a top-level `tests/hinge` directory, but that path is not scanned.

The scanner is also structurally fragile:

- Rust requires an exact `#[test]` line and then `fn <name>(` within the next five lines; it will miss `#[tokio::test]`, `#[test_case]`, `pub fn`, or tests with additional attributes beyond the small window.
- Go requires `func Test` within three lines after the annotation; comments/build tags between the annotation and function can create false negatives.

**Impact:**

- Future hinge tests placed in the documented top-level test directory will not appear in the registry.
- Legitimate test forms can silently disappear from `anvil hinge list`.
- Because skipped annotations are not reported, false negatives are hard to diagnose.

**Suggested fix:**

- Include top-level `tests/` or specifically `tests/hinge/` in `scan_workspace`.
- Consider widening scanner windows and supporting common test attributes, or emit warnings for parsed hinge comments that do not bind to a test function.
- Add scanner tests for module-level skip, additional attributes, and top-level test files.

---

## 8. Low — `hinge flip` accepts an empty `--new-value`, allowing an invalid pinned state

**Location:**

- `crates/anvil-cli/src/main.rs:134-146` (`HingeCmd::Flip`)
- `crates/anvil-cli/src/hinge.rs:84-123` (`run_hinge_flip`)

**Problem:**

`run_hinge_flip` validates `reason`, but not `new_value`. A user can pass `--new-value ""` or whitespace and create a `HingeFlip` with an empty `new_value`.

This conflicts with `parse_hinge_comment`, which rejects empty `pins`, `intended`, and `phase` fields. If the flip is supposed to represent the new pinned value that the source annotation should later adopt, empty values should not be valid.

**Impact:**

- Audit records can contain unusable new pin values.
- Follow-up source updates become ambiguous.

**Suggested fix:**

- Reject `new_value.trim().is_empty()` before scanning or opening the audit store.
- Add a unit test mirroring the empty-reason validation pattern.

---

## Overall Assessment

The source scanner and CLI are a useful foundation, and all build/test/lint gates are clean. However, R1 under-satisfies several P10b acceptance criteria and Plan hardening requirements: flip reasoning is not audited, strict consensus is weaker than specified, strict checks are not enforced by Ship/CI, and registry persistence is source-only rather than audit-store-backed. These should be resolved or explicitly accepted via Plan amendment before considering P10b complete.
