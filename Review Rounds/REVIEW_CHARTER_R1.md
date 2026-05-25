# Charter — R1 Disposition

**Date:** 2026-05-15
**Scope:** Response to R1 findings on the Anvil Project Charter. R1 raised ten findings (4 × P1, 5 × P2, 1 × P3). Eight were accepted and folded into the Charter; one was partially accepted with explicit boundary; one was partially refuted on factual grounds with the underlying concern still addressed.
**Spec:** `new_project_charter.md` (updated this round; Hardening Notes (R1 — Consolidated) appended)
**Prior round:** R1 reviewer findings, raw packet (logged to conversation transcript; will move to audit store once the store schema is specified in the Plan stage)
**R1 reviewer:** pseudonymized as `R1-Reviewer-A`. Per Charter draft 1, the reviewer must be drawn from a different model family than the Coder (Claude). Vendor-tagging convention is a Plan-stage item.

---

## What changed since R1

R1 was the first independent review of the Anvil Charter. The reviewer's claims were verified line-by-line against the source before any Charter edits were made (per the *Findings Are Grounded* invariant). Verification produced one factual refutation (P3 #10, "encoding corruption" is a misframing — the cited characters are valid UTF-8 typography) and nine grounded findings.

The Charter was then amended in place. The amendments fall into three buckets:

- **Contradictions resolved.** The single-active-vs-multi-project tension and the single-writer-vs-audit-store tension were both real and both fixed by tightening language. The Charter no longer says one thing in one section and a contradictory thing in another.
- **Deferred decisions promoted.** Two items (fix-loop termination, quality measurement) were treated by the original draft as Open Items, but R1 correctly noted they are too load-bearing to defer. Both are now Charter decisions with explicit defaults plus override mechanisms.
- **Invariants sharpened or re-scoped.** Five invariants got more precise wording: Single-Writer, Adversarial Diversity, Human Gates, Findings Are Grounded (with paired Verifier scope), and the old Hinge Tests invariant (renamed and broadened to Deferred Decisions Are Tracked). Two new invariants were added: Plan Consolidation and a paired note on the audit-store distinction.

A new section `Evaluation Criteria` was added with six measurable metrics that turn the "produces higher-quality output" claim from rhetoric into a checkable hypothesis. Thresholds per metric are deferred to the Plan stage (now an Open Item in its own right).

---

## Verification of R1 claims

Each R1 finding cited specific line numbers in the Charter as it stood pre-amendment. Each citation was checked by reading the source at the cited line. The table below records the verification result *before* any edits were applied.

| Finding | Severity | Citations | Verified? | Notes |
|---|---|---|---|---|
| 1 — Scope contradiction | P1 | L59, L176 | ✓ | Both lines read as quoted; contradiction is real |
| 2 — Fix-loop termination deferred | P1 | L128, L206 | ✓ | L128 cross-refs Open Items; L206 explicitly defers |
| 3 — Single-Writer wording | P1 | L169 (Audit Trail), L117 (Verifier statuses) | ✓ | Both anchors correct; tension is real |
| 4 — Quality claim unmeasured | P1 | L25, L269 | ✓ | "dramatically higher quality" appears verbatim in both locations |
| 5 — Diversity rule under-specified | P2 | L39, L155, L256 | ✓ | All three references hold; vendor blur acknowledged in question 3 |
| 6 — Human-gating ceremony risk | P2 | L157, L226 | ✓ | Invariant and risk-mitigation both present as cited |
| 7 — Living Plan condensation | P2 | L163, L259 | ✓ | Invariant and reviewer-question both present |
| 8 — Hinge Tests elevated prematurely | P2 | L165, L213 | ✓ | Invariant statement and machinery-deferred Open Item both correct |
| 9 — Verifier authority unclear | P2 | L159, L161 | ✓ | Both invariants present; tension between them is real |
| 10 — Encoding corruption | P3 | L1, L17 | ✓ but *misinterpreted* | Lines exist as cited; cited characters are em-dash + smart quotes (valid UTF-8), not corruption |

Result: 9/10 findings grounded as claimed. Finding #10 partially refuted on factual grounds — the line citations are correct, but the interpretation of the cited characters as "corruption" is wrong. Underlying durability concern addressed in Corrections section below.

---

## Disposition of R1 findings

| # | Severity | Finding (one-line) | Disposition |
|---|---|---|---|
| 1 | P1 | Scope contradiction on project concurrency | **Fixed.** `What This Is Not` and `Scope Boundaries / Project-scoped` both updated to say single *active* project at a time with storage that may hold multiple. |
| 2 | P1 | Fix-loop termination deferred but load-bearing | **Fixed.** Locked in the Phase Review section: default is full-pool clean; configurable to single-clean per project. Removed from Open Items. |
| 3 | P1 | Single-Writer invariant inconsistent with non-coder durable records | **Fixed.** Tightened to "Coder authors human-facing project artifacts; others emit machine-readable records to audit/provenance store." Audit Trail paragraph cross-references this explicitly. |
| 4 | P1 | Quality claims without measurement | **Fixed.** "Dramatically higher" softened to "noticeably higher … measurable against Evaluation Criteria." New `Evaluation Criteria` section added with six metrics. Thresholds-per-metric added as Open Item. |
| 5 | P2 | Diversity rule under-specified for model supply chains | **Fixed.** Adversarial Diversity rewritten with operational tiers: enforced floor (different model family), preferred default (different vendor, ≥2 vendors collectively), pool size (≥2 distinct models). Family floor is non-relaxable; vendor preference is relaxable. |
| 6 | P2 | Human-gating risks ceremony without exception policy | **Partially accepted.** Gate granularity now explicit: stage boundaries and within-phase major beats are gates; internal mechanical operations are not separate gates (their output is reviewed at the next major beat). User's preference for human approval at every transition stands — what changed is the precise definition of "transition." |
| 7 | P2 | Living Plan needs condensation | **Fixed.** Plan Consolidation added as a paired invariant. Hardening Notes accumulate by design; consolidation absorbs them periodically (triggers added as Open Item). Provenance preserved via artifact graph. |
| 8 | P2 | Hinge Tests elevated to invariant before applicability proven | **Fixed.** Original Hinge Tests invariant replaced with broader "Deferred Decisions Are Tracked." Tracking is the invariant; hinge tests are the *preferred mechanism*; alternative mechanisms allowed where the stack doesn't support hinge tests cleanly. |
| 9 | P2 | Verifier authority boundary unclear | **Fixed.** Verifier scope now explicitly limited to evidence validation. Three result states: grounded, refuted, cannot-be-verified. Charter language: "Verification is about facts; disposition is about judgment." Both the Phase Review subsection and the Findings Are Grounded invariant carry the new wording. |
| 10 | P3 | "Encoding corruption" at line 1 and line 17 | **Partially refuted.** The cited characters are valid UTF-8 typography (em-dash U+2014, typographic quotes), not corruption. Charter intentionally uses UTF-8 typography to match the AiMe `IP/` convention. The reviewer's underlying durability concern is addressed by explicit commitment to UTF-8 in the Plan stage's file-system layout item. No Charter content change. See Corrections below. |

---

## Files changed since R1

| File | Action | Purpose | Approximate delta |
|---|---|---|---|
| `new_project_charter.md` | MODIFY | Apply 9 amendments (Findings 1–9). Soften L25/L269 quality language. Update L59 + L176 to resolve contradiction. Rewrite Phase Review verifier paragraph and termination paragraph. Replace Invariants section with revised invariants (Single-Writer, Adversarial Diversity, Human Gates, Findings Are Grounded, Living Plan, Plan Consolidation, Deferred Decisions Are Tracked, Provenance Graph, Reviewer Audit Trail). Insert new `Evaluation Criteria` section. Update Open Items (remove fix-loop termination; reframe hinge-test machinery as deferred-decision-tracking mechanisms; add Plan-consolidation triggers and Evaluation Criteria thresholds). Update Bottom Line. Append `Hardening Notes (R1 — Consolidated)`. | +~150 lines net (gains from Evaluation Criteria and Hardening Notes; losses negligible) |
| `REVIEW_CHARTER_R1.md` | CREATE | This document. | ~200 lines |

No other files touched. No code changes (Charter stage; no implementation surface yet).

---

## Corrections to R1 narrative

### P3 #10 — "encoding corruption" framing is factually wrong

R1 reads: *"The document has visible encoding corruption, which is small editorially but bad for a charter that emphasizes durable artifacts. Examples appear in new_project_charter.md:1, new_project_charter.md:17, and throughout the arrow/range punctuation."*

Verified by reading the file in UTF-8: line 1 reads `# Anvil — Project Charter` with U+2014 EM DASH; line 17 reads `The pitch is not "smarter codegen." The pitch is: **the workflow is the product.**` with typographic quotes U+201C / U+201D. These are valid UTF-8 codepoints, not corrupted bytes. The file opens correctly in any UTF-8-aware editor and renders correctly through Markdown processors.

The reviewer likely saw these characters rendered as escape sequences or as replacement characters in a non-UTF-8-aware tool. That is a reviewer-environment issue, not a Charter durability issue. The Charter's convention (typographic punctuation in UTF-8) matches the existing AiMe `IP/` plan documents, which have been the user's working format for many months without durability complaints.

**Disposition:** the corruption framing is rejected. The underlying durability concern (artifacts should be readable across decades) is granted and addressed by explicit Plan-stage commitment to UTF-8 as the file encoding standard. No Charter content edits triggered by this finding.

---

## Residual / deferred

- **Audit-store schema.** The Charter now distinguishes human-facing artifacts (Coder writes) from machine-readable records (other specialists may emit). The actual *schema* of the audit store — what files, what format, what queryability — is a Plan-stage decision.
- **Plan-consolidation triggers.** Plan Consolidation is now an invariant; the *signals* that fire consolidation (phase boundary, note-count threshold, time-based, manual) are deferred to the Plan stage as an Open Item.
- **Deferred-decision tracking mechanisms beyond hinge tests.** The invariant allows alternatives (flagged registry, deferral docs, calendar reminders); the *implementation* of those alternatives is a Plan-stage item.
- **Evaluation Criteria thresholds.** The six metrics are named in the Charter; the per-project numeric thresholds are set in the Plan stage and become Ship criteria.
- **Reviewer-vendor tagging convention.** R1 reviewer is pseudonymized as `R1-Reviewer-A` pending a tagging convention (vendor + model family + version) that the audit store can carry. Plan-stage item.

---

## Reproducibility

The Charter's current state can be checked with simple greps. The R1 amendments are present iff each command below returns ≥1 match:

```bash
# Scope contradiction resolved (Finding 1):
grep -n "single \*active\* project" new_project_charter.md
grep -n "single-active" new_project_charter.md

# Fix-loop termination locked (Finding 2):
grep -n "full-pool clean" new_project_charter.md
grep -n "configurable per project to \*single clean pass\*" new_project_charter.md

# Single-Writer tightened (Finding 3):
grep -n "human-facing project artifacts" new_project_charter.md
grep -n "machine-readable records" new_project_charter.md

# Evaluation Criteria added (Finding 4):
grep -n "## Evaluation Criteria" new_project_charter.md
grep -n "Defect escape rate" new_project_charter.md

# Adversarial Diversity operational tiers (Finding 5):
grep -n "Floor (enforced)" new_project_charter.md
grep -n "model family" new_project_charter.md

# Human-Gating granularity (Finding 6):
grep -n "Stage boundaries" new_project_charter.md
grep -n "Within-phase major beats" new_project_charter.md

# Plan Consolidation invariant (Finding 7):
grep -n "Plan Consolidation" new_project_charter.md

# Deferred Decisions Are Tracked (Finding 8):
grep -n "Deferred Decisions Are Tracked" new_project_charter.md
grep -n "preferred mechanism" new_project_charter.md

# Verifier scope sharpened (Finding 9):
grep -n "strictly evidence validation" new_project_charter.md
grep -n "Verification is about facts" new_project_charter.md

# Hardening Notes section appended:
grep -n "Hardening Notes (R1 — Consolidated)" new_project_charter.md

# Fix-loop termination removed from Open Items:
grep -n "Fix-loop termination" new_project_charter.md
# Expected: 0 matches in the Open Items section (still appears in Hardening Notes summary)
```

Reviewer for the next rotation cycle (R2 reviewer in pool terminology, though this is the first response document) should run these to confirm the live state matches what this disposition claims.

---

## Bottom line

R1 was a high-yield round: nine substantive findings, eight folded into the Charter, one factually refuted with the underlying concern still addressed. The two largest gains were promoting the fix-loop termination decision and the quality-measurement question out of Open Items — both were load-bearing enough that leaving them open would have created downstream drift.

The Charter is now structurally tighter (no contradictions), measurably grounded (Evaluation Criteria), and more honest about the trade-offs the workflow imposes (gate granularity, audit-store distinction, deferred-decision mechanisms broader than hinge tests). It is ready for the next reviewer in rotation.

Per the now-locked termination condition (full-pool clean default), Charter rotation continues until every reviewer in the pool has produced a clean pass on this state.
