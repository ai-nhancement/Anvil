# Anvil Plan — Convergence Declaration

**Record type:** `convergence-declaration` (per Audit-Store Minimum Schema invariant).
**Date:** 2026-05-19
**Declared by:** John Canady (Coordinator)
**Declared on artifact:** `ANVIL_PLAN.md` (post-R5 state)
**Mechanism invoked:** Human Arbiter Authority (per *Plan-Level Convergence Safeguards* — same mechanism that closed the Charter at R4)

---

## Declaration

The Anvil Implementation Plan is declared **convergent**. No further review rounds will be conducted on the v1 Plan. The post-R5 state is approved and is now the constitutional input to the Build stage. P0 (Bootstrap) is unblocked.

## Reasoning

Five Plan Review rounds were completed under deterministic rotation:

- **R1** (first reviewer family): 11 findings, mostly P1 architectural surface. Trust-boundary invariants promoted to Plan-level locks; sidecar lifecycle locked; recoverability framework added; phase-acceptance criteria inlined; sidecar-driven Charter contradictions resolved.
- **R2** (second reviewer family): 6 findings. Phase 10 split into P10a (Evaluation Infrastructure) and P10b (Hinge-Test Framework); audit-store integrity check extended to detect physical record deletion; P4 acceptance added CI/headless env-var API-key bypass; P11 added CLI UX audit action; partial-output cost trade-off documented.
- **R3** (first reviewer family returning): 14 findings (5 P1, 5 P2, 4 P3). Internal contradictions resolved across phase count, provider routing, streaming invariant, hinge-framework timing, P11 ship gate boundedness, hinge totals, audit-store threat model, brittle-pin tests, distribution acceptance, `anvil-graph` naming, headless operation, cost controls, single-active-project semantics.
- **R4** (second reviewer family returning): 7 findings. Operational edge cases — split-brain state drift with config-epoch handshake, daemon zombie management via global registry, per-finding arbiter resolution (`ArbiterFindingResolution`), bi-language hinge consensus check, keyring fallback removed in favor of env-var floor, rollback resets rotation, pilot provider-diversity requirement.
- **R5** (first reviewer family returning, third pass): 3 actionable tightenings + 2 structural observations + 3 low-impact documentary notes + explicit "Ready to proceed" readiness verdict. Windows-specific daemon robustness in P11, P4 clean-Windows first-time-user walkthrough, v1.1 Design Seeds appendix.

The Coordinator's audit of the trajectory shows clear convergence:

| Round | Finding count | P1 count | Character |
|---|---|---|---|
| R1 | 11 | 4 | Structural gaps |
| R2 | 6 | 1 | Refinements |
| R3 | 14 | 5 | Contradiction-resolution / language alignment |
| R4 | 7 | 2 | Operational edge cases |
| R5 | 5 (3 actionable) | 0 | Minor tightenings + Ready-to-proceed verdict |

The trajectory matches the same pattern that closed the Charter at R4: findings shifting from "missing structure" through "operational refinement" to "minor tightenings + reviewer-issued readiness verdict." Further rounds would produce diminishing returns.

The Coordinator's standard practice (codified in the Charter as the *Convergence Safeguards*) is to declare convergence when:

- The next round is unlikely to surface P1 structural gaps. *Met:* R5 produced zero P1 findings and the reviewer explicitly signaled the architecture is solid.
- Further refinement risks crossing from constructive critique into stylistic or pedantic territory. *Met:* R5's "structural observations" (critical path length, distribution polish text) are exactly that — useful awareness, not actionable structural change.
- The artifact is operationally sufficient to be a useful constitutional input to the next stage. *Met:* R5's "Ready to proceed" verdict from a critical cross-vendor reviewer is the strongest external signal of this.

All three criteria hold for the post-R5 Plan.

## Outstanding items at convergence

**No outstanding Provisional Locks** are blocking. The Plan carries five Provisional Locks (Plan Consolidation triggers, per-metric numeric thresholds, file system layout, deferred-decision tracking mechanism, ship transport actions, runtime alert response policies) — each has an explicit revision trigger tied to P11 dogfooding and pilot data. Per the *Required Project-Level Choices* mechanism, Provisional Locks satisfy the pre-Build-stage gate; convergence proceeds.

**v1.1 Design Seeds** (five items in the appendix added in R5) are explicitly post-v1 and do not block. They are queryable design seeds for the next iteration, not unresolved v1 questions.

**Open Items** remaining in the Plan are all explicitly scoped as post-v1 or deferred to v1.x (audit store query language; concurrent project support; reviewer prompt management; reviewer findings deduplication; performance characterization; distribution release mechanics refinement; auto-update; external pilot project selected in P11 (Leaflog, representative Gate 1 artifacts; live execution deferred to Gate 2); `anvil init` vs `anvil setup` naming resolved in Draft 5). None block.

## What this convergence does

- The Plan is the **approved constitutional input** to the Build stage.
- P0 (Bootstrap) is unblocked. The Coder may begin implementation immediately on the Coordinator's go-ahead.
- All future references to "the Plan" in downstream artifacts (phase briefings, dispositions, hardening notes) point to this post-R5 state as the canonical Plan version.
- The Charter Amendment A1 review remains a separate, concurrent workstream and is independent of Plan convergence. v1 Plan references Amendment A1's intended invariants as Plan-level commitments, so the Plan does not block on Amendment convergence.

## What this convergence does *not* do

- It does not retire the Plan from future amendment. Any Build-stage discovery requiring Plan-level changes is handled via Plan amendment, which is itself reviewable.
- It does not pre-approve any phase. Each phase goes through its own Review cycle.
- It does not retire the Open Items or v1.1 Design Seeds — they remain queryable.
- It does not finalize Provisional Locks. Those are resolved when their respective revision triggers fire.

## Audit cross-references

- Plan at convergence: `ANVIL_PLAN.md` (post-R5 state)
- Plan hardening history: `PLAN_HARDENING_HISTORY.md` (contains all five round consolidations)
- Round dispositions: `REVIEW_PLAN_R1.md`, `REVIEW_PLAN_R2.md`, `REVIEW_PLAN_R3.md`, `REVIEW_PLAN_R4.md`, `REVIEW_PLAN_R5.md`
- Charter (parent artifact, already converged): `new_project_charter.md`, `CHARTER_CONVERGENCE.md`
- Charter Amendment A1 (concurrent workstream, in R1 review): `CHARTER_AMENDMENT_A1.md`
- Artifact Specifications (concurrent workstream, in R1 review): `ARTIFACT_SPECIFICATIONS.md`

Raw reviewer findings packets across the five Plan rounds are preserved in the conversation transcript pending audit-store schema implementation (which is itself a P2 deliverable in the now-converged Plan).
