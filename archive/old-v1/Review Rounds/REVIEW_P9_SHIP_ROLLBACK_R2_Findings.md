# Anvil — P9 Ship + Rollback R2 Findings

**Source review doc:** `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R2.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Pass**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass**

The P9 R2 validation state is clean. The R1 fixes described in the review document are present: `phase reopen --yes` is wired through CLI dispatch, the timestamp comparison has an explanatory invariant comment, project ship readiness combines never-shipped and unresolved-rollback blockers, and the Windows quoted-command transport test exists and passes on this Windows environment.

One note: the R2 document's test-count claim is stale for the current tree. It says 133 tests, but the current workspace run reports 153 non-doc unit tests by crate: audit 17, cli 52, core 49, graph 9, ship 15, sidecar-client 11.

---

## 1. High — Initialized projects still lack the `plan-consolidation` audit directory, so P7 consolidation can fail in real projects

**Location:**

- `crates/anvil-core/src/project.rs:8-28`
- `crates/anvil-audit/src/records.rs:28,50,139`
- `crates/anvil-audit/src/store.rs:104-115`
- `crates/anvil-cli/src/plan.rs:589-601`
- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R2.md:121`

**Problem:**

R2 correctly calls out the `plan-consolidation` directory gap as out of scope, but it is a real runtime bug in the current tree.

`RecordType::PlanConsolidation` exists and maps to the directory name `plan-consolidation`, and `ALL_RECORD_TYPES` includes it. `run_plan_consolidate` creates a `PlanConsolidationRecord` and appends it via the audit store:

```rust
let record = PlanConsolidationRecord::new(...);
store.append(&record)?;
```

But `anvil init` does not create `audit-store/plan-consolidation`:

```rust
pub const LAYOUT_DIRS: &[&str] = &[
    ...
    "audit-store/curated-findings",
    ".anvil",
    ".anvil/run",
    ".anvil/logs",
];
```

`AuditStore::append` does not create a missing record-type directory before opening the file:

```rust
let dir = self.audit_root.join(record.record_type().dir_name());
let file_path = dir.join(format!("{}.json", record.id()));
OpenOptions::new().create_new(true).open(&file_path)?;
```

Therefore, on a project initialized through normal `anvil init`, `run_plan_consolidate` attempts to append into `audit-store/plan-consolidation/<id>.json`, but the directory does not exist. The unit tests miss this because their helper creates directories for every `ALL_RECORD_TYPES` entry rather than using the real project layout.

**Impact:**

- A normal initialized project can fail Plan consolidation when P7/P8/P9 workflows expect PlanConsolidation provenance to exist.
- The P7 acceptance criterion that prior Plan versions remain queryable is broken for real initialized projects unless the user manually creates the missing directory.
- The R2 document is right to track this, but treating it as merely “next cleanup” leaves a known runtime failure in the end-to-end workflow.

**Suggested fix:**

- Add `"audit-store/plan-consolidation"` to `LAYOUT_DIRS` and update `test_project_layout_directories` accordingly.
- Add a regression test that uses `anvil_core::project::init`, then runs/appends a `PlanConsolidationRecord` without creating extra directories manually.
- Consider a broader invariant test that every `RecordType::dir_name()` has a corresponding initialized directory.

---

## 2. High / Medium — Project ship readiness trusts ship gate records alone, not the full phase shipped disposition

**Location:**

- `crates/anvil-ship/src/ship.rs:29-49`
- `crates/anvil-ship/src/ship.rs:81-93`
- `crates/anvil-cli/src/phase.rs:665-673`
- `Anvil Plan/ANVIL_PLAN.md:728`

**Problem:**

`check_all_phases_shipped` determines whether a phase is shipped by looking only for a `GateApproval` with the name `phase-{id}-ship`:

```rust
let gate_name = format!("phase-{phase_id}-ship");
let Some(ship_at) = latest_gate_at(store, &gate_name)? else {
    return Ok(false);
};
```

But `run_phase_ship` writes two records for a completed phase ship:

```rust
store.append(&gate)?;
store.append(&disposition)?;
```

where `disposition` is a `PhaseDisposition` with `disposition = "shipped"`.

Because project-level ship ignores `PhaseDisposition`, a phase can be considered shipped if the gate exists even when the disposition is absent. This can happen through partial failure after the gate append, manual/test-created gates, or any future command that writes a gate without the disposition. The P9 acceptance criterion says `anvil ship` succeeds only when all phases are “in shipped state,” which is stronger than “a gate with this name exists.”

**Impact:**

- Project-level ship can proceed from an incomplete phase-ship audit trail.
- A partial `run_phase_ship` failure after `phase-{id}-ship` gate append but before `PhaseDisposition` append would be treated as shipped by `anvil ship`.
- Metrics that rely on `PhaseDisposition` records can disagree with project ship readiness.

**Suggested fix:**

- Define shipped state as requiring both the latest `phase-{id}-ship` gate and a corresponding `PhaseDisposition { phase_id, disposition: "shipped" }` newer than the latest rollback.
- Alternatively, make `PhaseDisposition` the authoritative shipped-state record and treat `GateApproval` as approval provenance only.
- Add a regression test where a `phase-P1-ship` gate exists without a `PhaseDisposition`; `check_all_phases_shipped` should block.

---

## 3. Medium — `phase reopen --yes` still accepts an empty reason string

**Location:**

- `crates/anvil-cli/src/main.rs:241-250`
- `crates/anvil-cli/src/phase.rs:938-989`
- `crates/anvil-ship/src/rollback.rs:76-89`
- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R2.md:30,119`

**Problem:**

R2 intentionally keeps `--reason` required, which is good. However, there is no validation that the provided string is non-empty or non-whitespace.

`run_phase_reopen` receives the reason and passes it directly into `execute_rollback`:

```rust
anvil_ship::execute_rollback(&plan, &store, reason)?;
```

`execute_rollback` writes it into every `RollbackEvent`:

```rust
RollbackEvent::new(..., reason.to_owned(), vec![])
```

A user or script can still run an effectively unreasoned rollback, e.g. with `--reason ""` or `--reason "   "`, depending on shell quoting. The R2 document explicitly notes that an empty reason would produce an uninformative audit record, but the implementation does not reject it.

**Impact:**

- Rollback audit records can contain empty reasons despite `--reason` being required.
- CI/non-interactive usage can satisfy the CLI shape while losing the audit rationale that makes rollback reviewable.
- This is inconsistent with `arbiter declare-convergence` and `arbiter resolve-finding`, which reject empty reasoning.

**Suggested fix:**

- Reject `reason.trim().is_empty()` in `run_phase_reopen` before computing or writing rollback records.
- Return a typed empty-reason error if possible, or a clear `Io`/domain error naming `phase reopen --reason`.
- Add tests for empty and whitespace-only reasons.

---

## 4. Medium — Rollback writes can leave a partial invalidation set if an append fails mid-loop

**Location:**

- `crates/anvil-ship/src/rollback.rs:61-91`
- `crates/anvil-audit/src/store.rs:104-150`
- `Anvil Plan/ANVIL_PLAN.md:732`

**Problem:**

`execute_rollback` writes one `RollbackEvent` per phase in `plan.all_reset_phases`:

```rust
for invalidated_phase in &plan.all_reset_phases {
    let event = RollbackEvent::new(...);
    store.append(&event)?;
}
```

The function comment acknowledges the risk:

```rust
On failure the store may contain a partial set of records
```

This means a failure during a multi-phase rollback can leave only some affected phases invalidated in the audit store. Because the audit store is append-only, the command cannot remove already-written siblings. A later retry writes another set of records, but until that retry succeeds, project state is partially invalidated.

**Impact:**

- P9 acceptance criterion 5 (“one record per invalidated phase”) is not guaranteed under append failure.
- Project ship readiness and rotation reset can disagree across phases in the same intended rollback blast radius.
- Recovery semantics are not documented for operators beyond the code comment.

**Suggested fix:**

- Preflight all record-type directories and store availability before entering the append loop.
- Consider writing a rollback batch/correlation ID into each `RollbackEvent` so consumers can detect incomplete sibling sets.
- Add a recovery check that flags rollback batches whose `rotation_reset_phases` list names phases without corresponding sibling records.
- At minimum, document the retry/recovery procedure in CLI output when an append fails after partial writes.

---

## 5. Medium / Low — `anvil phase reopen` does not actually trigger Charter/Plan amendment workflow; it only prints a suggestion

**Location:**

- `crates/anvil-cli/src/phase.rs:998-1000`
- `Anvil Plan/ANVIL_PLAN.md:721,735`
- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R2.md:99`

**Problem:**

P9 acceptance criterion 8 says:

```text
Charter/Plan amendment workflow triggered by re-open
```

The implementation does not create an amendment record, gate, task, or required follow-up state. It only prints:

```rust
println!("  1. Amend Charter or Plan if the root cause requires it.");
```

The R1/R2 documents treat this as sufficient, but “print an instruction” is weaker than “trigger workflow.” There is no audit evidence that the Coordinator considered whether a Charter/Plan amendment was required, and no ship blocker if that consideration is skipped.

**Impact:**

- Re-open can proceed without auditable amendment triage.
- Root-cause fixes requiring Charter/Plan changes rely on human memory rather than workflow enforcement.
- The implementation under-satisfies the wording of AC8 unless the Plan is amended to define “triggered” as “display instruction only.”

**Suggested fix:**

- Either implement a lightweight amendment-triage gate/audit record on reopen, or amend AC8 to explicitly state that v1 only prints a Coordinator instruction.
- A minimal implementation could append a `GateApproval` or specific record indicating “amendment triage required/considered” with the rollback reason and affected phases.

---

## 6. Low — R2 review document is stale relative to the current tree

**Location:**

- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R2.md:6,77,125-132`
- Current validation output

**Problem:**

The R2 document says:

```text
Tests: 133 passing (15 in anvil-ship, 52 in anvil-cli)
```

The current workspace has 153 non-doc unit tests by crate:

- `anvil-audit`: 17
- `anvil-cli`: 52
- `anvil-core`: 49
- `anvil-graph`: 9
- `anvil-ship`: 15
- `anvil-sidecar-client`: 11

The document also says no new tests in `anvil-cli/src/ship.rs`, which is true for R2 relative to R1, but the current tree includes several additional phase tests not described in this P9 R2 briefing (`test_phase_ship_preflight_blocks_missing_gates`, `test_phase_ship_blocked_by_stale_briefing`, `test_phase_rotation_uses_round_number_not_round_count`, etc.). Those are beneficial fixes, but the review doc no longer fully describes the code under review.

**Impact:**

- Low runtime risk.
- Review reproducibility is harder because the stated test total does not match the current workspace.
- Future reviewers may miss that P8 follow-up fixes are now present in the tree.

**Suggested fix:**

- Update the R2 document’s test totals and files-changed/test-coverage sections to match the current tree, or explicitly state that additional P8 follow-up fixes landed after the P9 R2 briefing was written.

---

## Overall Assessment

P9 R2 resolves the direct R1 blockers: validation is clean, `phase reopen --yes` exists, rollback timestamp semantics are documented, combined readiness errors surface both categories, and Windows shell quoting has coverage.

I would still hold final convergence until the initialized-project `plan-consolidation` directory bug is fixed, because it is a known real-project runtime failure in the workflow stack. The other findings are smaller but worth addressing or explicitly accepting: project ship should probably require a complete shipped disposition, rollback reasons should be non-empty, and partial rollback write recovery should be specified.
