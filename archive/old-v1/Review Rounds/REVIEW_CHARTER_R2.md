# Charter — R2 Disposition

**Date:** 2026-05-15
**Scope:** Response to R2 findings on the Anvil Project Charter (post-R1 state). R2 raised six findings across three categories (Technical & Logic, Consistency & Integrity, Structural). All six were grounded and accepted; one (R2 #1.2, grep robustness) is fixed *in this disposition document* rather than in the Charter itself, because it was a flaw in the R1 disposition's reproducibility section, not in the Charter.
**Spec:** `new_project_charter.md` (updated this round; `Hardening Notes (R2 — Consolidated)` appended)
**Prior rounds:** `REVIEW_CHARTER_R1.md` (R1 disposition; some sections superseded by this document — see *Corrections* below)
**R2 reviewer:** pseudonymized as `R2-Reviewer-B`. Per the locked Adversarial Diversity invariant, drawn from a different model family than both Coder (Claude) and R1 reviewer.

---

## What changed since R2

R2 was the second independent review of the Anvil Charter — this time against the post-R1 state. The review's character was different from R1's: fewer line-anchored citations, more conceptual / architectural concerns about how the R1-introduced invariants would behave in practice. That difference is exactly what cross-family adversarial diversity is supposed to surface — different reviewers find different *kinds* of problems.

The six findings produced three substantive additions to the Charter:

- **Cross-Reference Integrity** as a new invariant. R2 correctly identified that the R1-introduced Single-Writer / Audit Trail split could allow the two stores to drift silently. The new invariant requires every human-facing decision to be backed by a machine-readable record in the audit store, and vice versa. This is the gap closed.
- **Falsifiable Evaluation Criteria.** R1 added the metrics; R2 noted they had no direction-of-success or qualitative thresholds, which left them unfalsifiable. Each of the six metrics now carries explicit direction and qualitative success indicators. Numeric thresholds remain a Plan-stage item.
- **Artifact Encoding invariant** with explicit UTF-8 commitment. R2 reopened the encoding question raised by R1's #10 (and refuted in R1 disposition), but R2's framing was constructive: future tooling that's not UTF-8 aware would resurface the corruption claim, so codify UTF-8 explicitly. Done — and the Plan-stage lint check at the audit-store boundary makes this enforceable.

Two smaller but useful tightenings:

- The six gates from the R1-revised Human-Gating invariant are now explicitly declared **exhaustive**, not exemplary. Six gates per phase loop, period. Sub-beats within complex Build phases are a Plan-stage parameterization, not an emergent property.
- The audit-store schema is **promoted to highest-priority Plan-stage item**, with an explicit schema-content checklist. Without this schema, both *Findings Are Grounded* and *Cross-Reference Integrity* are unenforceable — that level of risk doesn't belong on a flat priority list.

---

## Verification of R2 claims

R2 had fewer line-citable claims than R1 — most findings were conceptual / architectural. Where R2 made verifiable claims about the Charter's pre-R2 state, those were checked:

| Finding | Verifiable claim | Verified? | Notes |
|---|---|---|---|
| 1.1 — "Major beats" fuzzy | The post-R1 Charter listed three within-phase beats without declaring the list exhaustive | ✓ | Pre-R2 line read "Within-phase major beats: Briefing → Reviewer, ..." with no exhaustiveness marker. Wording allowed the ambiguity R2 flagged. |
| 1.2 — Grep returns Hardening Notes hits | The R1 disposition's `grep -n "Fix-loop termination" new_project_charter.md` returns matches in the Hardening Notes section, not only the Open Items section | ✓ | Confirmed by direct grep: returned 2 lines in Hardening Notes (lines 318, 353 of the post-R1 Charter). The R1 disposition's parenthetical comment acknowledged this but did not fix it. |
| 1.3 — Metrics lack baselines | The post-R1 Evaluation Criteria had descriptions but inconsistent direction-of-success and no qualitative thresholds | ✓ | Only "Defect escape rate" had an explicit "Lower is better" tag; the other five metrics had descriptive text but no falsifiability frame. |
| 2.1 — UTF-8 deferred to Plan | R1 disposition addressed encoding-durability concern via "Plan-stage commitment to UTF-8" rather than Charter-level commitment | ✓ | R1 disposition section *Corrections to R1 narrative* ends with this deferral. No Charter-level encoding statement existed in the pre-R2 file. |
| 2.2 — Provenance Graph lacks cross-reference requirement | The post-R1 Provenance Graph invariant says "graph is queryable" but does not explicitly require human-facing decisions to be backed by machine-readable records | ✓ | Confirmed by reading line 182 of post-R1 Charter. No cross-reference requirement present. |
| 3.1 — Audit-store schema is highest-risk deferral | The post-R1 Open Items list contained "Artifact store implementation" as one bullet among many, not flagged as priority | ✓ | The bullet existed but carried no priority signal. |

Result: 6/6 verifiable claims grounded. The conceptual / architectural framing claims (e.g., "the risk is X") are not directly grep-verifiable but their underlying premises are. All findings accepted as grounded.

---

## Disposition of R2 findings

R2 reviewer did not assign explicit severity tiers. The Coder assigned severities below based on the structural / load-bearing weight of each finding:

| # | Severity (Coder-assigned) | Finding (one-line) | Disposition |
|---|---|---|---|
| 1.1 | P2 | "Major beats" enumeration could be read as exemplary, not exhaustive | **Fixed.** Six gates per phase loop declared **exhaustive** in the Human-Gating invariant. Sub-beats within complex Build phases are explicitly Plan-stage parameterization. |
| 1.2 | P3 | R1 disposition's reproducibility greps return Hardening Notes hits | **Fixed in this disposition.** Affected R1 disposition commands are superseded by the surgical commands in *Reproducibility* below. R1 disposition file itself is not edited (single-writer artifact discipline; corrections live in the superseding document). |
| 1.3 | P1 | Evaluation metrics lack direction-of-success / qualitative thresholds → temporarily unfalsifiable | **Fixed.** All six metrics now carry explicit direction-of-success and qualitative success indicators (e.g., "≥70% precision indicates healthy reviewers; sustained <50% indicates reviewer drift"). Numeric thresholds remain Plan-stage. |
| 2.1 | P2 | UTF-8 commitment deferred to Plan stage leaves a forward-looking durability gap | **Fixed.** New `Artifact Encoding` invariant inserted: all human-facing artifacts are UTF-8; non-ASCII typography is permitted and intentional. Plan-stage commitment to a lint check at the audit-store boundary that flags invalid byte sequences and surfaces non-ASCII characters for explicit acknowledgement. |
| 2.2 | P1 | Provenance Graph invariant doesn't require cross-reference between human-facing artifacts and machine-readable records → drift risk | **Fixed.** New `Cross-Reference Integrity` invariant added between Provenance Graph and Reviewer Audit Trail: every human-facing decision must be backed by a machine-readable record in the audit store; a decision in prose but absent from the audit store is a violation. Bidirectional check required for "shipped" status. |
| 3.1 | P2 (observation) | Audit-store schema is highest-risk deferral; lack of schema allows "dark data" to accumulate | **Fixed.** Open Items entry rewritten as the explicit highest-priority Plan-stage item, with a schema-content checklist: record types, cross-reference key, queryability, append-only enforcement, UTF-8 lint boundary. The Charter now flags this as the load-bearing Plan-stage decision. |

---

## Files changed since R2

| File | Action | Purpose | Approximate delta |
|---|---|---|---|
| `new_project_charter.md` | MODIFY | Apply 5 amendments (R2 findings 1.1, 1.3, 2.1, 2.2, 3.1). Tighten Human-Gating invariant with exhaustive-6-gates wording. Add direction-of-success + qualitative success indicators to all six Evaluation Criteria. Add new `Cross-Reference Integrity` invariant. Add new `Artifact Encoding` invariant. Rewrite the audit-store Open Items entry as highest-priority with content checklist. Append `Hardening Notes (R2 — Consolidated)` section. | +~50 lines net |
| `REVIEW_CHARTER_R2.md` | CREATE | This document. | ~200 lines |
| `REVIEW_CHARTER_R1.md` | (UNTOUCHED) | Per single-writer artifact discipline, prior disposition documents are not edited. Sections that need superseding are addressed in *Corrections* below. | 0 |

---

## Corrections to R1 disposition

Per the AiMe `REVIEW_*_R2.md` pattern, when a current-round review identifies a flaw in the prior-round disposition, the current disposition explicitly supersedes the affected sections. Prior disposition files are not edited.

### R1 disposition *Reproducibility* section — superseded

The R1 disposition included greps such as:

```bash
# Fix-loop termination removed from Open Items:
grep -n "Fix-loop termination" new_project_charter.md
# Expected: 0 matches in the Open Items section (still appears in Hardening Notes summary)
```

R2 correctly flagged that this is fragile — a reviewer running the bare grep gets matches and may misread the result without reading the parenthetical comment. The grep is functionally correct but operationally weak.

**Superseding behavior:** Use section-isolated checks instead of global greps. The *Reproducibility* section below provides the surgical replacements. The R1 disposition's commands remain in place as historical record but should not be relied upon for verification of post-R2 state.

---

## Residual / deferred

- **Audit-store schema specification.** Now flagged as highest-priority Plan-stage item, but still deferred. Until the schema lands, the new *Cross-Reference Integrity* invariant is *aspirational* — it can be checked manually but not enforced by tooling. This is the largest remaining risk.
- **Sub-beat definitions for complex Build phases.** The Charter declares the six gates exhaustive at the framework level, and notes that sub-beats are a Plan-stage parameterization. Specific projects (those with multi-deliverable Build phases) will need to enumerate their own sub-beats in their Plan documents.
- **Numeric thresholds for Evaluation Criteria.** Direction and qualitative success are in the Charter; numbers are in the Plan.
- **UTF-8 lint mechanism.** Charter commits to a lint check at the audit-store boundary; the actual implementation (when does it run, how does it surface non-ASCII characters for acknowledgement, what is the bypass workflow) is Plan-stage.
- **Cross-Reference Integrity enforcement mechanism.** The invariant is stated; the *runtime check* (when is bidirectional consistency verified, what artifact metadata enables the cross-reference) needs to be specified alongside the audit-store schema.

---

## Reproducibility

Surgical section-isolated checks, replacing the R1 disposition's fragile global greps. Each command isolates a specific Charter section before checking, so Hardening Notes content does not contaminate verification of the canonical body.

```bash
# Helper: extract a section between two ## or higher-level headers.
# Use awk to print lines from <SECTION_HEADER> until the next header line.

# R2 #1.1 — "Major beats" is exhaustive (Invariants section)
awk '/^## Invariants/,/^## /' new_project_charter.md | grep -E "exhaustive|Six gates total"
# Expected: at least one match for each pattern

# R2 #1.3 — Each Evaluation Criterion has direction-of-success
awk '/^## Evaluation Criteria/,/^---$/' new_project_charter.md | grep -c "Direction:"
# Expected: 6 (one per metric)

# R2 #2.1 — Artifact Encoding invariant exists (Invariants section)
awk '/^## Invariants/,/^## /' new_project_charter.md | grep "Artifact Encoding"
# Expected: 1 match

# R2 #2.2 — Cross-Reference Integrity invariant exists (Invariants section)
awk '/^## Invariants/,/^## /' new_project_charter.md | grep "Cross-Reference Integrity"
# Expected: 1 match

# R2 #3.1 — Audit-store schema entry is flagged highest-priority (Open Items section)
awk '/^## Open Items/,/^## /' new_project_charter.md | grep "Highest-priority Plan-stage item"
# Expected: 1 match

# Fix-loop termination still absent from Open Items (post-R1, re-verify post-R2)
awk '/^## Open Items/,/^## /' new_project_charter.md | grep -c "Fix-loop termination"
# Expected: 0 (zero hits in Open Items; matches in Hardening Notes are excluded by section isolation)

# Hardening Notes (R2 — Consolidated) appended
grep -n "## Hardening Notes (R2 — Consolidated)" new_project_charter.md
# Expected: 1 match, after the R1 hardening section
```

These commands use `awk` range expressions to isolate one section at a time, so a match in `Hardening Notes` cannot contaminate a check for absence in `Open Items`. The R1 disposition's commands remain valid for the global presence of strings but should not be used for absence-in-section checks.

---

## Bottom line

R2 was complementary to R1, not redundant. R1 found contradictions and unmeasured claims at the structural-language level; R2 found integrity gaps and falsifiability gaps at the architectural-behavior level. That difference is the cross-family diversity invariant doing exactly what it was designed to do — different reviewers find different *kinds* of problems, and converging through them produces a tighter artifact than any single reviewer's pass could.

The Charter now has:

- All R1 amendments (9 substantive + 1 correction) folded in
- All R2 amendments (5 substantive + 1 disposition-level correction) folded in
- Two new invariants (*Cross-Reference Integrity*, *Artifact Encoding*) that close gaps the R1 changes exposed
- Falsifiable Evaluation Criteria with direction-of-success and qualitative thresholds
- An exhaustive (not exemplary) six-gate enumeration
- The audit-store schema flagged as the single highest-risk Plan-stage deferral

Per the locked termination condition (full-pool clean default), the next rotation step is **R1 reviewer (Codex-class) producing a clean confirmation pass on the post-R2 state**. The Charter does not ship until both reviewers' most recent passes come back clean on the same state. If R1 reviewer's confirmation pass surfaces additional findings, those become R3 input and the loop continues; if it returns clean, R2 reviewer (Gemini-class) gets one more pass to confirm no R3-induced changes broke their R2 sign-off, and only when both are clean on the *same* state does the Charter ship.
