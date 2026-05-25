# Plan — R5 Disposition

**Date:** 2026-05-19
**Scope:** Response to R5 findings on the Anvil Implementation Plan (post-R4 state). R5 raised five concerns plus three low-impact notes plus an explicit "Ready to proceed" readiness verdict. Three concerns Fixed; two acknowledged-but-not-fixed (structural observations rather than fix requests). The three low-impact notes are documentary acknowledgments requiring no Plan content change.
**Spec:** `ANVIL_PLAN.md` (updated this round with three tightenings; hardening notes appended to `PLAN_HARDENING_HISTORY.md`).
**Prior rounds:** `REVIEW_PLAN_R1.md`, `REVIEW_PLAN_R2.md`, `REVIEW_PLAN_R3.md`, `REVIEW_PLAN_R4.md`.
**R5 reviewer:** fifth rotation slot from the configured pool; different model family from Coder per Adversarial Diversity floor.

**Shell assumption for verification commands:** POSIX shell utilities (`grep`, `awk`).

---

## What R5 signaled

R5's character is unmistakably a *convergence round*. The reviewer issued an explicit readiness verdict — "Ready to proceed... If those three small tightenings are applied, the plan is solid for execution" — and limited substantive findings to five concerns, only three of which were actionable as small tightenings. The remaining two were structural observations the reviewer flagged for awareness, not fix requests. Three additional low-impact notes were documentary acknowledgments.

This matches the trajectory pattern the Coordinator named during the Charter rounds: findings have shifted across the five Plan rounds from "missing structure" (R1, 11 findings, mostly P1 architectural surface) → "refinements" (R2, 6 findings) → "contradiction-resolution" (R3, 14 findings, language alignment) → "operational edge cases" (R4, 7 findings, boundary conditions) → "minor tightenings + ready-to-proceed" (R5, 3 actionable items).

The reviewer's verdict is itself a meaningful data point. Critical reviewers do not lightly issue "Ready to proceed" — that the R5 reviewer did so, and that the round's substantive surface has compressed to three small tightenings, is the convergence signal.

---

## Verification of R5 finding premises

Each concern was checked against the current Plan state before any edits.

| Concern | Premise to verify | Verified? | Notes |
|---|---|---|---|
| 1 — Windows daemon lifecycle | Windows-specific scenarios (logoff, sleep, antivirus, fast user switching, ungraceful close) not in P11 smoke-test list | ✓ | Real gap; primary platform's edge cases were under-specified |
| 2 — P4 integration surface | "CLI UX audit action" noted in Draft 6 status but P4 acceptance criteria did not explicitly require non-author + clean-machine walkthrough | ✓ | Real gap; the audit-action note was directional, not gating |
| 3 — Partial-output discard economics | Post-v1 checkpoint/resume item lived in Open Items as a vague future note rather than a documented v1.1 design seed | ✓ | Real organizational gap; design intent was eroding into unstructured deferral |
| 4 — Critical path length | Linear spine P0→...→P11 is long; structural observation about cascade risk | ✓ but structural observation | Reviewer flagged for awareness, not fix; restructuring without a clearer signal would re-open converged decisions |
| 5 — Distribution polish | Smoke test does not explicitly verify first-run warning text | ✓ | Real gap addressed by the same tightening as Concern 1 |

Result: 5/5 premises grounded. Three are Fixed; one (Critical path length) is acknowledged-but-not-fixed; one (Distribution polish) is folded into Concern 1's fix.

---

## Disposition of R5 concerns

| # | Severity (Coder-assigned) | Concern | Disposition |
|---|---|---|---|
| 1 | P2 | Windows daemon lifecycle edge cases on primary platform | **Fixed.** P11 smoke-test list now includes five Windows-only scenarios: user logoff, laptop close-lid / sleep, fast user switching, antivirus quarantine, ungraceful terminal close. Each has defined acceptance. Smoke test also explicitly verifies first-run unsigned-binary warning text (addresses Concern 5 in the same edit). |
| 2 | P2 | P4 integration surface area; non-author walkthrough on clean Windows machine | **Fixed.** P4 acceptance criterion #12 added: a non-author reviewer walks the wizard end-to-end on a clean Windows machine (no prior install, daemon, registry, keychain entries) and records the walkthrough as `docs/p4-walkthrough.md`. The walkthrough is a P4 ship gate. |
| 3 | P3 | Partial-output discard economics; v1.1 design intent eroding into unstructured deferral | **Fixed.** New *v1.1 Design Seeds* appendix added near the end of the Plan. Two prior Open Items (checkpoint/resume for long sidecar streams; global sidecar sharing across workspaces) promoted to the appendix. Three other post-v1 items consolidated into the appendix (cryptographic tamper-proofing of audit store; reconsidering file-based credential encryption; hard-stop cost-limit policy evolution). Each seed carries: problem statement, constraints any v1.1 design must preserve, v1 data points expected to inform the design. |
| 4 | P3 (observation) | 15-phase critical path length | **Acknowledged, not fixed.** Real risk to monitor during execution, but the reviewer's suggestions (tighten P3c, add a thin vertical slice in P4) are speculative — restructuring phases without a clearer signal would risk re-opening converged decisions. The risk is logged in the R5 hardening notes as something to watch; no Plan content change. |
| 5 | P3 | Distribution polish — smoke test warning text | **Fixed via Concern 1 edit.** The same Distribution smoke-test edit that added Windows-specific scenarios also added "explicitly verify the exact text of the unsigned-binary warning each OS displays on first run." Single edit covers both concerns. |

**Three low-impact notes** (single-active-project scoping; latency budgets recorded by P11; cross-reference integrity and convergence-declaration are good safeguards) are documentary acknowledgments only. P11's metric collection already records end-to-end times via the *Human minutes per shipped phase* metric. No Plan content change needed.

---

## Files changed since R4

| File | Action | Purpose | Approximate delta |
|---|---|---|---|
| `ANVIL_PLAN.md` | MODIFY | Three tightenings: (1) P11 Distribution smoke-test additions (Windows-specific daemon scenarios + first-run warning text verification); (2) P4 acceptance criterion #12 (clean-Windows-machine first-time-user walkthrough); (3) new *v1.1 Design Seeds* appendix promoting two prior Open Items and consolidating three other post-v1 items. | +~120 lines net |
| `PLAN_HARDENING_HISTORY.md` | MODIFY | Append `Hardening Notes (R5 — Consolidated)` section. | +~75 lines |
| `REVIEW_PLAN_R5.md` | CREATE | This document. | ~180 lines |
| `REVIEW_PLAN_R1.md` through `REVIEW_PLAN_R4.md` | (UNTOUCHED) | Per single-writer artifact discipline. |

---

## Corrections to prior dispositions

None this round. R5 did not flag any prior round's disposition language as wrong.

---

## Residual / deferred

The Plan's open items list is now substantially smaller because R5 promoted two items to the *v1.1 Design Seeds* appendix and consolidated three others into the same appendix. Remaining Open Items in v1 are:

- Audit store query language (richer than `anvil audit list` / `audit show`; deferred beyond v1)
- Concurrent project support (out of v1 scope by design)
- Reviewer prompt management (deferred to v1.1)
- Reviewer findings deduplication (deferred to v1.1)
- Performance characterization (post-P11)
- Distribution open work for v1 release mechanics (acceptance now locked; mechanics still being refined)
- Auto-update (out of v1 scope)
- External pilot project selection (resolved in P11)
- `anvil init` vs `anvil setup` naming (resolved in Draft 5; documentation only)

The *v1.1 Design Seeds* appendix carries the post-v1 design intent for checkpoint/resume, global sidecar sharing, cryptographic tamper-proofing, file-based credential encryption reconsideration, and hard-stop cost-limit policy evolution.

---

## Reproducibility

```bash
# --- R5 #1 — Windows daemon robustness scenarios in P11 ---
awk '/^### \*\*P11/,/^## /' ANVIL_PLAN.md | grep -E "Windows-specific daemon robustness|User logoff|Laptop close-lid|Fast user switching|Antivirus quarantine|Ungraceful terminal close"
# Expected: ≥5 matches in P11 section.

# --- R5 #1 (also covers Concern 5) — first-run warning text verification ---
awk '/^- \*\*Distribution\.\*\*/,/^- \*\*/' ANVIL_PLAN.md | grep "exact text of the unsigned-binary warning"
# Expected: 1 match in Distribution open-item entry.

# --- R5 #2 — P4 clean-Windows-machine walkthrough ---
awk '/^### \*\*P4/,/^### \*\*P5/' ANVIL_PLAN.md | grep -E "Clean-Windows-machine first-time-user walkthrough|p4-walkthrough.md"
# Expected: ≥2 matches in P4 section.

# --- R5 #3 — v1.1 Design Seeds appendix ---
grep -n "^## v1.1 Design Seeds" ANVIL_PLAN.md
# Expected: 1 match.

grep -E "^### Seed [1-5]:" ANVIL_PLAN.md
# Expected: 5 matches (Seed 1: Checkpoint/resume; Seed 2: Global sidecar sharing; Seed 3: Cryptographic tamper-proofing; Seed 4: File-based credential encryption; Seed 5: Hard-stop cost-limit policy evolution).

# --- R5 #3 — promoted items removed from Open Items as standalone entries ---
awk '/^## Open Items \(Plan Stage\)/,/^---$/' ANVIL_PLAN.md | grep -c "Checkpoint/resume for long sidecar streams"
# Expected: 0 (no longer in Open Items; moved to v1.1 Design Seeds).

awk '/^## Open Items \(Plan Stage\)/,/^---$/' ANVIL_PLAN.md | grep -c "Global sidecar sharing across workspaces"
# Expected: 0 (same reason).

# --- R5 hardening notes appended ---
grep -n "^## Hardening Notes (R5 — Consolidated)" PLAN_HARDENING_HISTORY.md
# Expected: 1 match.
```

---

## Bottom line — convergence candidate

R5 is a convergence round. The reviewer's explicit "Ready to proceed" verdict, combined with the trajectory across five rounds (structural gaps → refinements → contradiction-resolution → operational edge cases → minor tightenings), is the strongest convergence signal the Plan has produced.

The three R5 tightenings are now applied. The two structural observations (critical path length; structural observation about restructuring) are acknowledged in hardening notes but not fixed — they would require speculative restructuring without clearer signal. The three low-impact notes are documentary acknowledgments requiring no Plan content change.

**The Coordinator may reasonably invoke human-arbiter convergence on the Plan at this point** per the *Convergence Safeguards* mechanism. Doing so would log a `ConvergenceDeclaration` audit record naming the post-R5 state as the canonical Plan version and end the Plan Review cycle.

The alternative — sending the post-R5 state to the prior reviewer (R4's reviewer family) for one more confirmation pass — is also defensible. Per the locked default termination condition (full-pool clean), the formally-correct path is for every reviewer in the pool to have produced a clean pass on the *same* state. R5 has produced one clean pass on the post-R4 state plus three small tightenings; R4's reviewer family has not yet seen the post-R5 state. A literal full-pool-clean read would require one more pass.

The trade-off is well-defined: human-arbiter convergence ships sooner with slightly weaker formal guarantee; one more rotation pass ships later with full-pool-clean satisfied. R5's "Ready to proceed" verdict is itself the reviewer's signal that the post-R5 state is unlikely to surface new substantive findings. The decision rests with the Coordinator.

**Either path leads to P0 starting next.** No further architectural surface area is expected.

**Charter Amendment A1 review remains a separate concurrent workstream** and is independent of the Plan convergence decision. The Plan can converge before, after, or in parallel with Amendment A1; the v1 Plan currently references the Amendment's intended invariants as Plan-level commitments, which means the Plan does not block on Amendment convergence.
