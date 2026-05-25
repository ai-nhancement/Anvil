# Anvil Charter — Convergence Declaration

**Record type:** `convergence-declaration` (per Audit-Store Minimum Schema invariant).
**Date:** 2026-05-15
**Declared by:** John Canady (Coordinator)
**Declared on artifact:** `new_project_charter.md` (post-R4 state)
**Mechanism invoked:** Human Arbiter Authority (per *Phase Review / Convergence safeguards*)

---

## Declaration

The Charter is declared **convergent**. No further review rounds will be conducted. The Charter is approved and is now the constitutional input to the Plan stage.

## Reasoning

Four review rounds were completed under deterministic rotation:

- **R1** (R1-Reviewer-A): 10 findings, 9 grounded and addressed, 1 refuted on factual grounds. Structural changes: scope contradiction resolved, fix-loop termination locked, Single-Writer tightened, Evaluation Criteria added, Adversarial Diversity operationalized, Human-Gating granularity clarified, Plan Consolidation paired with Living Plan, Hinge Tests broadened to Deferred Decisions Are Tracked, Verifier authority sharpened.

- **R2** (R2-Reviewer-B): 6 findings, all grounded and addressed. Structural changes: Cross-Reference Integrity invariant added, Evaluation Criteria falsifiability strengthened, Major Beats made exhaustive, Artifact Encoding invariant added, audit-store schema promoted to highest priority, grep robustness fixed.

- **R3** (R1-Reviewer-A returning): 12 findings, 11 grounded and addressed, 1 refuted on factual grounds. Structural changes: five must-lock-before-plan contracts promoted into the Charter (Planner Contract, Ship Abstraction, Audit-Store Minimum Schema, Coder Model Pinning, Rollback / Re-Open); Governance Taxonomy added; Evaluation Criteria split into three layers; two new risks; reviewer-independence language normalized; Hardening Notes moved out of Charter to dedicated history file.

- **R4** (R2-Reviewer-B returning): 5 findings, all grounded and addressed. Structural changes: Convergence Safeguards (severity-tiered + human-arbiter); Provisional Lock mechanism; Cascading Invalidation made explicit; Pre-Flight Environment Check; Planner Contract phase-size framed as constraint not mechanism.

The Coordinator's audit of the review trajectory shows a clear convergence pattern: R1 surfaced contradictions and missing structure (10 findings); R2 surfaced integrity and falsifiability gaps (6 findings); R3 surfaced still-deferred contracts (12 findings); R4 surfaced operational edge cases on the newly-locked contracts (5 findings).

The trajectory by severity:

- R1: 4×P1, 5×P2, 1×P3 — major structural issues
- R2: 1×P1, 4×P2, 1×P3 — integrity refinement
- R3: 4×P1, 5×P2, 3×P3 — contract promotion (the heaviest round; correctly identified that more should be Charter-level)
- R4: 3×P1, 1×P2, 1×P3 — operational stress-tests on post-R3 contracts

Findings have not been increasing in proportion of P1-level severity since R3. R4's three P1 findings were specific operational concerns about R3's new contracts (the cascading invalidation gap, the convergence-loop risk, the bottleneck risk on Required Choices) — each is genuinely consequential and each was fixed. But the *kind* of finding has shifted from "structural gap" to "operational refinement," which is the trajectory signal that further rounds would produce diminishing returns.

The Coordinator's standard practice (now codified in the Charter as the Convergence Safeguards) is to audit the review trajectory and call convergence when:

- The next round is unlikely to surface P1 structural gaps
- Further refinement risks crossing from constructive critique into stylistic / pedantic territory
- The artifact is operationally sufficient to be a useful constitutional input to the next stage

All three criteria hold for the post-R4 Charter. Convergence is therefore declared.

## Outstanding items at convergence

The Charter has no outstanding Provisional Locks — no Required Project-Level Choices have been locked provisionally because the Plan stage has not yet begun. (Provisional Locks become relevant when the project enters Plan stage with some Required Choices that need exploratory Plan work to inform them.)

The Charter's Open Items list contains items deferred to Plan stage by design — these are not "outstanding" in the sense of blocking convergence; they are implementation-level work that the Plan will address.

## What this convergence does

- The Charter is the **approved constitutional input** to the Plan stage.
- The Required Project-Level Choices (from Governance Taxonomy) must be locked next, before the Planner produces output. Some will use Charter defaults; some need explicit Coordinator decisions; some may be locked provisionally.
- Once the Choices are locked, the Planner consumes the Charter + Choices and produces a phased Plan packet, which the Coder renders as `anvil_plan.md` (or similar; file system layout is a Choice to lock).
- The Plan is itself an artifact that goes through the Plan Review cycle.

## What this convergence does *not* do

- It does not retire the Charter from future amendment. Any post-Plan-stage discovery that would require Charter-level changes is handled via Charter amendment, which is itself reviewable.
- It does not pre-approve the Plan. The Plan will be reviewed on its own merits per the workflow.
- It does not approve any Required Project-Level Choices. Those are a separate step.

---

## Audit cross-references

- Charter at convergence: `new_project_charter.md` (post-R4 state)
- Hardening history: `CHARTER_HARDENING_HISTORY.md` (contains all four round consolidations)
- Round dispositions: `REVIEW_CHARTER_R1.md`, `REVIEW_CHARTER_R2.md`, `REVIEW_CHARTER_R3.md`, `REVIEW_CHARTER_R4.md`
- Raw reviewer findings packets: present in the conversation transcript pending audit-store schema implementation (the audit-store storage layer is a Plan-stage item; until it exists, the conversation log serves as the audit trail for pre-implementation rounds).

Future Plan-stage reviewers consuming this Charter should treat it as the canonical text. Any reference to "the Charter" in downstream artifacts means this post-R4 state, identified by this convergence declaration.
