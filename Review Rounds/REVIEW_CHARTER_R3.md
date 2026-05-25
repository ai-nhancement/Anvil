# Charter — R3 Disposition

**Date:** 2026-05-15
**Scope:** Response to R3 findings on the Anvil Project Charter (post-R2 state). R3 raised twelve findings: 4 × P1, 5 × P2, 3 × P3. Eleven were grounded and accepted; one (P3 #12, encoding noise) was refuted on factual grounds — the cited characters are valid UTF-8 typography explicitly permitted by the Artifact Encoding invariant.
**Spec:** `new_project_charter.md` (substantially revised this round; hardening notes appended to `CHARTER_HARDENING_HISTORY.md` rather than the Charter body).
**Prior rounds:** `REVIEW_CHARTER_R1.md`, `REVIEW_CHARTER_R2.md`.
**R3 reviewer:** `R1-Reviewer-A` returning per deterministic rotation. This is the first time R1-Reviewer-A has seen the post-R2 state.

**Shell assumption for verification commands:** the *Reproducibility* section below uses POSIX shell utilities (`grep`, `awk`). On non-POSIX environments, the same checks can be performed by any tool that supports section-isolated text search; the intent of each command is described inline.

---

## What changed since R2

R3 was structurally the heaviest round so far. The reviewer's central thesis — "the Charter has done good work on philosophy but still defers several contracts the Planner will need to consume" — was correct, and the right response was to lock the abstract contracts now while keeping their implementations Plan-stage.

The largest changes in this round:

- **Five new normative blocks** added to the Charter: Planner Contract (in *Plan* subsection), Ship Abstraction (in *Ship* subsection), Audit-Store Minimum Schema (invariant), Coder Model Pinning (invariant), Rollback / Re-Open (invariant). These are the "must-lock-before-plan" items R3 identified — their abstract semantics are now Charter-level; their implementations remain Plan-stage.
- **New Governance Taxonomy section** between Specialist Roster and Invariants. Every load-bearing rule is now classified: *Immutable Invariant*, *Default Policy*, or *Required Project-Level Choice*. A project cannot enter Plan stage without first locking all Required Project-Level Choices.
- **Evaluation Criteria split into three layers**: Charter-level Product Health, Plan-level Project Success Targets, Runtime Alerts. Each layer has a clear owner and a clear role.
- **Two new risks**: artifact sprawl / retrieval overload, and planner-generated phase coupling. Both flagged with mitigations.
- **Hardening Notes moved out of Charter**: `CHARTER_HARDENING_HISTORY.md` is the new home for round-by-round provenance. The Charter ends at *Bottom Line* + *Provenance and Linked Artifacts*. This both addresses R3 #4 directly and aligns with the Plan Consolidation invariant.
- **Reviewer-independence language normalized**: the four call-sites (Core Architectural Commitment, Charter Review, Specialist Roster, Adversarial Diversity invariant) all now use the floor/default vocabulary introduced in R1.

The two smaller wins: disposition-language precision is now distinguished (Fixed vs. Locked-in-Charter-pending-Plan vs. Refuted), and the Reproducibility section explicitly states its shell assumption.

---

## Verification of R3 claims

R3 had a mix of line-cited claims (most findings) and conceptual claims. Each verifiable claim was checked against the post-R2 Charter state before any R3 edits were applied.

| Finding | Verifiable claim | Verified? | Notes |
|---|---|---|---|
| 1 — Too many planning-critical contracts deferred | Open Items L242–251 contain Planner contract, Ship semantics, Git integration, Audit-store schema, Model versioning, Recovery/rollback | ✓ | All cited items present in pre-R3 Open Items |
| 2 — Governance tension (invariant vs default vs override) | Termination configurable (L128); Diversity floor/default (L155); no governance taxonomy section exists | ✓ | Both configurability call-sites present; no taxonomy section in pre-R3 Charter |
| 3 — Single-writer artifact boundary still incomplete | Cross-Reference Integrity at L186 names the cross-reference requirement; no record types / lifecycle / artifact identity model specified | ✓ | Pre-R3 Charter had the invariant but not the operational schema |
| 4 — Charter mixes normative + review history | Hardening Notes sections at L412 (R1) and L479 (R2) totaled ~110 lines appended after Bottom Line | ✓ | Sections were substantial relative to normative body |
| 5 — Planner role underspecified | Workflow names Planner at L79; decomposition method deferred at L244 | ✓ | Planner had role description but no contract |
| 6 — Ship semantics too broad | Ship at L132 and L243 left open across commit/tag/deploy/hand-off | ✓ | Ship subsection was 3 lines; no abstraction stated |
| 7 — Evaluation criteria mixed product/plan/runtime | Pre-R3 had one bullet list with metric + direction + qualitative threshold; no three-layer split | ✓ | Single list conflated layers |
| 8 — Risks missing artifact sprawl + phase coupling | Pre-R3 had 6 risks; neither sprawl nor phase coupling | ✓ | Both genuinely absent |
| 9 — Reviewer-independence language not aligned | Roster L143 used "different vendor from Coder"; Charter Review L75 used "vendors different from the Interlocutor" — both retained older R1 vocabulary | ✓ | Old call-sites confirmed |
| 10 — R2 disposition overstates closure | R2 disposition used "Fixed" for charter-level commitments whose enforcement is Plan-stage | ✓ | Language precision issue confirmed |
| 11 — Reproducibility commands shell-specific | R2 disposition used `awk` and `grep` without stating POSIX assumption | ✓ | Shell-specific commands; no environment note |
| 12 — Encoding noise visible | L1 of Charter and L1 of R2 disposition contain em-dashes (U+2014) | ✓ but *misinterpreted* | Em-dashes are valid UTF-8 explicitly permitted by Artifact Encoding invariant (added R2 #2.1). Not corruption. See *Corrections* below |

Result: 11/12 findings grounded as claimed; one (P3 #12) verifiable in citation but refuted on framing.

---

## Disposition of R3 findings

R3 reviewer did not assign explicit severity tiers within the body. The Coder assigned severities below based on the structural / load-bearing weight of each finding, matching the reviewer's own labels in the header ("P1 / P2 / P3").

| # | Severity | Finding (one-line) | Disposition |
|---|---|---|---|
| 1 | P1 | Too many planning-critical contracts deferred | **Locked in Charter (enforcement pending Plan).** Five contracts promoted into Charter body: Planner Contract, Ship Abstraction, Audit-Store Minimum Schema, Coder Model Pinning, Rollback / Re-Open. Abstract semantics committed; implementations remain Plan-stage. |
| 2 | P1 | Governance tension: no clear classification of invariant vs default vs override | **Fixed.** New *Governance Taxonomy* section between Specialist Roster and Invariants. Every load-bearing rule classified into one of three buckets. *Required Project-Level Choices* must be locked before Plan stage begins. |
| 3 | P1 | Single-writer artifact boundary still incomplete (no record types, lifecycle, identity) | **Locked in Charter (enforcement pending Plan).** New *Audit-Store Minimum Schema* invariant: required record types, stable cross-reference key, append-only operational definition, creation-before-Ship timing, Ship-block vs. warn behavior. Storage format and runtime mechanism remain Plan-stage. |
| 4 | P1 | Charter mixes normative content with review history | **Fixed.** Hardening Notes (R1, R2) moved out of Charter into `CHARTER_HARDENING_HISTORY.md`. Charter ends at *Bottom Line* + *Provenance and Linked Artifacts*. Aligns with Plan Consolidation invariant. R3 hardening note added to history file. |
| 5 | P2 | Planner role underspecified | **Locked in Charter (enforcement pending Plan).** *Planner Contract* now appears in *The Workflow / Plan*: minimum inputs, outputs, required per-phase fields, phase size rule, treatment of cross-cutting concerns. Heuristics versus learned planning remain Plan-stage. |
| 6 | P2 | Ship semantics too abstract | **Locked in Charter.** *Ship Abstraction* now states: Ship is human approval of the artifact set as canonical output of the phase / project. Transport actions (commit, tag, deploy, hand-off) are Plan-level implementations. |
| 7 | P2 | Evaluation criteria mixed across product / plan / runtime | **Fixed.** Section restructured into three explicit layers: *Layer 1 — Product Health Metrics* (Charter, project-agnostic), *Layer 2 — Project Success Targets* (Plan, numeric thresholds), *Layer 3 — Runtime Alerts* (auto-fired). |
| 8 | P2 | Risks missing artifact sprawl + phase coupling | **Fixed.** Both added with explicit mitigations. Sprawl mitigated via Plan Consolidation + tree-archival in Plan stage. Phase coupling mitigated via Planner-Contract-required dependency declarations + Plan-review for hidden coupling. |
| 9 | P2 | Reviewer-independence language not aligned | **Fixed.** Roster table and Charter Review subsection updated to use floor/default language. All four call-sites (Core Architectural Commitment, Charter Review, Specialist Roster, Adversarial Diversity invariant) now use the same vocabulary. |
| 10 | P3 | R2 disposition overstates closure | **Acknowledged; applied prospectively.** From R3 disposition onward, dispositions distinguish *Fixed* (Charter language updated and operationally enforceable), *Locked in Charter, enforcement pending Plan* (commitment made; runtime check is Plan-stage), and *Refuted* (claim not grounded). R1 and R2 dispositions are not retroactively edited (single-writer artifact discipline). |
| 11 | P3 | Reproducibility commands shell-specific | **Fixed.** R3 disposition header explicitly states POSIX shell assumption. Reproducibility section below describes each check's intent in shell-neutral terms alongside the commands. |
| 12 | P3 | Encoding noise visible | **Refuted on factual grounds.** Cited characters (em-dashes, U+2014) are valid UTF-8 typography explicitly permitted by the *Artifact Encoding* invariant added in R2 (#2.1). The invariant distinguishes intentional typography from corruption (invalid byte sequences). No Charter content change. See *Corrections* below for full reasoning. |

---

## Files changed since R2

| File | Action | Purpose | Approximate delta |
|---|---|---|---|
| `new_project_charter.md` | MODIFY (substantial) | Apply 8 substantive R3 amendments. Insert *Governance Taxonomy* section. Add Planner Contract paragraph block to *Plan* subsection. Add Ship Abstraction paragraph block to *Ship* subsection. Add 3 new invariants (Coder Model Pinning, Audit-Store Minimum Schema, Rollback / Re-Open). Restructure *Evaluation Criteria* into three labelled layers. Add 2 new risks to *Risks and Failure Modes*. Normalize reviewer-independence language in Roster and Charter Review. Rewrite *Open Items* (remove Charter-locked items, add new Plan-stage items). **Remove both Hardening Notes sections** (R1, R2) and replace with a brief *Provenance and Linked Artifacts* pointer block. | +~200 lines net (gains in body, losses from history removal) |
| `CHARTER_HARDENING_HISTORY.md` | CREATE | New file containing R1, R2, and R3 hardening notes (consolidated summaries with disposition pointers). The Charter no longer accumulates round notes inline. | ~250 lines |
| `REVIEW_CHARTER_R3.md` | CREATE | This document. | ~250 lines |
| `REVIEW_CHARTER_R1.md` | (UNTOUCHED) | Per single-writer artifact discipline. |
| `REVIEW_CHARTER_R2.md` | (UNTOUCHED) | Per single-writer artifact discipline. |

---

## Corrections to prior dispositions

Per the single-writer artifact discipline, prior disposition documents are not edited. Corrections live in the superseding disposition.

### R2 disposition — "Fixed" language overuse (R3 #10)

R3 correctly observed that R2 used "Fixed" for several findings whose Charter language was updated but whose enforcement remains Plan-stage. The terms are not wrong but are imprecise — they overstate what has been made operationally true.

**Going forward (from R3 onward), dispositions use three terms:**

- **Fixed.** Charter language updated, and the resulting commitment is operationally enforceable by the runtime as currently scoped (e.g., a wording clarification, a section restructure, a refuted finding).
- **Locked in Charter, enforcement pending Plan.** Charter language updated; the commitment is made; but the runtime check that enforces it lives at Plan-stage (e.g., the new Audit-Store Minimum Schema invariant is locked, but the storage mechanism that performs the check is Plan-stage).
- **Refuted.** Claim is not grounded against the actual state.

This is applied prospectively. R1 and R2 dispositions remain as written. Their "Fixed" labels should be read with the looser interpretation common in software review prose; R3 onward uses the tighter taxonomy.

### Encoding finding (R3 #12) — refuted on factual grounds

R3 cited em-dashes at L1 of Charter and L1 of R2 disposition as "encoding noise that should be cleaned because the Charter explicitly locks UTF-8."

The premise conflates UTF-8 (an encoding) with ASCII (a subset of UTF-8). The *Artifact Encoding* invariant — added in R2 — explicitly says:

> Non-ASCII characters (em-dashes, typographic quotes, mathematical symbols) are *permitted and intentional* where they aid readability; they are not corruption.

The cited characters are exactly what the invariant permits. The em-dash (U+2014) is valid UTF-8 typography that the AiMe `IP/` convention uses extensively without issue.

This is the second time a reviewer has raised a similar concern (R1 #10 was also about encoding; refuted on the same grounds). Both refutations have produced useful adjacent improvements (the Artifact Encoding invariant was R2 #2.1; this clarification anchors the existing invariant against future re-litigation). The operational mitigation for genuine paste-from-wrong-source incidents is the Plan-stage lint check at the audit-store boundary, which is already in the invariant.

No Charter content change. The invariant's explicit allowance language is preserved.

---

## Residual / deferred

The R3 round substantially shrank the deferred surface. What remains:

- **Audit-store implementation.** The Minimum Schema is now Charter-locked. The Plan stage specifies storage format (filesystem, SQLite, etc.), indexing, and the runtime mechanism that performs Cross-Reference Integrity checking.
- **Planner implementation specifics.** The Contract is Charter-locked. The Plan stage specifies data schema, heuristic versus learned planning, operational handling of cross-cutting concerns.
- **Ship transport actions.** The Abstraction is Charter-locked. The Plan stage specifies which transport actions apply (commit, tag, deploy, hand-off) per project, and in what order.
- **Plan-consolidation thresholds.** The invariant is Charter-locked. The Plan stage specifies numeric thresholds.
- **Rollback detailed mechanics.** The invariant is Charter-locked. The Plan stage specifies artifact-graph mechanism for version bumps, dependent-phase notification, audit-store representation.
- **Deferred-decision tracking mechanisms beyond hinge tests.** Invariant locked; alternative mechanism implementations are Plan-stage.
- **File system layout.** Project folder structure conventions.
- **UI / UX surface.** CLI / web / desktop choice.
- **Interlocutor model override criteria.** Default locked; when to override is a project-level decision.
- **Per-metric numeric thresholds for Project Success Targets.** Layer-2 of Evaluation Criteria.
- **Runtime alert response policies.** Layer-3 of Evaluation Criteria.

All of these are genuinely implementation-level. R3 itself was the round that drew the line between "must be Charter-level" and "may be Plan-level," and the items above are on the Plan-level side of that line.

---

## Reproducibility

**Shell assumption:** the commands below use POSIX shell utilities (`grep`, `awk`). On non-POSIX environments, each command's *intent* is described inline so equivalent checks can be performed with any tool that supports section-isolated text search.

```bash
# --- R3 #1 — Six must-lock contracts present in Charter body ---
# Intent: confirm Planner Contract, Ship Abstraction, Audit-Store Minimum Schema,
# Coder Model Pinning, and Rollback / Re-Open all appear in normative sections.

grep -n "Planner Contract" new_project_charter.md
grep -n "Ship Abstraction" new_project_charter.md
grep -n "Audit-Store Minimum Schema" new_project_charter.md
grep -n "Coder Model Pinning" new_project_charter.md
grep -n "Rollback / Re-Open" new_project_charter.md
# Expected: each returns ≥1 match outside the *Open Items* section.

# --- R3 #2 — Governance Taxonomy section exists ---
# Intent: verify the three-bucket classification section is present between Roster and Invariants.

grep -n "^## Governance Taxonomy" new_project_charter.md
awk '/^## Governance Taxonomy/,/^## Invariants/' new_project_charter.md | grep -c "Immutable Invariants\|Default Policies\|Required Project-Level Choices"
# Expected: 1 section header; 3 bucket-name matches.

# --- R3 #4 — Charter no longer contains Hardening Notes sections ---
# Intent: confirm the Charter body ends cleanly and history lives elsewhere.

grep -c "^## Hardening Notes" new_project_charter.md
# Expected: 0 in the Charter.

grep -c "^## Hardening Notes" CHARTER_HARDENING_HISTORY.md
# Expected: 3 in the history file (R1, R2, R3).

# --- R3 #7 — Evaluation Criteria has three labelled layers ---
# Intent: confirm the three-layer structure exists in section.

awk '/^## Evaluation Criteria/,/^## /' new_project_charter.md | grep -E "^### Layer [123]"
# Expected: 3 matches (Layer 1, Layer 2, Layer 3).

# --- R3 #8 — Two new risks present ---
# Intent: confirm artifact sprawl and phase coupling risks added.

awk '/^## Risks and Failure Modes/,/^## /' new_project_charter.md | grep -E "artifact sprawl|phase coupling"
# Expected: ≥2 matches (one per risk).

# --- R3 #9 — Reviewer-independence language normalized ---
# Intent: confirm Roster and Charter Review use floor/default vocabulary.

awk '/^## Specialist Roster/,/^## /' new_project_charter.md | grep -E "Floor \(enforced\)|family floor"
# Expected: ≥1 match in Roster.

awk '/^### Charter Review/,/^### /' new_project_charter.md | grep -E "floor|default"
# Expected: ≥1 match in Charter Review subsection.

# --- R3 #11 — Shell assumption stated in this disposition ---
# Intent: confirm the assumption appears at top of this R3 disposition.

grep -n "Shell assumption" REVIEW_CHARTER_R3.md
# Expected: 1+ matches near the document header.
```

These commands isolate sections via `awk` range expressions, so matches in other sections (such as `CHARTER_HARDENING_HISTORY.md` or other dispositions) cannot contaminate the verification. The R1 / R2 disposition commands remain valid as historical record but should not be relied on for verification of post-R3 state.

---

## Bottom line

R3 was the round that drew the Charter / Plan boundary clearly. Eleven of twelve findings produced substantive movement; the twelfth was a recurrence of an already-refuted point and is now anchored against re-litigation by an explicit invariant.

The Charter is materially more useful to the Planner now: there is no longer a meaningful risk that the Plan ends up making Charter-level decisions implicitly, because the Charter-level commitments are explicit. The Planner has a Contract; Ship has an Abstraction; the audit store has a Minimum Schema; Coder versioning has a Pinning rule; rollback has Semantics; and every load-bearing rule has a Governance Taxonomy bucket telling the Planner whether it can be parameterized.

The Charter is also structurally cleaner: round-by-round provenance lives in `CHARTER_HARDENING_HISTORY.md`; the Charter body is the constitution; disposition files are the legislative record per round; the audit store is the receipts.

Per the locked termination condition (full-pool clean default), the next rotation step is **R2-Reviewer-B producing a clean confirmation pass on the post-R3 state**. The Charter does not ship until both reviewers' most recent passes come back clean on the same state. If R2-Reviewer-B's confirmation surfaces new findings, those become R4 input and the loop continues; if it returns clean, R1-Reviewer-A gets one more pass to confirm any R4-induced changes don't break their R3 sign-off, and only when both are clean on the same state does the Charter ship.
