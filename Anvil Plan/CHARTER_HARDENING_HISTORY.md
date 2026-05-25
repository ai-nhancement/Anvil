# Anvil Charter — Hardening History

This document is the **provenance log** for the Anvil Project Charter. The Charter itself (`new_project_charter.md`) is the canonical normative text — what the system *is*. This file is the historical log of how the Charter got to its current state — the round-by-round changes folded in through Charter Review.

Per *Cross-Reference Integrity*, every consolidated hardening note here corresponds to a disposition document (`REVIEW_CHARTER_R<N>.md`) and to raw reviewer-finding-packet records in the audit store. The hardening note summarizes; the disposition documents the round; the audit records prove.

Per *Plan Consolidation*, hardening notes accumulate in this file until a Plan-stage consolidation pass absorbs them into the Charter body and bumps the Charter's version. After consolidation, prior versions of the Charter remain queryable via the artifact graph, and this file retains the round-by-round detail.

The Charter does not append new hardening notes inline; they land here.

---

## Hardening Notes (R1 — Consolidated)

R1 surfaced ten findings (4 × P1, 5 × P2, 1 × P3). Eight were accepted and folded directly into the Charter body. One (P2 #6, human-gating ceremony) was *partially* accepted: gate granularity is now explicit, but the user's stated preference for human approval at every transition stands. One (P3 #10, "encoding corruption") was *partially refuted* on factual grounds — the cited characters are valid UTF-8 typography, not corruption — while the underlying durability concern is acknowledged via explicit commitment to UTF-8 + typographic punctuation matching the AiMe convention.

The eight items below capture the converged decisions in shape they will outlive this round. See `REVIEW_CHARTER_R1.md` for the full disposition table including refutations and reproducibility.

### 1. Scope contradiction resolved (P1)

R1 caught a contradiction between `What This Is Not` (single project at a time) and `Scope Boundaries / Project-scoped` (can host multiple projects). Resolution: v1 supports *single active* project with storage that may hold multiple projects. Project switching is an explicit user action. Both lines now state the same operating mode.

### 2. Fix-loop termination condition locked (P1)

Promoted from Open Items to Charter decision. **Default termination:** every reviewer in the project's pool has produced a clean pass on the most recent state (full-pool clean, the stronger independence guarantee). **Configurable per project** to *single clean pass* when the lower guarantee is acceptable. The Open Items list no longer carries fix-loop termination as an open question.

### 3. Single-Writer invariant tightened (P1)

The original wording — "exactly one model writes files" — was technically inconsistent with the Reviewer Audit Trail commitment (raw findings packets are append-only on disk). Tightened to: the Coder authors **human-facing project artifacts** (code, briefings, dispositions, amendments); other specialists may emit **machine-readable records** to the audit / provenance store but never to the human-facing artifact tree. The audit-store paragraph now explicitly notes it does not violate Single-Writer.

### 4. Quality claims softened; Evaluation Criteria section added (P1)

"Dramatically higher quality" rephrased as "noticeably higher-quality output … measurable against Evaluation Criteria" with explicit framing as hypothesis-to-be-validated. New section `Evaluation Criteria` names the metrics: defect escape rate, review finding precision, human minutes per shipped phase, review rounds per phase, cross-reviewer agreement, hinge-flip rate. Thresholds are deferred to the Plan stage (added as new Open Item).

### 5. Adversarial-Diversity given operational definition (P2)

The original phrasing pegged independence to *vendor* without acknowledging vendor-line blur. New invariant defines independence operationally with three tiers: enforced *floor* (different model family from Coder), preferred *default* (different vendor, two+ vendors collectively), and *pool size* requirement (at least two distinct models). Config schema enforces the floor; vendor preference is relaxable, family floor is not.

### 6. Human-Gating granularity clarified (P2, partially accepted)

The user's stated preference for human approval at every transition stands. R1's concern was over-strict reading that would have required a gate between every internal mechanical step. Resolved by explicit enumeration: **gates** are stage boundaries and within-phase major beats (briefing→reviewer, findings→curation/Coder, disposition→next-reviewer-or-Ship); **non-gates** are internal mechanics (Verifier execution, audit writes, rotation arithmetic, summary rendering). Internal mechanics run freely; their outputs are reviewed at the next major beat.

### 7. Living Plan paired with Plan Consolidation (P2)

Living Plan invariant retained; a paired *Plan Consolidation* invariant added. Hardening Notes accumulate by design but undergo periodic consolidation (at phase boundaries, version cuts, or note-count thresholds) where prior notes are absorbed into the main body, the Plan version is bumped, and provenance is preserved via the artifact graph. Plan-consolidation triggers added as a new Open Item.

### 8. Hinge Tests reframed as preferred mechanism under broader invariant (P2)

The original invariant elevated hinge tests to "never violate" before their applicability across project types was proven. Replaced with broader invariant **Deferred Decisions Are Tracked** — the requirement is first-class deferral as a queryable artifact, not specifically a hinge test. Hinge tests are named as the *preferred* mechanism; alternative mechanisms (flagged registry, deferral docs, calendar reminders) are allowed where the stack does not support hinge tests cleanly, provided the tracking property holds.

### 9. Verifier authority boundary sharpened (P2)

Verifier scope is now stated as *strictly evidence validation*. Findings emerge as **grounded**, **refuted**, or **cannot-be-verified** (semantic claims without anchors). Explicit charter language: "The Verifier does not decide whether a finding's recommendation is worth acting on — that is the human's role at curation time. Verification is about facts; disposition is about judgment." Both the Phase Review section and the Findings-Are-Grounded invariant reflect this.

### R1 reviewer

Pseudonymized as `R1-Reviewer-A` in the audit store pending vendor-tagging convention. (To be specified in the Plan stage.)

### Disposition document

`REVIEW_CHARTER_R1.md` (R1 round).

---

## Hardening Notes (R2 — Consolidated)

R2 came from a second reviewer (pseudonymized `R2-Reviewer-B`, drawn from a different model family than both Coder and R1 reviewer). The R2 review was more architectural than R1 — fewer line citations, more conceptual concerns about how the post-R1 invariants would behave in practice. Six findings raised; all six addressed.

The two largest gains in this round:

- The **Cross-Reference Integrity** invariant — a new explicit requirement that human-facing decisions must be backed by machine-readable records in the audit store. Without this, the R1-introduced Single-Writer / Audit Trail pair could allow prose and audit to drift apart silently. The new invariant closes that gap.
- The **falsifiability of Evaluation Criteria** — R1 added metrics but R2 correctly noted they were not yet falsifiable without direction-of-success indicators. Each metric now carries an explicit direction and a qualitative success indicator. Numeric thresholds remain a Plan-stage item, but the Charter is no longer claiming measurable quality without a measurement frame.

### 1. "Major beats" enumeration made exhaustive (R2 #1.1)

Previous wording listed three within-phase beats as examples, leaving room for "ceremony creep" (adding gates ad hoc) or "ceremony bypass" (skipping a gate by arguing it wasn't on the list). Tightened: six gates total per phase loop, declared exhaustive. Sub-beat enumeration for complex Build phases is explicitly a Plan-stage parameterization, not an emergent property.

### 2. Evaluation Criteria falsifiability strengthened (R2 #1.3)

Each of the six metrics now carries an explicit direction-of-success indicator and a qualitative threshold (e.g., "≥70% review precision is healthy"). The Charter is now falsifiable at the qualitative level *before* the Plan stage sets project-specific numeric thresholds — a phase that violates the qualitative direction without explanation is a flag.

### 3. Cross-Reference Integrity invariant added (R2 #2.2)

New invariant inserted between Provenance Graph and Reviewer Audit Trail: every human-facing decision in a project artifact must be backed by a corresponding machine-readable record in the audit store. The Coder's prose summarizes; the audit record proves. A decision present in prose but absent from the audit store is a violation. This invariant prevents drift between the two stores — the failure mode that R1's Single-Writer / Audit Trail split would otherwise be exposed to.

### 4. Artifact Encoding invariant added (R2 #2.1)

New explicit Charter-level commitment: UTF-8 is the encoding for all human-facing project artifacts. Non-ASCII typography (em-dashes, smart quotes) is permitted and intentional, not corruption. Plan-stage commitment to a lint check at the audit-store boundary that flags invalid byte sequences and surfaces non-ASCII characters for explicit human acknowledgement.

### 5. Audit-store schema elevated to highest-priority Plan-stage item (R2 #3.1)

Previous Open Items listing treated the audit-store schema as equivalent to other deferrals. R2 correctly identified it as the single highest-risk deferral — without the schema, both *Findings Are Grounded* and the new *Cross-Reference Integrity* invariants are unenforceable. Open Items entry rewritten with explicit "highest-priority" tag and a schema-content checklist (record types, cross-reference key, queryability, append-only, UTF-8 lint boundary).

### 6. Grep verification robustness — superseded in R2 disposition (R2 #1.2)

R1 disposition's reproducibility section used global greps that returned matches in the Hardening Notes section. R2 correctly flagged this as confusing. The R2 disposition document supersedes the affected reproducibility commands with surgical section-isolated checks. R1 disposition is not edited (single-writer artifact discipline); R2 disposition explicitly notes the supersession.

### R2 reviewer

Pseudonymized as `R2-Reviewer-B` in the audit store. Different model family from both Coder and R1 reviewer per the Adversarial Diversity floor.

### Disposition document

`REVIEW_CHARTER_R2.md` (R2 round).

---

## Hardening Notes (R3 — Consolidated)

R3 came from R1-Reviewer-A returning for the second pass per rotation. The review was structurally heavier than R1 or R2: twelve findings (4 × P1, 5 × P2, 3 × P3) with a central thesis that the Charter had done good work on philosophy but still deferred several *contracts* the Planner would need to consume. Eleven findings were accepted; one (P3 #12, encoding noise) was refuted on technical grounds because the cited characters are valid UTF-8 typography that the Artifact Encoding invariant explicitly permits.

The largest structural change in this round: a set of must-lock-before-plan contracts that had been deferred to the Plan stage were promoted into the Charter body. The reviewer's argument — that deferring these created the risk of the Plan implicitly making Charter-level decisions — was correct, and the right response was to lock the abstract contracts now while keeping their implementations Plan-stage.

### 1. Six must-lock-before-plan contracts promoted into Charter body (P1 from R3 #1, supported by #3, #5, #6)

Five new normative blocks added to the Charter body:

- **Planner Contract** (in *The Workflow / Plan*): minimum inputs, outputs, required per-phase fields, phase size rule, treatment of cross-cutting concerns. Implementation details (data schema, heuristics vs. learned planning) remain Plan-stage.
- **Ship Abstraction** (in *The Workflow / Ship*): Ship means human approval of the artifact set as canonical output. Transport actions (commit, tag, deploy, hand-off) are Plan-level.
- **Audit-Store Minimum Schema** (invariant): required record types, stable cross-reference key, append-only operational definition, creation-before-Ship timing, Ship-block vs. warn behavior. Storage format and runtime mechanism remain Plan-stage.
- **Coder Model Pinning** (invariant): Coder pinned to specific identity (vendor + family + version) for project lifetime; upgrade is a Charter amendment, not a config change.
- **Rollback / Re-Open** (invariant): Any shipped phase may be re-opened via Charter (or Plan) amendment. Re-opening invalidates dependent phases; the artifact graph version-bumps; the audit store is not rewritten.

### 2. Governance Taxonomy section added (P1 from R3 #2)

New section between Specialist Roster and Invariants. Classifies every load-bearing rule into one of three buckets: *Immutable Invariants* (cannot be relaxed; violation requires re-opening the Charter), *Default Policies* (Charter states a default; projects may override at config time), and *Required Project-Level Choices* (must be locked at project creation before Plan stage begins). Each existing invariant and policy now has an explicit bucket. A project that has not locked all Required Project-Level Choices cannot enter Plan stage.

### 3. Evaluation Criteria split into three layers (P2 from R3 #7)

The Evaluation Criteria section now has three explicit layers:

- *Layer 1 — Charter-level Product Health Metrics:* the six metrics + qualitative success indicators (project-agnostic).
- *Layer 2 — Project Success Targets:* numeric thresholds per project, set in Plan, become Ship criteria.
- *Layer 3 — Runtime Alerts:* metric states that fire automatically during execution.

Layer 1 says what to measure; Layer 2 says what numeric targets this project commits to; Layer 3 says what to do when a state fires. The Hinge-flip rate metric was generalized to Deferred-decision resolution rate to align with the broader Deferred Decisions Are Tracked invariant.

### 4. Two missing risks added (P2 from R3 #8)

Risks section now includes:

- *Artifact sprawl / retrieval overload* — durable artifacts accumulate; mitigation via Plan Consolidation and the *Human minutes per shipped phase* metric trending up.
- *Planner-generated phase coupling* — phases that are logically separable but operationally coupled; mitigation via explicit dependency declarations in the Planner Contract and Plan-stage review for hidden coupling.

### 5. Reviewer-independence language normalized to floor/default (P2 from R3 #9)

The Specialist Roster table and the Charter Review subsection had retained the older "different vendor from Coder" wording. Both updated to the floor/default language introduced in R1 (#5) and reinforced in R2 (#1.1). All four call-sites for reviewer independence now use the same vocabulary.

### 6. Normative Charter separated from review history (P1 from R3 #4)

R3 flagged that the appended Hardening Notes sections were competing with the normative body — a planner consuming the Charter could mistake history for requirements. Resolution: the Charter ends at *Bottom Line* + *Provenance and Linked Artifacts*. All hardening notes (R1, R2, this R3 note, and future rounds) live in this file (`CHARTER_HARDENING_HISTORY.md`). The Charter is the constitution; this file is the legislative record. The Plan Consolidation invariant still holds — at consolidation points, accumulated notes here are absorbed into the Charter body and the Charter version is bumped.

### 7. Disposition-language precision noted for future rounds (P3 from R3 #10)

R3 correctly observed that R2 disposition used "Fixed" liberally for charter-level commitments whose enforcement is still Plan-stage. Going forward, dispositions distinguish: *Fixed* (Charter language updated and operationally enforceable), *Locked in Charter, enforcement pending Plan* (commitment made; runtime check is Plan-stage), and *Refuted* (claim not grounded). R1 and R2 dispositions are not retroactively edited; the precision applies from R3 onward.

### 8. Reproducibility commands shell-assumption stated (P3 from R3 #11)

R3 flagged that R2 disposition's reproducibility commands assumed POSIX shell (awk, grep) without saying so. R3 disposition's reproducibility section explicitly states the shell assumption at the top, and offers both POSIX-shell commands and a brief shell-neutral description of what each command should check (so a reviewer on a different shell can adapt).

### 9. Encoding finding refuted (P3 from R3 #12)

R3 cited em-dashes in line 1 of Charter and R2 disposition as "encoding noise" that should be cleaned because the Charter explicitly locks UTF-8. Refuted: the Artifact Encoding invariant explicitly permits non-ASCII typography (em-dashes, smart quotes, mathematical symbols) as intentional, distinguishing it from corruption (invalid byte sequences). The cited characters are exactly what the invariant allows. This is the second time a reviewer has raised a similar concern (R1 #10 was also about encoding); both have been factually refuted, both have produced useful adjacent improvements (Artifact Encoding invariant in R2, this clarification in R3). The Plan-stage lint check at the audit-store boundary surfaces non-ASCII characters for explicit acknowledgement, which is the right operational mitigation for genuine paste-from-wrong-source incidents.

### What R3 did *not* change

- The three load-bearing commitments (staged workflow, single writer, cross-family adversarial review) remain unchanged in intent.
- The six gates remain six gates; their enumeration is unchanged.
- The six Evaluation metrics remain (with Hinge-flip generalized to Deferred-decision resolution rate to match the invariant rename from R1).
- The Adversarial Diversity floor/default structure from R1 (#5) is preserved; R3 just normalized the language at the remaining call-sites.

### R3 reviewer

`R1-Reviewer-A` returning for the second pass per deterministic rotation. (R1 → R2 → R3 = R1-Reviewer-A → R2-Reviewer-B → R1-Reviewer-A.) This is the first chance R1-Reviewer-A has had to see the post-R2 state, which is why this round was structurally heavier than the pure-confirmation pass it would have been if R2 had not introduced substantial new content.

### Disposition document

`REVIEW_CHARTER_R3.md` (R3 round).

---

## Hardening Notes (R4 — Consolidated)

R4 came from R2-Reviewer-B returning for the second pass per rotation. The review's character was confirmation-with-targeted-strengthening rather than structural reorganization: R3's heavier changes were accepted in substance, with five targeted concerns raised about how the newly-locked contracts would behave operationally. All five accepted: three were P1-level (governance bottleneck, cascading invalidation, sign-off loop), one P2 (environment hostility), one P3 (Planner Contract constraint-vs-mechanism precision).

The two largest gains in this round:

- **Convergence Safeguards** added to the Phase Review subsection. The termination condition was previously "every reviewer in the pool has produced a clean pass on the most recent state" — open-ended. R4 correctly identified the *Two Generals' Problem* failure mode: reviewers alternating minor findings could theoretically loop forever. The new safeguards pair severity-tiered convergence (after a configurable round limit, default 5, only P1 findings block) with explicit human-arbiter authority (the Coordinator may declare convergence at any time; the declaration is itself an audit record).
- **Provisional Lock** mechanism added to Governance Taxonomy. R4 noted that Required Project-Level Choices that "must be locked before Plan stage begins" creates a deadlock risk for choices that genuinely need exploratory Plan-stage work to inform them (e.g., per-metric numeric thresholds). The Provisional Lock allows hypothesis-based commitment with an explicit revision trigger; the workflow proceeds; the Choice is re-evaluated when the trigger fires.

### 1. Cascading Invalidation made explicit in Rollback / Re-Open (P1 from R4 #3)

Prior wording invalidated "all phases that declared a dependency on the re-opened phase" — readable as direct dependents only. R4 correctly noted that transitive invalidation is the real semantics. Tightened: re-opening invalidates the *transitive closure* of dependents through the dependency graph. If A → B → C, re-opening A invalidates both B and C. The Coordinator is shown the full blast radius at re-open time and approves it before the re-opening commits. New audit record type: `rollback-event`.

### 2. Convergence Safeguards added to termination condition (P1 from R4 #4)

Two safeguards layered onto the existing termination condition:

- *Severity-tiered convergence:* after a configurable round limit (default 5), only P1 findings block. P2 and P3 findings become advisory in rounds 6+. Round limit added to *Required Project-Level Choices*.
- *Human arbiter authority:* the Coordinator may declare an artifact convergent at any time, forcing the ship state. The declaration is logged to the audit store as a `convergence-declaration` record with reasoning. A project that relies on these frequently is signaled by the audit trail and triggers reviewer-configuration review.

### 3. Provisional Lock mechanism added to Governance Taxonomy (P1 from R4 #1)

Required Project-Level Choices may now be locked *provisionally* with an explicit hypothesis and revision trigger. The provisional lock satisfies the pre-Plan-stage gate. The revision trigger fires during Plan stage; the Choice is then confirmed, revised, or escalated to Charter amendment. New audit record type: `provisional-lock`. The Charter cannot ship while any Provisional Lock is outstanding.

### 4. Pre-Flight Environment Check added to Artifact Encoding invariant (P2 from R4 #5)

The Artifact Encoding invariant now specifies *two* layered protections: the existing lint check at the audit-store boundary (defends the artifact) and a new Pre-Flight Environment Check (defends the tool's view of the artifact). The pre-flight check validates that any tool's runtime is UTF-8 configured before it interacts with project artifacts. This addresses the recurring reviewer-environment failure mode where valid UTF-8 renders as garbled output and gets mistakenly flagged as corruption.

### 5. Planner Contract phase-size framed as constraint, not mechanism (P3 from R4 #2)

Existing wording was already constraint-like ("small enough to read in one session, large enough to deliver value"). R4 correctly flagged that the framing should be explicit to prevent the Charter from inadvertently mandating a specific phase-size mechanism (line caps, etc.). Added explicit "(constraint, not mechanism)" qualifier and a clarifying sentence: implementation of phase-size measurement is Plan-stage; the Charter must not be read as mandating a specific mechanism.

### Side-effects on Audit-Store Minimum Schema

R4 surfaced three new audit record types that the schema invariant now lists: `convergence-declaration` (R4 #4), `provisional-lock` (R4 #1), and `rollback-event` (R4 #3). The Audit-Store Minimum Schema invariant's required-record-types list is updated accordingly.

### What R4 did *not* change

- The four R3 must-lock-before-plan contracts (Planner Contract, Ship Abstraction, Audit-Store Minimum Schema, Coder Model Pinning, Rollback / Re-Open) remain in place; R4 sharpened them rather than re-opening them.
- The Governance Taxonomy section's three-bucket structure (Immutable / Default / Required Project-Level Choice) is preserved; R4 added the Provisional Lock as an escape hatch *within* the third bucket.
- The Evaluation Criteria three-layer split from R3 is unchanged.
- Reviewer-independence language is unchanged.
- No findings of style or prose were raised. The Charter's voice is unchanged.

### R4 reviewer

`R2-Reviewer-B` returning for the second pass per deterministic rotation. (R1 → R2 → R3 → R4 = A → B → A → B.) This was R2-Reviewer-B's first chance to see the post-R3 state.

### Disposition document

`REVIEW_CHARTER_R4.md` (R4 round).

---

## Amendment A1 Applied (2026-05-19)

Charter Amendment A1 (Open-Source Distribution + Defined Artifact Structures + Embeddable Workflow Infrastructure) converged on 2026-05-19 after two review rounds (R1: 15 findings; R2: 12 findings — consistency cleanup per the reviewer's explicit guidance). The Coordinator invoked human-arbiter convergence rather than running R3 because the trajectory matched the prior Charter (R4) and Plan (R5) convergence patterns: findings shifted from structural additions through consistency cleanup with a positive R2 reviewer signal.

The amendment added three new constitutional invariants (Open-Source Distribution; Artifact Structures Are Defined; Embeddable by Design), fifteen new Required Project-Level Choices, and several supporting Charter sections: Contract Inventory, Public vs Private Audit Records, Publication-Safe Git History Gate, Embedding Invariants, Structured CLI Output Stability, Repo-Readiness Acceptance Gates, Per-Item Disposition Mechanism, Cross-Document Convergence rules. The amendment's content is incorporated into the Charter via the new *Amendment A1 — Applied* section; the full amendment text remains queryable in `CHARTER_AMENDMENT_A1.md`. Round-by-round provenance is in `AMENDMENT_A1_HARDENING_HISTORY.md`. Convergence reasoning is in `AMENDMENT_A1_CONVERGENCE.md`.

Audit-store record types grew from the original 11 Charter-required to 16 in v1 with the addition of `PublicVisibilityPolicy`, `PublicExportApproval`, and `EmergencyFreezeDeclaration` (alongside the prior Plan extensions `ArbiterFindingResolution` and `SidecarReload`). The constitutional subset hinge `test_audit_store_required_types_present` is unchanged; Plan Draft 7 reconciles per-implementation counts.

One sibling document, `ARTIFACT_SPECIFICATIONS.md`, was edited mid-flight during R2 to resolve cross-document inconsistency on the spec amendment process — the first practical application of the *Cross-Document Convergence* rules added in this amendment. The spec's own R1 review proceeds separately; Item 2 ("Artifact Structures Are Defined") is fully shipped only when both documents have converged.

Plan Draft 7 is the required immediate next workstream to reconcile downstream Plan content with the amendment-applied Charter. Plan Draft 7 itself goes through review.

---

## Convergence (post-R4)

The Coordinator invoked Human Arbiter Authority on 2026-05-15 and declared the Charter convergent. The R4 round had shifted the *kind* of finding from "structural gap" to "operational refinement" — the trajectory signal that further rounds would produce diminishing returns. No outstanding Provisional Locks; no P1 structural gaps unresolved. Full reasoning and audit cross-references in `CHARTER_CONVERGENCE.md`.

The Charter is now the approved constitutional input to the Plan stage. Required Project-Level Choices (per Governance Taxonomy) must be locked next, before the Planner produces output.

This hardening-history file's role is unchanged: it remains the legislative record of how the Charter got to its current state. Future Charter *amendments* (post-convergence changes triggered by Plan-stage or Build-stage discovery) will continue to append hardening notes here, paired with disposition documents.

