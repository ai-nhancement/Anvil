# Charter — R4 Disposition

**Date:** 2026-05-15
**Scope:** Response to R4 findings on the Anvil Project Charter (post-R3 state). R4 raised five findings, all conceptual / structural rather than line-cited. All five accepted; severity tiers (Coder-assigned): 3 × P1, 1 × P2, 1 × P3.
**Spec:** `new_project_charter.md` (sharpened this round; hardening notes appended to `CHARTER_HARDENING_HISTORY.md`).
**Prior rounds:** `REVIEW_CHARTER_R1.md`, `REVIEW_CHARTER_R2.md`, `REVIEW_CHARTER_R3.md`.
**R4 reviewer:** `R2-Reviewer-B` returning per deterministic rotation. This is R2-Reviewer-B's first time seeing the post-R3 state.

**Shell assumption for verification commands:** POSIX shell utilities (`grep`, `awk`). On non-POSIX environments, each command's intent is described inline.

---

## What changed since R3

R4 was a confirmation-with-targeted-strengthening round rather than a structural reorganization. The R3 changes — Governance Taxonomy, the five must-lock-before-plan contracts, the three-layer Evaluation Criteria split, the move of hardening notes to a separate file — were accepted in substance. R4's five concerns were about how the newly-locked contracts would behave operationally.

The two largest gains in this round:

- **Convergence Safeguards** added to the Phase Review subsection. The termination condition was previously "every reviewer in the pool has produced a clean pass on the most recent state" — open-ended and exposed to the *Two Generals' Problem* failure mode where reviewers could alternate minor stylistic findings indefinitely. The new safeguards pair a severity-tiered convergence rule (after a configurable round limit, default 5, only P1 findings block; P2 / P3 in rounds 6+ are advisory) with explicit human-arbiter authority (the Coordinator may declare convergence at any time; the declaration is itself an audit record).
- **Provisional Lock** mechanism added to Governance Taxonomy. R4 noted that Required Project-Level Choices that "must be locked before Plan stage begins" creates deadlock risk for choices that genuinely need exploratory Plan-stage work to inform them. The Provisional Lock allows hypothesis-based commitment with an explicit revision trigger; the workflow proceeds; the Choice is re-evaluated when the trigger fires.

The three smaller gains: Cascading Invalidation in Rollback / Re-Open is now explicit (transitive closure, not just direct dependents); the Pre-Flight Environment Check is layered onto the Artifact Encoding invariant; the Planner Contract's phase-size rule is explicitly framed as a constraint, not a mechanism.

Three new audit-store record types were surfaced by this round (`convergence-declaration`, `provisional-lock`, `rollback-event`); the Audit-Store Minimum Schema invariant's required-record-types list now includes them.

---

## Verification of R4 finding premises

R4 was a conceptual review. Each finding's premise was verified against the current Charter state before amendments were applied.

| Finding | Premise to verify | Verified? | Notes |
|---|---|---|---|
| 1 — Required Project-Level Choices Bottleneck | Governance Taxonomy hard-requires lock before Plan begins; no provisional mechanism exists | ✓ | Confirmed at line "A project that has not locked all *Required Project-Level Choices* cannot enter the Plan stage" |
| 2 — Planner Contract phase-size constraint vs mechanism | Phase size rule currently phrased as constraint, but framing is implicit | ✓ | Pre-R4 wording was constraint-like but did not explicitly label itself or preempt mechanism creep |
| 3 — Rollback semantics need explicit cascading invalidation | Pre-R4 wording: "invalidates all phases that declared a dependency on the re-opened phase" — readable as direct dependents only | ✓ | Transitive invalidation was implicit; could be misread |
| 4 — Reviewer Sign-off Loop / Two Generals' Problem | Pre-R4 termination condition was open-ended; no convergence threshold, no human-arbiter mechanism | ✓ | The rotation could in principle iterate indefinitely on minor findings |
| 5 — UTF-8 environment hostility (Pre-Flight Check) | Pre-R4 Artifact Encoding invariant specified lint at audit-store boundary but not at tool-environment boundary | ✓ | Mitigation defended the artifact, not the tool's view of it |

Result: 5/5 premises grounded. All five findings accepted.

---

## Disposition of R4 findings

| # | Severity (Coder-assigned) | Finding (one-line) | Disposition |
|---|---|---|---|
| 1 | P1 | Required Project-Level Choices Bottleneck — premature commitment risk | **Fixed.** Provisional Lock mechanism added to Governance Taxonomy. A Required Choice may be locked provisionally with a hypothesis and revision trigger; satisfies the pre-Plan-stage gate; revision happens when trigger fires in Plan stage. Charter cannot ship while any Provisional Lock is outstanding. New audit record type: `provisional-lock`. |
| 2 | P3 | Planner Contract phase-size might be read as mechanism | **Fixed.** Phase-size rule now explicitly labeled "(constraint, not mechanism)" with a clarifying sentence that implementation of phase-size measurement is Plan-stage and the Charter must not be read as mandating a specific mechanism. |
| 3 | P1 | Rollback semantics need explicit cascading invalidation | **Fixed.** Rollback / Re-Open invariant now has an explicit *Cascading Invalidation (blast radius)* paragraph: re-opening invalidates the **transitive closure** of dependents through the dependency graph, not just direct dependents. Coordinator is shown the full blast radius at re-open time and approves it before commit. New audit record type: `rollback-event`. |
| 4 | P1 | Reviewer Sign-off Loop / Two Generals' Problem | **Fixed.** Convergence Safeguards added to Phase Review subsection. *Severity-tiered convergence:* after a configurable round limit (default 5), only P1 findings block; P2 / P3 in rounds 6+ become advisory. Round limit added to Required Project-Level Choices. *Human arbiter authority:* Coordinator may declare convergence at any time, logged as `convergence-declaration` audit record with reasoning. |
| 5 | P2 | UTF-8 environment hostility | **Fixed.** Artifact Encoding invariant now specifies two layered protections: the existing lint check at the audit-store boundary (defends the artifact) and a new Pre-Flight Environment Check (defends the tool's view of the artifact). Pre-flight validates the tool's runtime is UTF-8 configured before it interacts with project artifacts. |

---

## Files changed since R3

| File | Action | Purpose | Approximate delta |
|---|---|---|---|
| `new_project_charter.md` | MODIFY | Apply 5 amendments. Add Provisional Lock paragraph to Governance Taxonomy. Add round-limit Choice to Required Project-Level Choices. Add Convergence Safeguards block to Phase Review. Make Cascading Invalidation explicit in Rollback / Re-Open. Add Pre-Flight Environment Check to Artifact Encoding. Update Audit-Store Minimum Schema record types list (add `convergence-declaration`, `provisional-lock`, `rollback-event`). Frame Planner Contract phase-size as constraint, not mechanism. | +~50 lines net |
| `CHARTER_HARDENING_HISTORY.md` | MODIFY | Append `Hardening Notes (R4 — Consolidated)` section. | +~80 lines |
| `REVIEW_CHARTER_R4.md` | CREATE | This document. | ~200 lines |
| `REVIEW_CHARTER_R1.md`, `R2.md`, `R3.md` | (UNTOUCHED) | Per single-writer artifact discipline. |

---

## Corrections to prior dispositions

None this round. R4 did not flag any R3 disposition language as wrong; R3's tightened terminology (Fixed / Locked in Charter, enforcement pending Plan / Refuted) is preserved.

---

## Residual / deferred

R4 did not surface new Plan-stage items beyond what R3 already enumerated. The Open Items list is unchanged in structure; one item-level expansion: the Audit-Store Minimum Schema work is now slightly larger because three new record types must be specified (`convergence-declaration`, `provisional-lock`, `rollback-event`).

One observation from the R4 round that flags a future operational concern: every charter-level mechanism that introduces an audit record type also introduces a Plan-stage implementation burden. The Plan's audit-store schema work is no longer just "the original 8 record types" — it's now 11 types plus their cross-reference keys. This isn't a residual finding; it's a useful data point: the Charter's audit-store schema invariant is doing its intended work of forcing the Plan to be explicit about every machine-readable touchpoint.

---

## Reproducibility

**Shell assumption:** POSIX shell utilities (`grep`, `awk`). On non-POSIX environments, each command's intent is described inline.

```bash
# --- R4 #1 — Provisional Lock mechanism present in Governance Taxonomy ---
# Intent: confirm the Provisional Lock escape hatch is documented.

awk '/^## Governance Taxonomy/,/^---$/' new_project_charter.md | grep -E "Provisional Lock|revision trigger"
# Expected: ≥2 matches inside the Governance Taxonomy section.

# --- R4 #2 — Phase size rule explicitly labeled as constraint not mechanism ---
# Intent: confirm the framing is explicit, not implicit.

grep -n "Phase size rule (constraint, not mechanism)" new_project_charter.md
# Expected: 1 match in the Planner Contract block.

# --- R4 #3 — Cascading Invalidation explicit in Rollback / Re-Open ---
# Intent: confirm transitive invalidation is named.

awk '/\*\*Rollback \/ Re-Open\.\*\*/,/\*\*Artifact Encoding\.\*\*/' new_project_charter.md | grep -E "transitive closure|blast radius|Cascading Invalidation"
# Expected: ≥2 matches inside the Rollback invariant.

# --- R4 #4 — Convergence Safeguards added to termination condition ---
# Intent: confirm severity-tiered + human-arbiter safeguards are documented.

awk '/^### Phase Review/,/^### Ship/' new_project_charter.md | grep -E "Convergence safeguards|Severity-tiered convergence|Human arbiter authority"
# Expected: 3 matches inside the Phase Review subsection.

# --- R4 #5 — Pre-Flight Environment Check added to Artifact Encoding ---
# Intent: confirm the second layer of UTF-8 protection is in the invariant.

awk '/\*\*Artifact Encoding\.\*\*/,/^---$/' new_project_charter.md | grep -E "Pre-Flight Environment Check"
# Expected: ≥1 match inside the Artifact Encoding invariant.

# --- Side-effects — Audit-Store record types extended ---
# Intent: confirm the three new record types are listed in the schema invariant.

grep -E "convergence-declaration|provisional-lock|rollback-event" new_project_charter.md
# Expected: ≥3 matches (one per new type), at minimum in the Required record types list inside Audit-Store Minimum Schema.

# --- Hardening history extended ---
# Intent: confirm R4 hardening notes are appended to history file, not to Charter.

grep -c "^## Hardening Notes" new_project_charter.md
# Expected: 0 (Charter contains no Hardening Notes sections; provenance lives in history).

grep -c "^## Hardening Notes" CHARTER_HARDENING_HISTORY.md
# Expected: 4 (R1, R2, R3, R4).
```

---

## Bottom line

R4 was a high-quality second-pass-on-the-opposite-side round. The reviewer accepted R3's structural work and used the round to stress-test how the new contracts would behave under operational pressure. Three of five findings prevented genuinely high-cost failure modes — workflow deadlock (R4 #1), incomplete rollback semantics (R4 #3), and the Two Generals' loop (R4 #4) — each of which would have been expensive to discover during Plan or Build stage.

The Charter is now harder to break in the failure modes the round identified. The Governance Taxonomy has an explicit escape hatch (Provisional Lock). The Rollback invariant has explicit transitive semantics. The termination condition has explicit convergence safeguards. The Artifact Encoding invariant defends both the artifact and the tools that interact with it.

Per the locked termination condition (full-pool clean default), the next rotation step is **R1-Reviewer-A producing a clean confirmation pass on the post-R4 state**. The Charter does not ship until both reviewers' most recent passes come back clean on the same state. This is now round 5 of rotation (R1 / R2 / R3 / R4 done; R5 next). Under the *Convergence Safeguards* introduced in this round, R5 itself is still pre-threshold — full P1 / P2 / P3 findings would still be in scope. If R5 surfaces only P2 / P3 findings and the rotation has converged on no new P1 surface area, the Coordinator may invoke the Severity-tiered convergence rule (round 6+ → only P1 blocks) or declare human-arbiter convergence on the post-R5 state, whichever the project's policy permits.
