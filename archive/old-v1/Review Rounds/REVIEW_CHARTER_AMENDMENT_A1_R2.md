# Charter Amendment A1 — R2 Disposition

**Date:** 2026-05-19
**Scope:** Response to R2 findings on Charter Amendment A1 Draft 2. R2 raised twelve findings (4 Fix First, 4 Important, 4 Medium). All twelve Fixed in Draft 3 per the reviewer's explicit guidance to produce a consistency-cleanup-focused draft rather than reopen the architectural surface.
**Spec:** `CHARTER_AMENDMENT_A1.md` (updated to Draft 3 this round; hardening notes in `AMENDMENT_A1_HARDENING_HISTORY.md`). `ARTIFACT_SPECIFICATIONS.md` was touched mid-flight to resolve cross-document inconsistency (Fix First #2); this was the right move per the new *Cross-Document Convergence* rules added in Draft 3.
**Prior round:** `REVIEW_CHARTER_AMENDMENT_A1_R1.md`.
**R2 reviewer:** second reviewer family per Charter rotation; different model family from Coder per Adversarial Diversity floor.

**Shell assumption for verification commands:** POSIX shell utilities (`grep`, `awk`).

---

## What changed since Draft 2

R2 was a high-quality consistency-cleanup review. The reviewer correctly identified that Draft 2 had closed the architectural gaps but introduced or retained internal inconsistencies across sections. The reviewer's explicit guidance — "produce Draft 3 focused only on consistency cleanup" — set the scope discipline.

The four Fix First items closed the most consequential issues:

- **Trademark consistency restored** — the Posture A lock from R1 is now reflected in Reviewer Questions and Acceptance Criteria; previously these sections still said the decision was pending.
- **Artifact-spec governance aligned** — `ARTIFACT_SPECIFICATIONS.md` now requires full Charter amendment for major spec changes (matching the amendment's claim); the previous "notification only" language was a cross-document contradiction.
- **Publication-safe git history gate** — a new constitutional gate runs before the repo flips public: full-history secret scan, full-history license scan, commit-message sensitivity review, and an exceptional history-rewrite allowance. This closes a real safety gap: Draft 2's "git history preserved verbatim" claim was outside the protected audit-export path.
- **All audit records private by default** — record types carry only a non-binding `recommended_visibility` metadata field; no automatic public publication. The brittle "quoted strings ≥40 chars" redaction heuristic from Draft 2 is replaced (also closes Important #6) by structured sensitivity labels + secret/license scans + Coordinator manual review.

The four Important items closed structural gaps: Plan Draft 7 must reconcile record-type count from 13 to 16 (Draft 3 adds `EmergencyFreezeDeclaration` for Medium #9, on top of Draft 2's `PublicVisibilityPolicy` and `PublicExportApproval`); cross-document conflict resolution defined; constitutional outcomes separated from implementation tools (CycloneDX/cargo-audit/govulncheck/gitleaks/GPG framed as Plan-stage defaults).

The four Medium items addressed real operational concerns: BDFL-adversarial emergency-freeze mechanism (with new audit record type); DCO substantive/persistent definitions; mandatory `--describe-schema` CLI flag; v1.2 embedding transport form loosened from specific to principles-only.

One side effect worth noting: `ARTIFACT_SPECIFICATIONS.md` was edited mid-flight to resolve Fix First #2's cross-document inconsistency. This is the first application of the newly-added *Cross-Document Convergence* rules — a finding identified inconsistency between two documents in the same review pool's scope, the fix touched both documents, and the round's disposition documents the cross-document change.

---

## Verification of R2 finding citations

All twelve R2 citations verified against Draft 2 (and `ARTIFACT_SPECIFICATIONS.md` for Fix First #2). Each premise grounded.

| Finding | Citation(s) | Verified? | Premise |
|---|---|---|---|
| Fix First #1 Trademark contradiction | A1:71 vs A1:328 + A1:344 | ✓ | Trademark Posture A locked at L71; Reviewer Questions and Acceptance still said pending |
| Fix First #2 Spec governance | A1:178 + A1:193 vs SPEC:260 + SPEC:274 | ✓ | A1 said "full Charter amendment for major"; spec said "notification only" |
| Fix First #3 Audit/publication tension | A1:57 (history verbatim) vs A1:146 (default-deny export) | ✓ | Git history outside the protected export path; commit messages/diffs/secrets unprotected |
| Fix First #4 Public-safe defaults | A1:146 (ConvergenceDeclaration default public-safe) | ✓ | Record-type defaults too aggressive; convergence reasoning can include sensitive content |
| Important #5 Record-type counts | A1:159 (15 types) vs prior Plan acceptance | ✓ | Plan currently says 13; needs Draft 7 reconciliation |
| Important #6 Redaction heuristics | A1:148 (quoted strings ≥40 chars) | ✓ | Brittle; may miss unquoted secrets and remove legitimate evidence |
| Important #7 Spec convergence definition | A1:343 (sibling spec convergence required, mechanism undefined) | ✓ | No cross-document conflict resolution rules |
| Important #8 Constitutional vs implementation | A1:289 (gitleaks, CycloneDX, etc. named in Required Choices) | ✓ | Tool-specific names become constitutional baggage |
| Medium #9 BDFL-adversarial | A1:87 covers unavailability not compromise | ✓ | No emergency mechanism for hostile/compromised BDFL |
| Medium #10 DCO enforceability | A1:106 ("substantive" and "persistently" undefined) | ✓ | Inconsistent application risk in CI |
| Medium #11 Schema discovery | A1:331 (raised as question; should be requirement) | ✓ | Subprocess embedders cannot rely on filesystem layout |
| Medium #12 v1.2 transport lock-in | A1:223 (network service + typed clients pre-specified) | ✓ | May over-specify v1.2 before design |

Result: 12/12 grounded.

---

## Disposition of R2 findings

| # | Severity (R2 label) | Finding | Disposition |
|---|---|---|---|
| Fix First 1 | P1 equivalent | Trademark contradiction across sections | **Fixed.** Reviewer Question #4 and Acceptance Criterion #5 both updated to reflect Posture A locked with re-evaluation trigger. |
| Fix First 2 | P1 equivalent | Spec governance misaligned with amendment | **Fixed.** `ARTIFACT_SPECIFICATIONS.md` versioning policy updated: MAJOR changes require full Charter amendment; MINOR/PATCH use narrower spec-amendment process; Selection Rule defaults ambiguity to MAJOR. Cross-document fix; documented in *Files changed*. |
| Fix First 3 | P1 equivalent | Audit/publication tension on git history | **Fixed.** New constitutional Publication-Safe Git History Gate: full-history secret scan + license scan + commit-message sensitivity review + exceptional history-rewrite allowance. Gate runs before public flip; on critical path for v1 ship. |
| Fix First 4 | P1 equivalent | Public-safe defaults too aggressive | **Fixed.** All records private by default; `recommended_visibility` metadata is non-binding guidance only. Per-record explicit opt-in required. Multi-pass scan: secret scan, license scan, structured sensitivity labels, mandatory Coordinator manual review. (Also closes Important #6.) |
| Important 5 | P2 equivalent | Record-type count Plan reconciliation | **Fixed.** Amendment explicitly states Plan Draft 7 must update record-type total from 13 to 16 (after Draft 3 adds `EmergencyFreezeDeclaration` for Medium #9). Constitutional hinge `test_audit_store_required_types_present` is unchanged (subset check). |
| Important 6 | P2 equivalent | Brittle redaction heuristics | **Fixed via Fix First #4** above. Multi-pass scan replaces quoted-strings heuristic; structured sensitivity labels + multiple specialized scanners + mandatory human approval. |
| Important 7 | P2 equivalent | "Artifact specifications converged" undefined | **Fixed.** New *Cross-Document Convergence* subsection: same reviewer pool reviews both documents; cross-document findings labeled and addressed in both; Item 2 cannot converge without both documents clean; conflict-resolution rule (constitutional → amendment, operational → spec). |
| Important 8 | P2 equivalent | Tool-specific names in Required Choices | **Fixed.** Choices now lock *outcomes* (SBOM produced, dep-scan at every CI run, signing offline-verifiable, secret-scan pre-commit + CI). Specific tools (CycloneDX, cargo audit, govulncheck, GPG signing, gitleaks) named as Plan-stage defaults that the Plan may revise without amendment. |
| Medium 9 | P3 equivalent | BDFL-adversarial gap | **Fixed.** New emergency-freeze provision: two-thirds maintainer majority may halt merges/releases/governance actions on credential compromise or hostile-action concern. Freeze is narrow (pauses activity; doesn't transfer authority). New audit-record type `EmergencyFreezeDeclaration` (16th record type). |
| Medium 10 | P3 equivalent | DCO enforceability definitions missing | **Fixed.** "Substantive code paths" defined: ≥20 lines new code OR new file OR code in core crates; doc/test/build excluded. "Persistently" defined: warning on first submission; blocking on subsequent updates without trailers. Thresholds operational; revisable in Plan without amendment. |
| Medium 11 | P3 equivalent | Schema discovery should be requirement | **Fixed.** `--describe-schema` is now mandatory on every command emitting structured output. Schemas embedded into binary at build; subprocess embedders discover schemas through same channel as output. |
| Medium 12 | P3 equivalent | v1.2 transport lock-in premature | **Fixed.** Amendment now locks only the *principles* (Vault authority, no gate bypass, typed API, audit records mandatory, per-embedder auth, multi-tenant isolation). Specific transport form (network service, in-process, IPC, FFI, hybrid) is v1.2 design choice with its own Charter amendment cycle. |

---

## Files changed since Draft 2

| File | Action | Purpose | Approximate delta |
|---|---|---|---|
| `CHARTER_AMENDMENT_A1.md` | MODIFY | Status bumped to Draft 3. Apply 12 R2 fixes. Update Reviewer Questions (close trademark; refresh others). Update Acceptance Criteria (close trademark). Rewrite Public-safe audit bundle section with all-records-private-by-default + multi-pass scan. Add Publication-Safe Git History Gate to Publication Milestone. Update Required Choices table: separate constitutional outcomes from Plan-default tools. Extend Governance Mechanics with emergency-freeze. Define DCO substantive/persistently thresholds. Promote schema discovery to requirement. Loosen v1.2 transport from specific to principles-only. Add Cross-Document Convergence subsection. Update record-type total to 16 with Plan-Draft-7 reconciliation note. | +~250 lines net |
| `ARTIFACT_SPECIFICATIONS.md` | MODIFY | Spec versioning policy aligned with amendment: MAJOR changes require full Charter amendment; Selection Rule defaults ambiguity to MAJOR. Spec amendment process rewritten with two paths (major vs minor/patch). | +~25 lines net |
| `AMENDMENT_A1_HARDENING_HISTORY.md` | MODIFY | Append `Hardening Notes (R2 — Consolidated)` covering all 12 findings. | +~110 lines |
| `REVIEW_CHARTER_AMENDMENT_A1_R2.md` | CREATE | This document. | ~250 lines |
| `REVIEW_CHARTER_AMENDMENT_A1_R1.md` | (UNTOUCHED) | Per single-writer artifact discipline. |

---

## Corrections to prior dispositions

None this round. R1 disposition language stands.

---

## Residual / deferred

- **`ARTIFACT_SPECIFICATIONS.md` own R1 review.** The spec doc is in its own concurrent R1 review. Draft 3's cross-document fix updates the spec; the spec's R1 reviewer will see the post-edit state. Per the new *Cross-Document Convergence* rules, this is the right pattern.
- **Plan Draft 7 production.** Required by this amendment after convergence. Will reconcile record-type counts (13 → 16), incorporate the impact matrix, and integrate the publication-safe-history gate into P11 acceptance. Plan Draft 7 is a separate workstream.
- **`schemas/cli/` per-command schema files.** Required by Structured CLI Output Stability; lands in Plan phases P5–P10a as commands are implemented.
- **`anvil audit export --public` implementation.** Committed in this amendment; lands in Plan P2's audit-store work.

---

## Reproducibility

```bash
# --- Fix First 1 — Trademark consistency restored ---
grep -E "Posture A locked|locked: Posture A" CHARTER_AMENDMENT_A1.md
# Expected: ≥3 matches (Required Choices table, Trademark Posture section, Reviewer Questions).

awk '/^## Acceptance Criteria/,/^---$/' CHARTER_AMENDMENT_A1.md | grep -E "Posture A locked|Closed 2026-05-19"
# Expected: 1 match (item #5).

# --- Fix First 2 — Spec governance aligned ---
awk '/^### Semver Discipline/,/^### Process/' ARTIFACT_SPECIFICATIONS.md | grep -E "Requires full Charter amendment"
# Expected: 1 match (MAJOR row).

# --- Fix First 3 — Publication-Safe Git History Gate ---
grep -n "^**Publication-safe git history gate" CHARTER_AMENDMENT_A1.md
# Expected: 1 match in Publication Milestone section.

awk '/^### Publication Milestone/,/^### /' CHARTER_AMENDMENT_A1.md | grep -E "Secret scan over full history|License scan over full history|Commit-message review pass|Exceptional history rewrite allowance"
# Expected: 4 matches.

# --- Fix First 4 — All records private by default ---
awk '/^### Public-safe audit bundle/,/^---$/' CHARTER_AMENDMENT_A1.md | grep -E "All records private by default|recommended_visibility|never grants automatic public status"
# Expected: ≥2 matches.

# --- Important 7 — Cross-Document Convergence ---
grep -n "^### Cross-Document Convergence" CHARTER_AMENDMENT_A1.md
# Expected: 1 match.

# --- Important 8 — Constitutional outcomes vs Plan-default tools ---
grep -E "Outcome \(constitutional\)|Default tools \(Plan-stage" CHARTER_AMENDMENT_A1.md
# Expected: ≥3 matches in Required Choices.

# --- Medium 9 — BDFL-adversarial emergency mechanism ---
grep -n "BDFL-adversarial emergency" CHARTER_AMENDMENT_A1.md
# Expected: 1 match in Governance Mechanics.

grep "EmergencyFreezeDeclaration" CHARTER_AMENDMENT_A1.md
# Expected: ≥1 match.

# --- Medium 10 — DCO definitions ---
grep -E "Substantive code path|≥20 lines new code" CHARTER_AMENDMENT_A1.md
# Expected: ≥2 matches.

# --- Medium 11 — Schema discovery requirement ---
grep -E "Schema discovery via CLI|--describe-schema.*mandatory|Schema discovery.*mandatory" CHARTER_AMENDMENT_A1.md
# Expected: ≥1 match.

# --- Medium 12 — v1.2 transport provisional ---
grep -E "Embeddable transport form \(intentionally provisional|principles only|specific transport form is a v1.2 design choice" CHARTER_AMENDMENT_A1.md
# Expected: ≥1 match.

# --- Record-type total updated to 16 ---
grep -E "Total v1 audit-store record types.*16|16 \(11 Charter-required" CHARTER_AMENDMENT_A1.md
# Expected: ≥1 match.

# --- R2 hardening notes appended ---
grep -n "^## Hardening Notes (R2 — Consolidated)" AMENDMENT_A1_HARDENING_HISTORY.md
# Expected: 1 match.
```

---

## Bottom line — convergence candidate

R2 closed Draft 2's internal inconsistencies. The reviewer's explicit guidance ("consistency cleanup, not architectural reopening") set the right scope; Draft 3 stays inside that scope while addressing all twelve findings.

The trajectory across the amendment's two rounds:

- R1: 15 findings (5 P1, 5 P2, 5 P3) — substantial structural additions (Contract Inventory, Public/Private Audit Records, Security Posture as constitutional, Governance Mechanics, spec amendment path tightened, plus seven more).
- R2: 12 findings (4 Fix First, 4 Important, 4 Medium) — consistency cleanup + targeted refinements; explicit reviewer guidance that the architecture is settled and only internal alignment needs work.

This matches the convergence trajectory you observed on the Charter (R1 → R4) and the Plan (R1 → R5): findings shift from "missing structure" to "consistency and refinement," and reviewer guidance shifts from "fix these gaps" to "tighten these alignments."

**Two paths from here, same as prior convergence calls:**

1. **R3 delta review.** Per the R2 reviewer's explicit suggestion, send Draft 3 to the next reviewer for a *short delta review* focused on whether the R2 fixes landed cleanly. Likely surface area: 2–4 findings, mostly P3.
2. **Convergence call.** Given the R2 reviewer's positive signal on Draft 2's substance + Draft 3 closes all twelve R2 findings + the trajectory matches the prior convergence patterns, invoking human-arbiter convergence on the amendment is defensible. The post-Draft-3 state is unlikely to surface new architectural surface area.

If you converge, the next steps are: apply the amendment's three new invariants and 13+ new Required Choices into the Charter body; the amendment document remains as a historical artifact; CHARTER_HARDENING_HISTORY receives an "Amendment A1 — Applied" entry summarizing the convergence; Plan Draft 7 begins to integrate the amendment's downstream impact (record-type count update, impact matrix, publication-safe-history gate in P11, etc.).

The Artifact Specifications spec's own R1 review remains a separate workstream — its convergence is required for Item 2's full ship, per the new Cross-Document Convergence rules.
