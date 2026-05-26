# Anvil — P7 Plan Stage Pipeline R2 Findings

**Source review doc:** `Review Rounds/REVIEW_P7_PLAN_STAGE_PIPELINE_R2.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Pass**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass** (119 tests)

The R2 validation claim is accurate. I verified the specific R2 changes: `.rev().any()` is present in the Charter gate, `parse_planner_contract` exists and is tested, `PhaseDepGraph::dangling_deps()` exists and is tested, and `render_plan_doc` no longer carries `#[allow(clippy::too_many_lines)]`.

---

## 1. High — Charter approval gate is still not tied to the current `charter.md` artifact state

**Location:**

- `crates/anvil-cli/src/plan.rs:97-114`
- `crates/anvil-cli/src/plan.rs:116-123`
- `Review Rounds/REVIEW_P7_PLAN_STAGE_PIPELINE_R2.md:14`
- `Anvil Plan/ANVIL_PLAN.md:673`

**Problem:**

R2 changes the gate from iterating forward to iterating backward:

```rust
let charter_approved = conv_entries.iter().rev().any(|e| {
    ... r.phase_id == "charter.md"
});
```

This checks that some `ConvergenceDeclaration` exists for `charter.md`, scanning latest records first. But it still does not bind that declaration to the actual current `charter.md` contents that are read immediately afterward and sent to the Planner.

A stale-approval failure mode remains:

1. Charter reaches convergence and a `ConvergenceDeclaration` for `charter.md` is appended.
2. `charter.md` is edited manually or by another command after convergence.
3. `anvil plan invoke` finds the old declaration and passes the gate.
4. The Planner receives the modified, post-declaration Charter as if it were approved.

The R2 disposition says this is refuted because “once a charter is converged in an append-only store there is no mechanism to un-converge it.” That does not address filesystem state drift: the artifact file can change without appending a new convergence record. P6 already introduced artifact hashes for same-state review checks; the Plan-stage gate needs a similar current-state check.

**Impact:**

- P7 acceptance criterion 1 is only partially met: the command exits non-zero when no declaration exists, but not when the current Charter differs from the approved/converged state.
- The Planner can produce a Plan from unapproved Charter text.
- Audit trust is weakened because the Plan may claim to derive from an approved Charter while actually consuming a later unreviewed edit.

**Suggested fix:**

- Record the approved Charter artifact hash or version in, or alongside, the `ConvergenceDeclaration`.
- On `anvil plan invoke`, compute the current `charter.md` hash and require it to match the approved declaration's artifact hash/version.
- If older declarations lack hashes, surface a warning or require re-declaration depending on the migration policy.
- Add a regression test: append a convergence declaration for Charter state A, mutate `charter.md` to state B, and assert `run_plan_invoke` fails before invoking the sidecar.

---

## 2. High / Medium — `run_plan_invoke` erases typed Planner Contract validation errors

**Location:**

- `crates/anvil-cli/src/plan.rs:169-178`
- `crates/anvil-core/src/plan.rs:112-132`
- `Anvil Plan/ANVIL_PLAN.md:674`

**Problem:**

R2 adds `parse_planner_contract`, which correctly returns typed errors such as `AnvilError::PhaseMissingField { phase_id, field }`.

However, `run_plan_invoke` immediately maps every parser/validator error into a generic `AnvilError::Io`:

```rust
let contract = anvil_core::plan::parse_planner_contract(contract_json).map_err(|e| {
    eprintln!("error: Planner Contract invalid: {e}");
    AnvilError::Io(std::io::Error::other(
        "Planner Contract validation failed — fix the phase fields and re-invoke",
    ))
})?;
```

So the CLI return path no longer carries the typed `PhaseMissingField` error. The field/phase detail is printed to stderr as a side effect, but the actual error surfaced through `run_plan_invoke` and `main` is generic.

This conflicts with P7 acceptance criterion 2:

```text
missing fields produce typed errors naming the field and phase
```

The core helper satisfies this, but the primary P7 command path does not.

**Impact:**

- Callers, tests, and future structured output cannot inspect the typed validation failure.
- The final CLI error printed by `main` loses the phase and field name.
- R2's “field-level error” claim is true for `parse_planner_contract` in isolation but not for `anvil plan invoke` as a user-facing command.

**Suggested fix:**

- Let `parse_planner_contract(contract_json)?` propagate its original `AnvilError` instead of wrapping it in `Io`.
- If an additional human-readable hint is desired, add context without discarding the typed variant, or print the hint separately while returning the original error.
- Add a `run_plan_invoke`-level test or extracted helper test proving a missing phase field returns `AnvilError::PhaseMissingField` with the expected `phase_id` and `field`.

---

## 3. Medium — Plan consolidation mutates files before the provenance audit record is safely appended

**Location:**

- `crates/anvil-cli/src/plan.rs:588-605`
- `Anvil Plan/ANVIL_PLAN.md:677`

**Problem:**

`run_plan_consolidate` writes the consolidated Plan and clears `PLAN_HARDENING_HISTORY.md` before opening the audit store and appending `PlanConsolidationRecord`:

```rust
std::fs::write(&plan_path, consolidated.as_bytes())?;
std::fs::write(&history_path, b"")?;

let store = AuditStore::open(project_root)?;
...
store.append(&record)?;
```

If `AuditStore::open` or `store.append` fails after the file writes, the project is left in a partially committed state: the Plan has been changed and the hardening history cleared, but no `PlanConsolidationRecord` exists to preserve the prior Plan snapshot.

This is the same class of “gate/audit before commit” issue previously caught in the Charter findings flow.

**Impact:**

- P7 acceptance criterion 5 can fail under I/O/audit-store error conditions: the prior version may not remain queryable even though the Plan file was consolidated.
- Hardening notes can be erased without an audit record proving what was absorbed.
- Append-only provenance is weakened by non-atomic file/audit side effects.

**Suggested fix:**

- Open and validate audit-store availability before mutating files.
- Avoid clearing hardening history until the provenance record is guaranteed durable.
- Consider a safer commit sequence using temporary files and a clearly defined recovery behavior if the audit append or file replacement fails.
- Add a regression test that simulates audit-store append/open failure and asserts the Plan and hardening history are not modified.

---

## 4. Medium — P7 commands use `ANVIL_PLAN.md` while project initialization and artifact specs use `plan.md`

**Location:**

- `crates/anvil-cli/src/plan.rs:80-81`
- `crates/anvil-cli/src/plan.rs:185-186`
- `crates/anvil-cli/src/plan.rs:213-214`
- `crates/anvil-core/src/project.rs:67-78`
- `Anvil Plan/ANVIL_PLAN.md:258-264`
- `Anvil Plan/ARTIFACT_SPECIFICATIONS.md:289`

**Problem:**

The P7 pipeline defines the default plan artifact as:

```rust
pub const DEFAULT_PLAN_FILE: &str = "ANVIL_PLAN.md";
```

and `plan invoke`, `plan review`, `plan findings`, and `plan consolidate` all operate on `ANVIL_PLAN.md`.

But initialized projects create an empty `plan.md` placeholder:

```rust
for name in &["charter.md", "plan.md", ...]
```

The project layout in the Plan also lists:

```text
plan.md                           # Project Plan
```

and the artifact specifications mention the conventional path:

```text
<project>/plan.md
```

`ANVIL_PLAN.md` is appropriate for the Anvil repository's own implementation plan, but the v1 user project layout appears to standardize on `plan.md`. The current implementation leaves `plan.md` unused/stale and writes a separate `ANVIL_PLAN.md` file.

**Impact:**

- User projects end up with two plan-looking files: an empty `plan.md` from initialization and a generated `ANVIL_PLAN.md` from P7.
- Downstream commands or users following the documented project layout may inspect or edit the wrong file.
- Cross-reference keys for Plan review use `ANVIL_PLAN.md`, diverging from the documented `plan.md` artifact path.

**Suggested fix:**

- Decide whether the user-project Plan artifact is `plan.md` or `ANVIL_PLAN.md`.
- If the intended generic project artifact is `plan.md`, change P7 commands and cross-refs to use `plan.md`.
- If `ANVIL_PLAN.md` is intentional, update `project::init`, the Plan layout, and Artifact Specifications to remove or explain the `plan.md` placeholder.
- Add a test that `anvil init` and `anvil plan invoke/review/consolidate` agree on the same plan path.

---

## 5. Low / Medium — Dangling dependencies are surfaced in the library but not in the CLI graph output

**Location:**

- `crates/anvil-graph/src/phase_graph.rs:75-80`
- `crates/anvil-cli/src/graph.rs:19-40`
- `crates/anvil-cli/src/graph.rs:51-69`
- `Review Rounds/REVIEW_P7_PLAN_STAGE_PIPELINE_R2.md:16`

**Problem:**

R2 adds `PhaseDepGraph::dangling_deps()`, which is useful and tested. But the user-facing graph commands never call it:

```rust
let graph = PhaseDepGraph::build_from_contract(&contract);
```

`run_graph_show` prints phases and dependencies, and `run_graph_blast_radius` prints dependents, but neither warns when `graph.dangling_deps()` is non-empty.

As a result, dangling dependencies are no longer invisible to library consumers, but they remain easy to miss from the CLI surface that P7 explicitly exposes for graph queries.

**Impact:**

- A user can run `anvil graph show` on a malformed Planner Contract and receive no explicit warning that some dependency IDs do not correspond to real phases.
- The R2 fix only partially addresses the operational issue from R1: diagnostics are available programmatically but not presented to normal users.

**Suggested fix:**

- In `run_graph_show` and `run_graph_blast_radius`, print a warning when `graph.dangling_deps()` is non-empty.
- Consider exiting non-zero for dangling dependencies if the Planner Contract is expected to be valid before graph use.
- Add CLI-level tests or output-capture tests for a contract containing a dangling dependency.

---

## 6. Low — Minor stale comments/docs after P7 record and graph changes

**Location:**

- `crates/anvil-audit/src/records.rs:6`
- `crates/anvil-cli/src/graph.rs:12-14`

**Problem:**

Two comments are now stale:

- `RecordType` is documented as “All 14 audit record types,” but `ALL_RECORD_TYPES` now contains 15 entries after `PlanConsolidation`.
- `run_graph_show` says it loads from the audit store / latest `PlanConsolidationRecord` / `ANVIL_PLAN.md` JSON, but the implementation only loads `.anvil/plan_contract.json`.

**Impact:**

- Low runtime risk, but misleading maintenance documentation.
- Future implementers may assume graph commands already consult audit-store consolidation records when they do not.

**Suggested fix:**

- Update the record-type count comment.
- Update the graph command doc comment to match the actual loader, or implement the documented fallback behavior.

---

## Overall Assessment

P7 R2 resolves several R1 findings at the unit/library level and remains clean under fmt, clippy, and tests. The implementation has the main shape required for P7: Planner invocation, contract parsing/validation, Plan rendering, Plan review/curation reuse, consolidation records, and a queryable dependency graph.

However, I would not mark P7 fully converged yet. The main blockers are semantic and operational:

1. The Charter approval gate still accepts stale convergence declarations for changed `charter.md` contents.
2. The main `anvil plan invoke` command discards typed `PhaseMissingField` errors even though the acceptance criterion requires typed field/phase errors.
3. Plan consolidation can mutate files before the provenance audit record is appended.
4. The Plan artifact path is inconsistent between project initialization/specs and P7 commands.

Minimum recommended before approval:

1. Bind Charter convergence to the current `charter.md` artifact hash/version before invoking the Planner.
2. Preserve typed Planner Contract validation errors through `run_plan_invoke`.
3. Make consolidation side effects provenance-safe under audit-store failures.
4. Resolve the `plan.md` vs. `ANVIL_PLAN.md` path convention.
5. Surface dangling dependency diagnostics in the CLI graph commands.
