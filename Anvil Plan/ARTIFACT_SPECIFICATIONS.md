# Anvil — Artifact Specifications

**Date:** 2026-05-19
**Status:** Draft 1 — Companion to Charter Amendment A1; awaiting R1 Review (concurrent with Amendment A1 review)
**Spec version:** 1.0.0 (semver; major version bumps require Charter-amendment notification per Charter Amendment A1, Item 2)
**Authority:** This document is the canonical specification referenced by the Charter invariant *Artifact Structures Are Defined* (proposed in Charter Amendment A1, Item 2). All canonical Anvil workflow artifacts conform to the templates and schemas defined here.

---

## Scope and Audience

This specification defines the structure of four canonical Anvil workflow artifacts and the standard vocabularies that those artifacts use. It is the contract between Anvil and:

- **The Coder specialist**, which produces artifacts that satisfy these specifications.
- **The Reviewer specialists** and the human Coordinator, who consume artifacts that satisfy these specifications.
- **External contributors** to the Anvil open-source project, who author Plans, Briefings, Dispositions, and Findings Packets as part of their participation in the workflow.
- **Embedding programs** (AiMe and other future consumers per Charter Amendment A1, Item 3) that depend on artifact shapes being stable contracts.

This is not a tutorial. It is a reference. Templates here describe required structure; the *content* of any individual artifact is project-specific and is the responsibility of the authoring Coder.

---

## Document Family Overview

Four canonical artifacts are specified:

1. **Plan document** — the phased implementation specification for a project, produced by the Planner specialist and rendered by the Coder. One per project.
2. **Phase Review Briefing** — the "what was built / what to review" self-audit the Coder writes before sending a phase for review. One per phase per round.
3. **Disposition document** — the "how findings were addressed" document the Coder writes after a review round. One per artifact per round (R1, R2, ...).
4. **Findings Packet** — the structured packet a reviewer produces during a review round; consumed by the human Coordinator at curation; persisted to the audit store. Not a human-facing markdown document; a typed data structure.

Plus standard vocabularies (Section *Standard Vocabularies* below) used across all four artifacts.

---

## Standard Vocabularies

These vocabularies are the same across all artifacts. Authors must use these exact terms — synonyms or paraphrases break tooling and contributor onboarding.

### Severity Tiers

Findings carry one of three severity tiers. Severity is assigned by the reviewer producing the finding; the human Coordinator may edit severity during curation but must use the same tier vocabulary.

| Tier | Meaning | Blocks Ship? |
|---|---|---|
| **P1** | Substantive issue: contradiction, missing required content, broken invariant, integrity gap, factual error that affects downstream decisions. | Yes, in all rounds. |
| **P2** | Material refinement: clarity issue, missing edge case, under-specified contract, alignment opportunity, risk worth naming. | Yes, in rounds 1–5; advisory in rounds 6+ (per Charter *Convergence Safeguards*). |
| **P3** | Style, prose, formatting, or cosmetic issue. Does not affect correctness or downstream consumption. | Advisory in all rounds. |

### Disposition Labels

The Coder assigns one of four disposition labels to each finding when rendering the Disposition document. The labels are exhaustive; no other label is permitted.

| Label | Meaning |
|---|---|
| **Fixed** | Charter or document language updated, and the resulting commitment is operationally enforceable by the runtime as currently scoped (e.g., a wording clarification, a section restructure, a contradiction resolved). |
| **Locked in Charter, enforcement pending Plan** | Charter or document language updated; the commitment is made; but the runtime check that enforces it lives at Plan stage or later (e.g., a new invariant is locked but the test that verifies compliance is implementation-deferred). |
| **Refuted** | The finding's claim is not grounded against the actual state. The Coder presents the counter-evidence in the Disposition document's *Corrections* section. |
| **Deferred** | The finding is acknowledged but resolution is intentionally deferred to a later phase or version. The Disposition document names where the deferral lives (Open Items, hinge test, Provisional Lock, future Charter amendment). |

### Lock States

Lock states apply to Required Project-Level Choices in the Plan.

| State | Meaning |
|---|---|
| **Final** | The Choice is fixed for the project's lifetime. Changing it requires Charter amendment. |
| **Provisional** | The Choice is locked with a current best-guess value, a stated hypothesis, and an explicit revision trigger. When the trigger fires, the Choice is re-evaluated: confirmed (Provisional → Final), revised (new Provisional with new trigger), or escalated to Charter amendment. |
| **Unlocked** | The Choice has not been set. A project with any Unlocked Required Choice cannot enter the Plan stage. |

### Status Values

Status values apply to Charter, Plan, Amendment, and spec documents.

| Status | Meaning |
|---|---|
| **Draft N** | The document is in author-revision before R1 review begins (N ≥ 1). Multiple drafts may exist before the first review round. |
| **Awaiting R<N> Review** | The document is locked from further author-revision and is in flight to the reviewer pool. |
| **R<N> Disposition** | The document is being updated in response to R<N> findings. |
| **Convergent** | The document has passed the termination condition (full-pool clean) or the Coordinator has declared human-arbiter convergence. |
| **Approved** | Synonym for Convergent; used when the document is ready for downstream consumption. |
| **Superseded** | The document has been replaced by a newer version. The superseding document references this one; this one remains as a queryable historical artifact. |

---

## Plan Template

The Plan is the project's phased implementation specification. It is produced by the Planner specialist from the approved Charter and rendered by the Coder.

### Required Top-Level Sections (in order)

1. **Header block** — date, status (per *Status Values* vocabulary), Charter version consumed (with hash if pinned), authoring model, Planner-Contract compliance statement.
2. **Executive Summary** — what's being built (one paragraph), the architectural shape (diagram or prose), and the deliverable acceptance test (the criterion that determines v1 "done").
3. **Project Context** — recursive notes, related artifacts, candidate Charter amendments surfaced during planning. Optional but encouraged.
4. **Product Positioning** — if the project has external-audience positioning concerns, locked here as anchor for implementation trade-offs. Optional but required for open-source projects.
5. **Locked Required Project-Level Choices** — table with columns Choice, Lock Type, Value, Revision Trigger.
6. **File System Layout** — both source repo layout and per-project layout (if the project produces per-project workspaces).
7. **Phase Decomposition** — each phase as a sub-section per the *Phase Definition* sub-spec below.
8. **Phase Dependency Graph** — explicit visualization (ASCII art is acceptable) or formal DAG. Must show the critical path and any parallelizable branches.
9. **Cross-Cutting Concerns** — concerns that span multiple phases and are declared at Plan level rather than absorbed into phases.
10. **Deferred-Decision Registry** — all hinge tests across all phases, listed in one place for visibility.
11. **Evaluation Metric Targets** — Layer-2 numeric targets per the Charter's *Evaluation Criteria*. May be Provisional initially.
12. **Plan-Specific Risks** — risk + mitigation pairs. Charter-level risks are not duplicated here; only Plan-specific risks.
13. **Open Items** — Plan-stage deferrals that are not blocking but warrant tracking.
14. **Candidate Charter Amendments** (optional) — items surfaced during planning that warrant Charter-level treatment; will be processed through an amendment cycle.
15. **Plan-Level Acceptance Criteria** — numbered list of conditions; the Plan is satisfied when all are true.
16. **Plan Review Process** — how this Plan will be reviewed (typically references the standard process).
17. **Version Transition** (optional) — if the Plan is for v1 of a project where v1.x exists, the transition design.
18. **Bottom Line** — short summary appropriate for a reviewer reading only this section.

### Phase Definition (sub-spec)

Each phase listed under *Phase Decomposition* must include:

- **Phase ID** — a short alphanumeric code (e.g., `P0`, `P3a`, `P11`) used in dispositions and audit-store cross-references.
- **Descriptive Name** — human-readable phase title.
- **Goal** — one-sentence statement of what the phase accomplishes.
- **Action List** — bulleted or numbered list of concrete actions the Coder will take to complete the phase. Each action should be specific enough that a reviewer can evaluate whether it was done.
- **Deliverable** — what artifact(s) the phase produces. Concrete and inspectable.
- **Acceptance Criteria** — numbered list of conditions that must hold for the phase to be considered shipped. Each criterion must be testable.
- **Dependencies** — list of Phase IDs this phase depends on (transitively closed, not just direct).
- **Hinge-Test List** — deferred decisions encoded by this phase as hinge tests. Each entry names the test, what current state it pins, and what would flip it.
- **Evaluation-Metric Impact** — which Layer-1 Product Health Metrics this phase moves (from the Charter's *Evaluation Criteria*).
- **Estimated Rounds-to-Convergence** — author's expectation of how many R-N rounds the phase will need (1, 2, 3, or more). Informs scheduling and triggers review of the phase decomposition if it converges much slower than estimated.

### Optional Sections

- **Anchors** — code-citation index for phases that touch specific files. Useful for review.
- **Glossary** — project-specific terminology beyond the standard vocabularies.
- **Pre-flight checklist** — items that must be true before P0 begins.

---

## Phase Review Briefing Template

The Phase Review Briefing is the document the Coder writes *before* sending a phase to the reviewer pool. It is the self-audit that arms the reviewer with a structured starting point.

### Required Top-Level Sections (in order)

1. **Header block** — date, scope (one-line description of what this briefing covers), spec link (which Plan section is being implemented), tests, status.
2. **What Was Built** — table with columns File, Action (CREATE / MODIFY / DELETE), Purpose, Lines (approximate delta). Captures the concrete file-level changes.
3. **Architecture Compliance** — table or section mapping each invariant the phase touches to the evidence that the implementation satisfies the invariant. The Coder argues the case; the reviewer evaluates the argument.
4. **What to Review** — numbered, specific questions the Coder wants the reviewer to engage with. Reviewers may also raise findings outside this list, but the list focuses attention.
5. **Test Coverage Summary** — table with columns Area, Tests Added, Coverage Status. Names what is tested and what is intentionally deferred.
6. **How to Activate for Testing** — runbook-style instructions for the reviewer to manually verify the phase's behavior. CLI commands, expected outputs, rollback path if anything regresses.
7. **Next Phase** — preview of what ships after this phase, so the reviewer can evaluate boundary decisions in context.

### Optional Sections

- **Risks** — phase-specific risks not in the Plan-level Risks section.
- **Known Limitations** — issues the Coder is aware of but is deferring to a later phase. Each item should have a cross-reference (hinge test, Open Items entry, future phase).
- **Asks** — explicit requests for the human Coordinator that are not finding-shaped (e.g., "please confirm vendor X's API rate limits before P3c integration tests").

---

## Disposition Document Template

The Disposition document is the document the Coder writes *after* a review round, in response to the Findings Packet curated by the human Coordinator. It is named `REVIEW_<artifact>_R<N>.md` where N is the round number.

### Required Top-Level Sections (in order)

1. **Header block** — date, scope, spec link, prior round (if N > 1), reviewer pseudonym or identity, shell assumption for any reproducibility commands.
2. **What Changed Since R<N-1>** (or "What Changed in This Round" for R1) — narrative summary of the round's substantive changes. Reads as prose, not as a table.
3. **Verification of R<N> Claims** — table with columns Finding, Verifiable Claim, Verified?, Notes. Documents the Finding Verifier's pass over the reviewer's evidence before any code action.
4. **Disposition of R<N> Findings** — table with columns #, Severity, Finding (one-line), Disposition (per *Disposition Labels* vocabulary). One row per finding.
5. **Files Changed Since R<N-1>** — table with columns File, Action, Purpose, Approximate Delta. Same shape as the Phase Review Briefing's *What Was Built*, scoped to this round's changes.
6. **Corrections to R<N-1> Narrative** — present only if the current round's review found errors in the prior round's disposition (or related prior artifacts). Each correction names the affected paragraph or section explicitly and provides the superseding statement. Per single-writer discipline, prior disposition documents are not edited; corrections supersede them.
7. **Residual / Deferred** — findings not fully addressed in this round, with explicit rationale and cross-reference (Open Items, hinge test, future phase, future amendment).
8. **Reproducibility** — POSIX-shell (or stated-assumption) commands a reviewer can run to verify the live state matches the disposition's claims. Each command's *intent* must be described inline.
9. **Bottom Line** — short summary appropriate for a reviewer reading only this section.

### Optional Sections

- **What Was Built Net-New** — for phases where the disposition introduces significant new architecture (rather than refining existing), a summary of the new pieces.
- **Open Questions for the Next Reviewer** — explicit prompts for the next round's reviewer (analogous to *What to Review* in the Briefing template).

---

## Findings Packet Schema

The Findings Packet is the structured data object a reviewer produces during a review round. It is *not* a markdown document; it is a typed schema persisted to the audit store as a `reviewer-finding-packet` record (per the Charter's *Audit-Store Minimum Schema*).

### Top-Level Schema

```
FindingsPacket {
  packet_id: string             // UUID generated at packet creation
  artifact_ref: string          // cross-reference key of the reviewed artifact (e.g., "charter.md:post-R3-state")
  round_number: integer         // R1 = 1, R2 = 2, ...
  reviewer_id: string           // pseudonym or full reviewer identifier
  reviewer_model_identity: string  // model family + version, for diversity-audit purposes
  produced_at: string           // ISO 8601 timestamp
  findings: Finding[]           // ordered list; reviewer's intended order is preserved
  reviewer_meta: {              // optional; informs Coordinator and audit
    review_duration_seconds: integer?
    notes: string?              // free-form reviewer notes; not findings
  }
}
```

### Finding Schema

```
Finding {
  id: string                    // local to the packet (e.g., "F1", "F2"); becomes global on persistence
  severity: SeverityTier        // P1 | P2 | P3 (per Standard Vocabularies)
  location: LocationAnchor      // see below
  claim: string                 // one-sentence statement of the issue
  evidence: string              // citation of the artifact text or code that supports the claim
  recommendation: string        // proposed resolution or direction
  metadata: {                   // optional
    related_finding_ids: string[]?
    proposed_severity_floor: SeverityTier?  // reviewer's view of the minimum severity if Coordinator disagrees
  }
}
```

### LocationAnchor Schema

```
LocationAnchor {
  artifact_path: string         // relative path of the reviewed artifact
  section_id: string?           // section heading or ID, if applicable
  line_range: [integer, integer]?   // 1-indexed, inclusive
  symbol_name: string?          // for code artifacts: function / type / module name
  quote: string?                // short verbatim quote from the artifact for grounding
}
```

A LocationAnchor must contain at least one of `section_id`, `line_range`, or `symbol_name`. A finding without any anchor is permitted but is classified as `CannotBeVerified` by the Finding Verifier; it does not block ship by itself.

### Curation Annotations

When the human Coordinator curates a Findings Packet, the curation does not modify the original packet (single-writer / append-only discipline). Instead, a sibling `curated-findings` record is written referencing the original packet by ID. The curated record carries per-finding dispositions:

```
CuratedFindings {
  packet_id: string             // references the original FindingsPacket
  curated_at: string            // ISO 8601 timestamp
  curated_by: string            // Coordinator identifier
  dispositions: CurationDisposition[]
}

CurationDisposition {
  finding_id: string            // references Finding.id in the original packet
  action: "keep" | "drop" | "edit" | "annotate"
  edited_finding: Finding?      // present only if action is "edit"
  annotation: string?           // present only if action is "annotate" or "drop"
}
```

---

## Versioning Policy

The specifications in this document are themselves a versioned artifact.

### Semver Discipline

- **MAJOR** version (1.0.0 → 2.0.0) — backward-incompatible changes. Existing artifacts may not satisfy the new spec. **Requires full Charter amendment** (per Charter Amendment A1, Item 2's tightening after R1 — the prior "Charter-amendment notification" language was insufficient because it could bypass full constitutional review). Migration window during which both old and new shapes are accepted runs one Plan-stage phase boundary by default, configurable per project.
- **MINOR** version (1.0.0 → 1.1.0) — backward-compatible additions. New required fields are *not* permitted; new optional fields and new vocabulary terms are permitted. Existing artifacts continue to satisfy the spec. Uses the narrower spec-amendment review process below.
- **PATCH** version (1.0.0 → 1.0.1) — clarifications, prose improvements, example additions. No schema or vocabulary changes. Uses the narrower spec-amendment review process below.

### Process for Spec Amendments

This specification document is reviewable through one of two paths depending on change severity:

**Major changes → Full Charter amendment cycle.** Backward-incompatible spec changes (per the MAJOR version definition above) go through the full Charter Amendment process, identical in form to Charter Amendment A1 itself: drafted as `CHARTER_AMENDMENT_<id>.md`, reviewed by the full reviewer pool, dispositioned through R-N rounds, converged via full-pool-clean or human-arbiter declaration, applied to both this specification document and (where applicable) the Charter itself.

**Minor and patch changes → Narrower spec-amendment review.** Backward-compatible additions (MINOR) and clarifications (PATCH) use this narrower process:

1. Coder drafts a `SPEC_AMENDMENT_<id>.md` proposal document referencing the change.
2. Proposal goes through R1 review by the same reviewer pool (or a narrower spec-focused pool if the project's Plan configures one).
3. Convergence per the standard termination condition or human-arbiter declaration.
4. Coder applies the amendment to this document and bumps the spec version per semver.
5. The Charter's hardening history receives a notification entry naming the spec change so constitutional-adjacent changes remain visible.

**Selection rule.** If there is ambiguity about whether a change is MAJOR (full Charter amendment) or MINOR (narrower review), the safer reading wins: treat it as MAJOR. Tightening this rule was an explicit R1 outcome on Charter Amendment A1 — narrower review must not bypass constitutional oversight.

---

## How to Author a New Artifact

A contributor (Coder, external participant, or embedding program) authoring a new artifact follows this process:

### For Plans, Briefings, and Dispositions (markdown documents)

1. Read the relevant template above and identify the required top-level sections.
2. Copy section headings into a new file at the conventional path (`<project>/plan.md`, `<project>/reviews/REVIEW_<artifact>_R<N>.md`, etc.).
3. Populate each required section. Optional sections may be included or omitted at the author's discretion.
4. Validate locally: every required section is present, every required field within each section is populated, vocabulary is used per the *Standard Vocabularies* section.
5. Save and proceed with the workflow (send for review, etc.).

### For Findings Packets (structured data)

1. The reviewer specialist produces a Findings Packet directly as structured output (typically JSON conforming to the schema above). This is the reviewer's job; contributors do not hand-author Findings Packets in normal workflow operation.
2. If a contributor needs to hand-author a finding (e.g., to file a post-release issue against an open-source Anvil project), they may produce a Finding-shaped object and submit it as an issue or a pull-request finding entry; the project's process for absorbing such contributions is documented in `CONTRIBUTING.md`.

### Validation Tools

The Anvil CLI (per the v1 Plan, P5–P10) provides validation commands:

- `anvil validate plan` — checks that a Plan satisfies this spec.
- `anvil validate briefing <path>` — checks a Phase Review Briefing.
- `anvil validate disposition <path>` — checks a Disposition document.
- `anvil validate findings <path-or-record-id>` — checks a Findings Packet record.

A document that fails validation cannot proceed to the next workflow stage (e.g., a Plan that fails `anvil validate plan` cannot enter Plan Review).

---

## Examples and References

This specification refers to concrete prior artifacts as examples of the templates in practice. Readers can examine these directly:

- **Plan template** — see `ANVIL_PLAN.md` (the Anvil v1 Plan itself, which satisfies this template as of Draft 6).
- **Phase Review Briefing template** — see AiMe's `IP/review_rounds/REVIEW_USER_PROFILE_WIDGET_PHASE1.md` (the AiMe pattern that informs this spec).
- **Disposition document template** — see `REVIEW_CHARTER_R1.md`, `REVIEW_CHARTER_R2.md`, `REVIEW_CHARTER_R3.md`, `REVIEW_CHARTER_R4.md` (the Anvil Charter review rounds).
- **Findings Packet schema** — no concrete prior example in markdown form because Findings Packets are structured data, not documents. The Anvil v1 implementation (P3a contract definition + P4 audit-store implementation) is the first concrete realization.

---

## Bottom Line

This specification defines four artifact shapes (Plan, Phase Review Briefing, Disposition document, Findings Packet) and four standard vocabularies (severity tiers, disposition labels, lock states, status values) that all canonical Anvil workflow artifacts must conform to. It is the contract between Anvil and its contributors, reviewers, and embedding programs.

This specification is itself a versioned artifact under semver discipline. Backward-incompatible changes require a Charter-amendment notification and a migration window. The Charter's *Artifact Structures Are Defined* invariant (proposed in Charter Amendment A1, Item 2) makes this contract constitutional.

Next step: this specification is reviewed concurrently with Charter Amendment A1 through R1 review.
