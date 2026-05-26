# Anvil — P6 Multi-Reviewer Rotation + Convergence Safeguards R1 Findings

**Source review doc:** `REVIEW_P6_MULTI_REVIEWER_ROTATION_R1.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Pass**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass — 84 tests, 0 failures**

Note: the requested review doc was not under `C:\Anvil\Review Rounds`; it was found at `C:\Anvil\REVIEW_P6_MULTI_REVIEWER_ROTATION_R1.md`. I placed this findings file beside the source review doc using the requested filename convention.

I did not run real model/sidecar end-to-end flows because they require configured provider credentials and an installed/running sidecar.

---

## 1. High — `Drop-Advisory` and `Defer-Advisory` allow empty required context and still pass the advisory gate

**Location:**

- `crates/anvil-cli/src/charter.rs:392-404`
- `crates/anvil-core/src/pipeline.rs:387-410`
- `Anvil Plan/ANVIL_PLAN.md:635`

**Problem:**

The Plan requires:

```text
Drop-Advisory (finding refuted or non-applicable; reason required)
Defer-Advisory (deferred to a named future phase; target phase required)
```

The CLI prompts for this context but allows empty input:

```rust
let note: String = Input::new()
    .with_prompt(prompt)
    .allow_empty(true)
    .interact_text()
```

Then the advisory gate only checks whether `advisory_disposition.is_some()`:

```rust
.any(|d| d.finding_id == f.id && d.advisory_disposition.is_some())
```

So a coordinator can choose `Drop-Advisory` or `Defer-Advisory`, submit an empty reason/target, and the gate passes.

**Impact:**

- P6 acceptance criterion 4 is only partially satisfied: the disposition enum persists, but required disposition data may be absent.
- Audit records can claim an advisory finding was dropped/deferred without the required human rationale or target phase.
- The convergence safeguard is weakened because advisory findings can effectively pass with an unreasoned disposition.

**Suggested fix:**

- For `DropAdvisory`, require non-empty annotation text as the reason.
- For `DeferAdvisory`, require non-empty annotation text as the target phase.
- Enforce this in both the interactive CLI and the core gate/check function, so malformed records cannot pass if constructed outside the CLI.
- Add tests for all three advisory disposition types, including negative cases for empty Drop/Defer context.

---

## 2. High — Full-pool clean does not verify clean passes against the same current artifact state

**Location:**

- `crates/anvil-cli/src/status.rs:166-252`
- `Anvil Plan/ANVIL_PLAN.md:648`
- `Anvil Plan/new_project_charter.md:136`

**Problem:**

The Plan requires full-pool clean on the current artifact state:

```text
Full-pool clean termination requires all pool members to have produced a clean pass on the current artifact state
```

`check_full_pool_clean()` instead selects each reviewer's latest RFP independently:

```rust
let reviewer_rfp = latest_by_reviewer.get(binding_name.as_str());
```

It then considers the pool clean if each reviewer has any latest clean packet, regardless of whether those packets reviewed the same artifact revision/state.

Example failure mode:

1. R1: reviewer-1 reviews charter state A and produces a clean pass.
2. The charter changes to state B.
3. R2: reviewer-2 reviews state B and produces a clean pass.
4. `check_full_pool_clean()` can report full-pool clean even though reviewer-1 never reviewed state B.

Current records do not store a charter content hash or artifact version in a way this function compares for equality across reviewers.

**Impact:**

- The default termination condition can be satisfied by partial-pool coverage of multiple different artifact states.
- This violates the core purpose of multi-reviewer rotation: every pool member must have seen the final/current state.
- P6 acceptance criterion 7 is not met as written.

**Suggested fix:**

- Store the reviewed artifact content hash/version in `ReviewerFindingPacket` or its `artifact_ref` in a stable, comparable form.
- Define “current artifact state” explicitly for Charter now and general artifacts later.
- In `check_full_pool_clean()`, require each pool member's clean pass to match the current artifact state hash/version.
- Add a regression test with two reviewers clean on different artifact versions and assert full-pool clean is **not** satisfied.

---

## 3. High / Medium — Reviewer pool and single-clean-pass override are not configurable through the CLI and are not shown in `config show`

**Location:**

- `crates/anvil-core/src/config.rs:20-27`
- `crates/anvil-cli/src/main.rs:292-347`
- `crates/anvil-cli/src/main.rs:349-380`
- `crates/anvil-cli/src/setup.rs:721-755`
- `Anvil Plan/ANVIL_PLAN.md:649`

**Problem:**

P6 adds config fields:

```rust
pub reviewer_pool: Vec<String>,
pub single_clean_pass_override: bool,
```

But:

- `anvil config set` only supports:
  - `sidecar.idle_timeout_secs`
  - `sidecar.binary_path`
- `anvil config show` does not display `reviewer_pool` or `single_clean_pass_override`.
- `anvil setup` writes `model_bindings` but does not populate `reviewer_pool` with `reviewer-1` / `reviewer-2`.

As a result, the P6 rotation code usually falls back to a single reviewer:

```rust
if config.reviewer_pool.is_empty() {
    vec![ROLE_REVIEWER_1.to_owned()]
}
```

A user can manually edit TOML, but the CLI surface does not support or display the P6 configuration. The Plan specifically requires:

```text
Single-clean-pass override is configurable per project; override is visible in `anvil status` and in the config.
```

**Impact:**

- Multi-reviewer rotation is not practically reachable through the supported CLI setup/config workflow.
- The default behavior after setup remains single-reviewer, despite P6 being “multi-reviewer rotation.”
- The override is visible in `anvil status`, but not in `anvil config show`, so the “visible in the config” acceptance criterion is incomplete.

**Suggested fix:**

- Populate `reviewer_pool` during setup from configured reviewer bindings, e.g. `reviewer-1`, `reviewer-2` when both exist.
- Add `anvil config set reviewer_pool ...` and `anvil config set single_clean_pass_override ...`, or a dedicated `anvil config reviewer-pool` command.
- Display both values in `anvil config show`.
- Add tests for config serialization, `config show` output, and effective rotation using a two-reviewer pool produced by setup/config commands.

---

## 4. High / Medium — Advisory gate runs after writing disposition and hardening-history files

**Location:**

- `crates/anvil-cli/src/charter.rs:560-619`

**Problem:**

`run_charter_findings()` renders and writes the disposition document, then appends hardening history, then performs the advisory gate check:

```rust
std::fs::write(&disp_path, doc.as_bytes())?;
append_charter_hardening_history(...)?;

let missing_advisory = check_advisory_gate(...);
if !missing_advisory.is_empty() {
    return Err(...);
}
```

Today the interactive flow normally assigns `advisory_disposition` for advisory findings, so this may not trigger often. But as a gate/invariant, it is placed too late: if the gate ever fails, the command exits with an error after mutating project files.

**Impact:**

- A failed curation gate can leave a disposition document and hardening-history entry behind without a corresponding `CuratedFindingsRecord`.
- This violates the expected “gate before commit” shape and creates inconsistent file/audit state.
- It also conflicts with the Plan-level trust-boundary principle that invalid outputs should not be committed.

**Suggested fix:**

- Run all curation gate checks before writing any files or appending audit records.
- Treat rendering/writing as the commit phase after validation succeeds.
- Add a regression test or extracted helper test proving gate failure produces no file/hardening/audit side effects.

---

## 5. Medium / High — `resolve-finding` accepts arbitrary IDs and does not verify the target finding exists

**Location:**

- `crates/anvil-cli/src/arbiter.rs:69-112`
- `crates/anvil-cli/src/status.rs:215-220`
- `Anvil Plan/ANVIL_PLAN.md:637`

**Problem:**

The docs say `finding_id` must be composite:

```text
<packet_id>:<finding_id>
```

But `run_resolve_finding()` accepts any non-empty reason and stores whatever `finding_id` string was provided:

```rust
let record = ArbiterFindingResolution::new(
    finding_id.to_owned(),
    ...
);
```

It does not:

- validate that the string has exactly the expected composite form,
- verify the packet exists,
- verify the finding exists inside that packet,
- verify the finding is on the active artifact.

`check_full_pool_clean()` later ignores a finding only when its generated composite ID exactly matches an arbiter record. A typo therefore produces a successful-looking ArbiterFindingResolution that does not actually resolve anything.

**Impact:**

- Users can create meaningless arbiter records.
- The CLI prints “This finding is excluded from the full-pool clean blocking set” even when the ID does not correspond to any finding.
- Audit quality and user trust are reduced.

**Suggested fix:**

- Parse and validate `<packet_id>:<finding_id>` before appending.
- Load the referenced `ReviewerFindingPacket` and assert the finding exists.
- Consider requiring the target finding to belong to the active artifact/current review scope.
- Add negative tests for malformed IDs, unknown packet IDs, and unknown finding IDs.

---

## 6. Medium — `anvil status` and convergence declaration counts are not scoped to the requested active artifact

**Location:**

- `crates/anvil-cli/src/status.rs:40-67`
- `crates/anvil-cli/src/arbiter.rs:37-48`
- `Anvil Plan/ANVIL_PLAN.md:638`

**Problem:**

The Plan says status shows counts “on the active artifact.” The current status and convergence code counts all records of a type:

```rust
let rfp_entries = store.list(RecordType::ReviewerFindingPacket)?;
let conv_entries = store.list(RecordType::ConvergenceDeclaration)?;
let arbiter_entries = store.list(RecordType::ArbiterFindingResolution)?;
```

There is no artifact parameter to `anvil status`, and `run_declare_convergence(artifact, ...)` also derives `round_count`, advisory count, and arbiter count from all RFP / arbiter records rather than filtering by the supplied artifact.

This is mostly hidden while only Charter review exists, but P6 is the shared convergence machinery for later Plan and Build stages.

**Impact:**

- Counts will become incorrect once Plan or phase reviews create their own RFP/VR/curation records.
- A convergence declaration for one artifact can record round/advisory/arbiter counts from another artifact.
- This weakens the audit value of `ConvergenceDeclaration` and `anvil status`.

**Suggested fix:**

- Add an artifact parameter or active-artifact context to `anvil status`.
- Filter RFP, curated, convergence, and arbiter records by canonical cross-reference/artifact reference.
- Make `run_declare_convergence(artifact, ...)` count only records for that artifact.
- Add tests with two artifacts and assert counts remain isolated.

---

## 7. Medium — Acceptance criterion for reviewer briefing of Arbiter-Decided findings is not implemented

**Location:**

- `crates/anvil-cli/src/charter.rs:140-151`
- `Anvil Plan/ANVIL_PLAN.md:651`

**Problem:**

P6 acceptance criterion 10 says:

```text
Reviewers receive Arbiter-Decided findings in their input briefing with explicit flag; re-raising the same direction-of-finding does not change the ship-gate status
```

`run_charter_review()` sends only the charter content and round number:

```rust
let user_message = format!(
    "Please review the following Charter document (round {round_number}):\n\n{charter_content}"
);
```

It does not load `ArbiterFindingResolution` records or include Arbiter-Decided findings in the reviewer prompt/briefing.

**Impact:**

- Reviewers are not told which findings have already been arbiter-decided.
- Re-raising the same direction may happen because the reviewer lacks the required context.
- P6 acceptance criterion 10 is not satisfied.

**Suggested fix:**

- Include relevant `ArbiterFindingResolution` records in the reviewer prompt/briefing for the active artifact.
- Clearly label them Arbiter-Decided and explain the ship-gate semantics.
- Add an integration-style test or prompt-construction unit test showing arbiter-decided context appears in the reviewer input.

---

## 8. Medium — P3 is treated as blocking in rounds 1–5 despite the artifact severity spec saying P3 is advisory in all rounds

**Location:**

- `crates/anvil-core/src/pipeline.rs:12-14`
- `crates/anvil-core/src/pipeline.rs:372-385`
- `crates/anvil-cli/src/status.rs:215-227`
- `Anvil Plan/ARTIFACT_SPECIFICATIONS.md:47-48`

**Problem:**

The artifact severity vocabulary says:

```text
P3 ... Advisory in all rounds.
```

But `apply_severity_tiering()` marks P2/P3 advisory only in rounds 6+:

```rust
if is_advisory_round(round_count) {
    if matches!(finding.severity, FindingSeverity::P2 | FindingSeverity::P3) {
        finding.advisory = true;
    }
}
```

And `check_full_pool_clean()` treats any non-advisory finding as blocking, regardless of severity:

```rust
if f.advisory { return false; }
true
```

Therefore P3 findings in rounds 1–5 block full-pool clean unless manually resolved by arbiter.

There is some tension in the Plan text: the P6 section says “After round 5, P2 and P3 findings are marked advisory,” but the artifact severity spec explicitly says P3 is advisory in all rounds. The implementation chose the stricter P6 reading, while its own comment says “P3 is advisory in all rounds.”

**Impact:**

- P3 style/cosmetic findings may block convergence during early rounds, contrary to the Artifact Specifications severity table.
- The system may over-iterate on non-blocking cosmetic findings.
- Reviewers and coordinators receive inconsistent semantics for P3.

**Suggested fix:**

- Decide the normative rule: either P3 is advisory in all rounds, or only advisory after round 5.
- If P3 is always advisory, set `advisory = true` for P3 findings in all rounds and update tests.
- If P3 blocks in rounds 1–5, update `ARTIFACT_SPECIFICATIONS.md` and misleading code comments to remove the contradiction.

---

## 9. Medium / Low — Advisory dispositions are not represented clearly in rendered disposition documents

**Location:**

- `crates/anvil-core/src/render.rs:165-185`
- `crates/anvil-cli/src/charter.rs:377-410`

**Problem:**

P6 adds `AdvisoryDispositionType` to `CurationDisposition`, but `render_disposition_doc()` only receives `curation_actions` and `disposition_map`.

For advisory findings:

- `Accept-Advisory` maps to action `Keep` but no normal disposition label is inserted, so the rendered table can show `—`.
- `Defer-Advisory` also maps to action `Keep`, and again can render as `—`.
- `Drop-Advisory` maps to action `Drop`, so it renders as `Dropped`, losing the advisory-specific meaning and required reason.

The persisted audit record has more detail than the human-facing disposition document.

**Impact:**

- The disposition document may not show the explicit advisory disposition required by P6.
- Human reviewers reading only the disposition doc cannot reliably see whether advisory findings were accepted, dropped, or deferred.
- The audit record and rendered artifact can diverge in semantic clarity.

**Suggested fix:**

- Pass advisory disposition data into `DispositionInput` or derive a display label before rendering.
- Render explicit labels: `Accept-Advisory`, `Drop-Advisory: <reason>`, `Defer-Advisory: <target phase>`.
- Add render tests for all three advisory disposition types.

---

## 10. Low / Medium — RotationLog for round 1 records a self-rotation

**Location:**

- `crates/anvil-cli/src/charter.rs:102-108`
- `crates/anvil-cli/src/charter.rs:218-226`

**Problem:**

For round 1, `prev_reviewer` is set to the selected reviewer:

```rust
let prev_reviewer = if round_number > 1 { ... } else { reviewer_binding_name.clone() };
```

The resulting `RotationLog` says the rotation went from reviewer-1 to reviewer-1, even though there was no prior reviewer.

**Impact:**

- The rotation path is technically auditable, but the first transition is misleading.
- Consumers cannot distinguish “first review round” from “same reviewer selected twice.”

**Suggested fix:**

- Represent no previous reviewer explicitly, e.g. `rotated_from = "<none>"` or add `previous_reviewer: Option<String>` in a future schema version.
- Add a test for the first-round rotation log semantics.

---

## 11. Low — Review document is outside `Review Rounds/`, unlike prior review artifacts

**Location:**

- `C:\Anvil\REVIEW_P6_MULTI_REVIEWER_ROTATION_R1.md`
- `C:\Anvil\Review Rounds\` prior review files

**Problem:**

The requested R1 review document was found at the repository root, not in `Review Rounds/` where prior review docs and findings files live.

**Impact:**

- Review artifacts are harder to discover consistently.
- Automation that scans `Review Rounds/` may miss this P6 review.

**Suggested fix:**

- Move the P6 review doc and this findings doc into `Review Rounds/`, or standardize future review artifact location explicitly.

---

## Overall Assessment

The code is format-, clippy-, and test-clean, and several P6 building blocks are present: rotation arithmetic, advisory flagging in rounds 6+, new audit record fields, arbiter commands, and status output.

However, I would not approve P6 R1 yet. The main blockers are semantic rather than validation-gate failures:

1. Advisory `Drop`/`Defer` dispositions can omit required context and still pass.
2. Full-pool clean does not prove all reviewers reviewed the same current artifact state.
3. Multi-reviewer configuration is not exposed through setup/config, so the default CLI path remains effectively single-reviewer.
4. The advisory gate runs after file side effects.
5. Arbiter finding resolution accepts arbitrary IDs without target validation.

Minimum recommended before approval:

1. Enforce required annotation/target for `Drop-Advisory` and `Defer-Advisory` in core gate logic and tests.
2. Add artifact-state/version tracking and require full-pool clean on the same current state.
3. Expose `reviewer_pool` and `single_clean_pass_override` through setup/config and display them in `config show`.
4. Move advisory gate checks before any file or audit side effects.
5. Validate `resolve-finding` targets an existing finding.
6. Scope status/convergence counts by active artifact.
