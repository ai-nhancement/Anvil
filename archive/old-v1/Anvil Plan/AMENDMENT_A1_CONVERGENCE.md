# Charter Amendment A1 — Convergence Declaration

**Record type:** `convergence-declaration` (per Audit-Store Minimum Schema invariant).
**Date:** 2026-05-19
**Declared by:** John Canady (Coordinator)
**Declared on artifact:** `CHARTER_AMENDMENT_A1.md` (Draft 3 / post-R2 state)
**Mechanism invoked:** Human Arbiter Authority (per *Plan-Level Convergence Safeguards*).

---

## Declaration

Charter Amendment A1 is declared **convergent**. The three constitutional additions (Open-Source Distribution, Defined Artifact Structures, Embeddable Workflow Infrastructure) are approved and applied to the Anvil Project Charter as of this date.

The amendment's specific content — three new invariants, fifteen new Required Project-Level Choices, the Contract Inventory, the Public vs Private Audit Records mechanism, the Governance Mechanics, the Security Posture as constitutional, the Embedding Invariants, the Structured CLI Output Stability rules, the Publication Milestone, the Repo-Readiness Acceptance Gates, the Per-Item Disposition Mechanism, the Cross-Document Convergence rules, and the DCO Extension — is incorporated into the Charter via the new *Amendment A1 — Applied* section in `new_project_charter.md`.

The Amendment document (`CHARTER_AMENDMENT_A1.md`) remains as a historical artifact. The amendment hardening history (`AMENDMENT_A1_HARDENING_HISTORY.md`) and the two round dispositions (`REVIEW_CHARTER_AMENDMENT_A1_R1.md`, `REVIEW_CHARTER_AMENDMENT_A1_R2.md`) likewise remain queryable.

`ARTIFACT_SPECIFICATIONS.md` was edited mid-flight during R2 to resolve cross-document inconsistency on the spec amendment process. That edit applied the new *Cross-Document Convergence* rules in practice — Item 2 cannot fully ship until the spec document itself converges through its own concurrent R1 review (which proceeds as a separate workstream).

## Reasoning

Two Amendment Review rounds were completed under deterministic rotation:

- **R1** (first reviewer family): 15 findings (5 P1, 5 P2, 5 P3). Major structural additions: Contract Inventory, Public vs Private Audit Records, Security Posture as constitutional, Governance Mechanics, spec amendment path tightened, Per-Item Disposition Mechanism, Embedding Invariants, Structured CLI Output Stability, DCO Extension, Trademark Posture (Coordinator decision pending; locked Posture A on 2026-05-19), Publication Milestone, Repo-Readiness Gates, Plan Draft 7 Impact Matrix requirement, AiMe reframed.
- **R2** (second reviewer family): 12 findings (4 Fix First, 4 Important, 4 Medium). Consistency cleanup per the reviewer's explicit guidance to not reopen architectural surface. Major outcomes: trademark consistency restored; spec governance aligned cross-document (first application of *Cross-Document Convergence* rules); Publication-Safe Git History Gate added; all audit records private by default; tools-vs-outcomes separation; BDFL-adversarial emergency-freeze; DCO definitions; schema discovery as requirement; v1.2 transport loosened to principles.

The Coordinator's audit of the trajectory:

| Round | Finding count | Severity mix | Character |
|---|---|---|---|
| R1 | 15 | 5 P1 / 5 P2 / 5 P3 | Major structural additions |
| R2 | 12 | 4 Fix First / 4 Important / 4 Medium | Consistency cleanup + targeted refinements |

This matches the convergence pattern observed on the original Charter (R1→R4) and the Plan (R1→R5): findings shift from "missing structure" through "operational refinement" to "consistency cleanup with positive reviewer signal."

The Coordinator declared convergence rather than running R3 because:

- The R2 reviewer's guidance was explicit and constructive ("produce Draft 3 focused only on consistency cleanup" — a signal that the substance was settled).
- All 12 R2 findings are closed in Draft 3 with substantive content, not deferred.
- The amendment's three core commitments have not changed in intent across both rounds; only their operational guardrails have been added and tightened.
- The remaining open items at convergence are all Plan-stage workstreams (Plan Draft 7 record-type reconciliation, impact matrix production, `GOVERNANCE.md` / `SECURITY.md` / `CONTRIBUTING.md` / `TRADEMARK.md` actual drafting, `schemas/cli/` per-command schema authoring) — none of which block constitutional convergence.

## Outstanding items at convergence

**No Provisional Locks remain on the amendment side.** Trademark Posture A was Coordinator-locked during R1; all other Required Choices are Final with explicit values or constitutional outcomes.

**Plan Draft 7 is required as the immediate next downstream workstream.** It must:

- Reconcile audit-store record-type counts (Plan currently states 13; amendment adds `PublicVisibilityPolicy`, `PublicExportApproval`, `EmergencyFreezeDeclaration` → new total is 16).
- Produce the impact matrix promised in the amendment's *Plan Draft 7 Impact Matrix* section.
- Integrate the Publication-Safe Git History Gate into P11 acceptance.
- Add the public-export bundle mechanism (`anvil audit export --public`) to P2's audit-store work.
- Update P3a's `InvokeRequest` schema if any embedding-related fields shift (unlikely given Medium #12's loosening).
- Add the `--describe-schema` CLI requirement to P5–P10a CLI command implementations.
- Update P0 acceptance to include the new repo-readiness gates (LICENSE, NOTICE, etc.) where they were not already covered.

**`ARTIFACT_SPECIFICATIONS.md` own R1 review remains a separate workstream.** The spec's R1 review may surface findings that affect the artifact-structures invariant; per *Cross-Document Convergence*, Item 2 is not fully shipped until both documents have converged through their respective review cycles. Item 2's constitutional invariant is locked here (so the principle is now Charter-level), but operational specifics in the spec may still evolve through narrow review.

## What this convergence does

- The amendment's three new invariants are now **Charter invariants** (Never Violate). They are integrated into the Charter body via the new *Amendment A1 — Applied* section.
- The amendment's Required Choices are now part of the Charter's Governance Taxonomy. (Listed in the amendment-applied section with a forward reference to *Governance Taxonomy*.)
- The amendment's supporting sections (Contract Inventory, Public/Private Audit Records, Governance Mechanics, Security Posture, DCO Extension, Trademark Posture, Publication Milestone, Embedding Invariants, Structured CLI Output Stability, Repo-Readiness Acceptance Gates, Per-Item Disposition Mechanism, Cross-Document Convergence) are referenced from the Charter body; the full text remains in `CHARTER_AMENDMENT_A1.md` to keep the Charter's narrative readable.
- The Charter's status header is updated to reflect the amendment-applied state.
- `CHARTER_HARDENING_HISTORY.md` receives an "Amendment A1 — Applied" entry summarizing the convergence.
- The Amendment document's status is bumped to "Converged."

## What this convergence does *not* do

- It does not retire the amendment from future further amendment. Any post-convergence discovery that requires changes goes through a new Charter amendment cycle.
- It does not unlock the Plan from review. The Plan is already converged at R5; the amendment's downstream impact is handled via a Plan amendment (Draft 7) that itself goes through review.
- It does not converge `ARTIFACT_SPECIFICATIONS.md`. That document's R1 review proceeds separately.
- It does not authorize publication. Publication happens at the v1-ship moment per the Publication Milestone gate, which includes the publication-safe git history scan.

## Audit cross-references

- Amendment at convergence: `CHARTER_AMENDMENT_A1.md` (Draft 3 / post-R2 state)
- Amendment hardening history: `AMENDMENT_A1_HARDENING_HISTORY.md` (R1 + R2 + Coordinator decisions)
- Round dispositions: `REVIEW_CHARTER_AMENDMENT_A1_R1.md`, `REVIEW_CHARTER_AMENDMENT_A1_R2.md`
- Charter (parent artifact): `new_project_charter.md` (updated with Amendment A1 — Applied section)
- Charter convergence (original R4): `CHARTER_CONVERGENCE.md`
- Plan (parent dependent artifact, already converged): `ANVIL_PLAN.md`, `PLAN_CONVERGENCE.md`
- Artifact Specifications (concurrent workstream): `ARTIFACT_SPECIFICATIONS.md` (still in own R1 review)

Raw reviewer findings packets across the two amendment rounds are preserved in the conversation transcript pending audit-store schema implementation.
