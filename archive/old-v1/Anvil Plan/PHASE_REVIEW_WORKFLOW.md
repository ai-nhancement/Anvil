# Phase Review Workflow

**Author:** John Canady Jr.
**Date:** 2026-05-19
**Status:** Canonical — applies to all multi-phase implementation work in this repo

---

## Overview

All architecture-shaped changes (new features, refactors, migrations) follow a phased build + outside-review cycle before committing. One-line bug fixes skip this and commit directly.

---

## Per-Phase Steps

1. **Code the phase** according to the IP/ plan doc.
   - Implement all deliverables listed for the phase.
   - Write tests that cover the success criteria.
   - All tests must pass before writing the review doc.

2. **Write R1 review doc** at repo root: `REVIEW_<TOPIC>_R1.md`
   - What was built (file table with action + purpose).
   - Architecture decisions and why.
   - Open questions and risks surfaced by the implementation.
   - Test coverage summary.
   - How to activate / verify if applicable.

3. **User runs outside review** on R1.
   - Findings come back categorized (High / Medium / Low / Improvement).
   - High = must fix before proceeding; Medium = should fix; Low = fix if cheap; Improvement = optional.

4. **Apply fixes** from R1 findings.
   - Each High/Medium finding gets a fix and a test pinning the fix.

5. **Write R2 review doc** at repo root: `REVIEW_<TOPIC>_R2.md`
   - For each R1 finding: restate the finding, describe the fix, cite the test that pins it.
   - Note any findings deliberately deferred and why.

6. **Apply fixes from R2** as needed.
   - If R2 opens new High/Medium items, continue to R3 with the same pattern.

7. **Approved → commit → proceed to next phase.**
   - Commit covers: implementation files, tests, review docs.

---

## Review Doc Format

```
# <Feature> — <Phase N> Review Briefing (R1 / R2 / ...)

**Date:** YYYY-MM-DD
**Scope:** One-sentence description of what this phase covers
**Plan spec:** IP/<PLAN>.md §<section>
**Tests:** tests/<test_file>.py (N tests, all passing)
**Status:** <e.g. "No runtime consumers changed — purely additive">

---

## What Was Built

| File | Action | Purpose |
|---|---|---|
...

---

## Architecture Decisions
...

## Phase N Success Criteria (from plan)
| Criterion | Status |
...

## What to Review
(numbered list of concrete questions / risks to evaluate)

---

## Test Coverage Summary
...
```

For R2+: replace "What to Review" with a "Findings from R1" table:
```
| Finding | Severity | Fix Applied | Test |
|---|---|---|---|
```

---

## File Placement

| Artifact | Location |
|---|---|
| IP plan docs | `IP/<PLAN_NAME>.md` |
| Review docs | `C:\AiMe\REVIEW_<TOPIC>_R1.md`, `R2.md`, ... (repo root) |
| Tests | `tests/test_<feature_phase>.py` |

Review docs do **not** go in `IP/review_rounds/`, worktrees, or module directories.

---

## What Triggers the Full Workflow

- New multi-phase feature (new subsystem, new specialist, new pipeline stage)
- Refactor that touches multiple files across module boundaries
- Migration that changes a contract used by 3+ consumers
- Any change to SBA spine contracts, living memory schema, or authority rules

## What Skips the Full Workflow

- One-line bug fix (typo, missing import, off-by-one)
- Config value change with no logic change
- Doc-only change
- Test-only addition that adds coverage without changing production code

When in doubt: use the workflow.

---

## Example Naming

| Feature | Phase | R1 doc | R2 doc |
|---|---|---|---|
| Model Route Migration | Phase 1 (schema) | `REVIEW_ROUTE_MIGRATION_PHASE1_R1.md` | `REVIEW_ROUTE_MIGRATION_PHASE1_R2.md` |
| Response Specialist | Phase 1.5 | `REVIEW_RESPONSE_SPECIALIST_PHASE1_5_R1.md` | `..._R2.md` |
| Presence Vision | (single phase) | `REVIEW_PRESENCE_VISION_R1.md` | `..._R2.md` |
