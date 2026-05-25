# Plan — R3 Disposition

**Date:** 2026-05-19
**Scope:** Response to R3 findings on the Anvil Implementation Plan (post-R2 state, Draft 6). R3 raised fourteen findings: 5 × P1, 5 × P2, 4 × P3. Thirteen Fixed; one Deferred (P3 Finding 4, positioning section length).
**Spec:** `ANVIL_PLAN.md` (updated this round; hardening notes appended to `PLAN_HARDENING_HISTORY.md`).
**Prior rounds:** `REVIEW_PLAN_R1.md`, `REVIEW_PLAN_R2.md`.
**R3 reviewer:** third rotation slot from the configured pool; different model family from Coder per Adversarial Diversity floor.

**Shell assumption for verification commands:** POSIX shell utilities (`grep`, `awk`). Equivalent checks possible with any tool that supports section-isolated text search.

---

## What changed since R2

R3 was the most structurally substantive Plan round to date. The reviewer's central thesis was that the Plan had accumulated *internal contradictions* between sections that no longer agreed with each other after multiple drafts — phase count drifted, provider routing was incoherent across three locations, streaming invariant said three different things, hinge framework was claimed before it existed, P11 was an unbounded ship gate. None of these were architectural mistakes; they were each cases of language locking in one place while the mechanism that would enforce it was still loose elsewhere.

The five P1 findings all got Fixed treatment. The five P2 findings all got Fixed treatment. Three of four P3 findings got Fixed treatment; one (positioning section length) is Deferred as a structural pass rather than a correctness issue.

The two highest-value resolutions in this round:

- **Provider routing made coherent across P1, P3a, and P3c.** `InvokeRequest` now carries both `provider_connection_id` (non-secret routing field, populated by Vault) and `Credentials` (per-call secret material). The opacity-to-Vault claim narrowed from "all provider metadata" to "the connection's internal configuration (endpoint, region, provider-specific routing held in `--provider-config`)." The secret-flow language clarified that env vars are a *CLI-layer* injection mechanism, not a sidecar-startup mechanism — the sidecar never reads env vars for credentials.
- **Streaming invariant boundary made precise.** The earlier wording allowed three readings; the new wording explicitly distinguishes ephemeral display (`Token` events shown live in the terminal) from authoritative commit (`FinalResult` only). Mid-stream error stops forwarding tokens to the display sink, discards stream state from the commit path, and surfaces only the typed error. Tokens already shown remain visible — the terminal cannot un-print — but audit and artifact state are unaffected. P3a contract docs, P3b client docs, P3c sidecar acceptance criteria, and Plan-Level Trust-Boundary Invariant #1 all now describe the same boundary.

---

## Verification of R3 finding citations

Each line citation was checked against the current Plan state before any edits were applied.

| Finding | Citation(s) | Verified? | Notes |
|---|---|---|---|
| 1 — Phase count inconsistency | L292, L950 | ✓ | Both said "14 phases" against actual 15. Confirmed. |
| 2 — Provider routing & secrets | L332, L397, L465, L75, L470 | ✓ | Three locations disagreed about routing field and secret-injection mechanism. Confirmed real contradiction in routing; secrets language was technically consistent but ambiguously worded. |
| 3 — Streaming partial-output | L399, L435, L478 | ✓ | Three statements technically incompatible if read strictly. Confirmed. |
| 4 — Hinge / eval timing | L373, L705 | ✓ | Hinges named throughout P0–P9; framework arrives at P10b. Confirmed timing gap. |
| 5 — P11 ship gate | L735–L749 | ✓ | External pilot required without rubric / timebox / scope ceiling. Confirmed unboundedness risk. |
| 6 — Hinge totals | L857, L872 | ✓ | 46 vs 44 contradiction. Confirmed. |
| 7 — Audit-store integrity | L357, L819 | ✓ | Deletion-detection mechanism described without threat-model framing. Confirmed. |
| 8 — Brittle pin tests | named: `test_error_class_count`, `test_wizard_step_count`, adapter counts | ✓ | Exact-count pins on growable values. Confirmed. |
| 9 — Distribution acceptance | L920, L960 | ✓ | "Open" + "primary platform" without platform/install/signing/checksum specifics. Confirmed. |
| 10 — `anvil-graph` naming | L604 | ✓ | Ambiguous between crate name and CLI subcommand. Confirmed. |
| 11 — Non-interactive behavior | L521 | ✓ | Only API-key env-var bypass documented; other gates undocumented for headless. Confirmed. |
| 12 — Cost controls | (absence) | ✓ | No mention of budget caps, token accounting, spend limits anywhere. Confirmed by absence. |
| 13 — Single-active-project | L789 | ✓ | "Outside v1" while describing actual behavior. Confirmed ambiguity. |
| 14 — Positioning section | L82–L130 | ✓ | Long positioning section ahead of normative content. Structural suggestion, not correctness issue. |

Result: 14/14 finding premises grounded. All Fixed except Finding 14, Deferred to a later structural pass.

---

## Disposition of R3 findings

Disposition labels per the *Disposition Labels* vocabulary established in `ARTIFACT_SPECIFICATIONS.md` and used from R3 forward (Fixed / Locked in Charter, enforcement pending Plan / Refuted / Deferred).

| # | Severity | Finding (one-line) | Disposition |
|---|---|---|---|
| 1 | P1 | Phase count says 14, actual is 15 | **Fixed.** Both prose references updated to 15 with enumeration; phase-count-audit parenthetical added at Phase Decomposition header. |
| 2 | P1 | Provider routing & secret flow contradictions | **Fixed.** `InvokeRequest` proto now carries `provider_connection_id` + `Credentials`. Opacity narrowed to connection's internal configuration. Secret-flow language clarified: env vars are CLI-layer injection, sidecar never reads them at startup. Plan-Level Trust-Boundary Invariant #2 updated to make the routing-vs-secret distinction explicit. |
| 3 | P1 | Streaming partial-output invariant says three different things | **Fixed.** Boundary distinguishes ephemeral display (`Token` events live) from authoritative commit (`FinalResult` only). Mid-stream error discards stream state from commit path; tokens already displayed remain visible. Updated in P3a (contract), P3b (client), P3c (sidecar acceptance #5), and Plan-Level Trust-Boundary Invariant #1. All four now describe the same boundary. |
| 4 | P1 | Hinge / evaluation infrastructure named before built | **Fixed.** Hinge tests are ordinary unit tests with structured comment annotations (`// hinge_test: pins=<value>, intended=<value>, phase=<P-id>`) from P0 onward; P10b's framework auto-discovers and registers them. P0 action list and P2 acceptance criterion #8 updated to make this explicit. |
| 5 | P1 | P11 external pilot is unbounded ship gate | **Fixed.** Pilot selection rubric added to P11 action list: scope ceiling (3–7 phases), timebox (14 days; partial completion acceptable evidence), external user (project from someone other than Coordinator), domain unrelated to workflow tools / AI coding / developer productivity, failure-class triage (pilot-blocking vs pilot-informing). Pilot artifacts preserved in `docs/examples/external-pilot/`. |
| 6 | P2 | Hinge totals contradict (46 vs 44) | **Fixed.** Both prose counts removed. Registry table is canonical; count derived via `anvil hinge list --count`. Pin convention added distinguishing constitutional (exact-equality) from operational (minimum-equality) pins. |
| 7 | P2 | Audit-store deletion detection lacks threat model | **Fixed.** Acceptance criterion #7 in P2 now explicitly names the threat model — *local tamper detection, not adversarial tamper-proofing*. Catches accidental deletion, partial restore, filesystem corruption; does not defend against coordinated index-plus-file modification. Cryptographic tamper-proofing surfaced in Open Items as v1.x consideration. |
| 8 | P2 | Brittle pin tests on growable values | **Fixed.** Pin convention paragraph added to Deferred-Decision Registry. Constitutional pins (Charter-tied) remain exact-equality; operational pins (growable values) become minimum-equality. P10b's hinge proc-macro accepts `style: "exact" \| "minimum"` attribute enforcing the convention. |
| 9 | P2 | Distribution acceptance vague | **Fixed.** Open Items / Distribution rewritten with concrete v1 release acceptance: target platforms (Windows primary, macOS + Linux stretch), install method (per-platform release archive), signing (GPG-signed SHA-256 checksums), artifact layout, scripted smoke tests. Acceptance criterion #11 updated to reference. |
| 10 | P2 | `anvil-graph` crate-vs-CLI ambiguity | **Fixed.** Prose updated to state `anvil-graph` is the Rust crate (library); the CLI surface is `anvil graph <verb>` per verb-resource pattern. |
| 11 | P3 | Non-interactive behavior under-specified | **Fixed.** New Cross-Cutting Concern entry *Headless / non-interactive operation* documents `--headless` on wizard, `--yes` + `--reason` on approval gates, `--dry-run`, structured `--format json`, per-class exit codes, audit-record approval-source labels. |
| 12 | P3 | Cost controls absent | **Fixed.** New Cross-Cutting Concern entry *Model / provider cost controls*. Token counts and per-call cost in `RotationLog`; aggregation in P10a; optional `cost_limits` block in `anvil.toml`; warn-only default with explicit opt-in to hard-stop. |
| 13 | P3 | Single-active-project semantics unclear | **Fixed.** Sidecar lifecycle Cross-Cutting Concern clarified: single-active *by design*, multi-workspace *supported-but-uncoordinated* with visible CLI warning. Same-workspace concurrent access remains hard-blocked. |
| 14 | P3 | Positioning section length | **Deferred.** Structural suggestion (move to rationale appendix) reasonable but invasive. Logged as future Plan Consolidation candidate. No content change this round. |

---

## Files changed since R2

| File | Action | Purpose | Approximate delta |
|---|---|---|---|
| `ANVIL_PLAN.md` | MODIFY | Apply 13 Fixed dispositions across 14 distinct locations. Update phase count (Phase Decomposition header + Plan-Level Acceptance Criteria #1). Update Plan-Level Trust-Boundary Invariant #1 (streaming boundary) and #2 (routing vs secret distinction). Update P3a `InvokeRequest` envelope (add `provider_connection_id`, `Credentials`). Update P3b streaming partial-output rule. Update P3c acceptance #5 (streaming) + provider-access prose + configuration line. Update P0 action list (hinge-test convention details). Update P2 acceptance criteria (#7 threat model named; #8 metric-instrumentation hooks). Update P7 prose (`anvil-graph` clarification). Update P11 action list (external-pilot rubric). Update Deferred-Decision Registry (pin convention; canonical-list framing). Update Evaluation Metric Targets (deferred-decision row). Update Open Items (Distribution rewrite; new Auto-update line context). Add Cross-Cutting Concern entries (headless/non-interactive, cost controls). Update Cross-Cutting Concern (sidecar lifecycle / multi-workspace). | +~250 lines net |
| `PLAN_HARDENING_HISTORY.md` | MODIFY | Append `Hardening Notes (R3 — Consolidated)` section covering 14 findings with disposition summaries. | +~180 lines |
| `REVIEW_PLAN_R3.md` | CREATE | This document. | ~250 lines |
| `REVIEW_PLAN_R1.md`, `REVIEW_PLAN_R2.md` | (UNTOUCHED) | Per single-writer artifact discipline. |

---

## Corrections to prior dispositions

None this round. R3 did not flag any R1 or R2 disposition language as wrong; the prior rounds' terminology stands.

R3 itself uses the tightened disposition vocabulary (Fixed / Locked in Charter, enforcement pending Plan / Refuted / Deferred) established in Charter R3 and inherited via `ARTIFACT_SPECIFICATIONS.md`.

---

## Residual / deferred

- **Positioning section restructure (Finding 14).** Suggested move to rationale appendix is reasonable but structural; logged for a future Plan Consolidation pass. No content change this round.
- **Cryptographic tamper-proofing of audit store.** Mentioned in Open Items as v1.x consideration. The current implementation (index-vs-disk check) handles local tamper detection; chained record hashes / signed manifests are not in v1 scope.
- **Hard-stop cost limits.** v1 ships with warn-only default; users must opt into `cost_limits.enforce = true` to get hard-stop behavior. Policy evolution deferred to v1.x based on P11 + pilot data.
- **Global sidecar sharing across workspaces.** Multi-workspace operation is supported-but-uncoordinated in v1; coordinated multi-workspace daemons are post-v1.
- **R3 finding line citations now stale.** The line numbers in R3's findings refer to the *pre-R3* Plan state. After this disposition's edits, those lines have shifted by the deltas above. Future reviewers reading R3 should consult the Plan's post-R3 state via the verification commands below, not by re-grepping the original line numbers.

---

## Reproducibility

**Shell assumption:** POSIX shell utilities (`grep`, `awk`).

```bash
# --- R3 #1 — Phase count reconciled ---
grep -n "Fifteen phases" ANVIL_PLAN.md
# Expected: 1 match in Phase Decomposition header.

grep -n "All 15 phases" ANVIL_PLAN.md
# Expected: 1 match in Plan-Level Acceptance Criteria.

# --- R3 #2 — Routing field present in proto ---
awk '/^### \*\*P3a/,/^### \*\*P3b/' ANVIL_PLAN.md | grep -E "provider_connection_id|Credentials"
# Expected: ≥2 matches in P3a section.

# --- R3 #3 — Streaming boundary precise across three locations ---
awk '/Plan-Level Trust-Boundary Invariants/,/^---$/' ANVIL_PLAN.md | grep -E "Ephemeral display|Authoritative commit"
# Expected: ≥2 matches in invariants section.

# --- R3 #4 — Hinge convention from P0 ---
awk '/^### \*\*P0/,/^### \*\*P1/' ANVIL_PLAN.md | grep "structured-comment convention"
# Expected: ≥1 match in P0 action list.

# --- R3 #5 — P11 pilot rubric present ---
awk '/^### \*\*P11/,/^## /' ANVIL_PLAN.md | grep -E "Pilot selection rubric|Scope ceiling|Timebox|External user|Domain unrelated|Failure-class triage"
# Expected: 5 matches (one per rubric item).

# --- R3 #6 — Hinge counts derived from registry, not prose ---
grep -E "Total hinges: [0-9]+\." ANVIL_PLAN.md
# Expected: 0 matches (no hard-coded count remains).

# --- R3 #7 — Audit-store threat model named ---
awk '/^### \*\*P2/,/^### \*\*P3a/' ANVIL_PLAN.md | grep "local tamper detection"
# Expected: ≥1 match in P2 acceptance criteria.

# --- R3 #8 — Pin convention present ---
grep "Pin convention" ANVIL_PLAN.md
# Expected: ≥1 match in Deferred-Decision Registry.

# --- R3 #9 — Distribution acceptance details locked ---
awk '/^- \*\*Distribution\.\*\*/,/^- \*\*/' ANVIL_PLAN.md | grep -E "Target platforms|Install method|Signing|Smoke tests"
# Expected: 4 matches in the Distribution open-item entry.

# --- R3 #10 — anvil-graph crate-vs-CLI clarified ---
grep -E "anvil-graph.*Rust crate|anvil graph <verb>" ANVIL_PLAN.md
# Expected: ≥1 match (typically in P7 prose).

# --- R3 #11 — Headless operation documented ---
grep -E "^- \*\*Headless / non-interactive operation\.\*\*" ANVIL_PLAN.md
# Expected: 1 match in Cross-Cutting Concerns.

# --- R3 #12 — Cost controls documented ---
grep -E "^- \*\*Model / provider cost controls\.\*\*" ANVIL_PLAN.md
# Expected: 1 match in Cross-Cutting Concerns.

# --- R3 #13 — Multi-workspace clarified as supported-but-uncoordinated ---
grep "supported-but-uncoordinated" ANVIL_PLAN.md
# Expected: 1 match.

# --- R3 hardening notes appended ---
grep -n "^## Hardening Notes (R3 — Consolidated)" PLAN_HARDENING_HISTORY.md
# Expected: 1 match.
```

---

## Bottom line

R3 was a contradiction-resolution round. The Plan had accumulated multiple internal disagreements between sections that had each been locked at different times — phase count, provider routing, streaming invariant, hinge framework timing — and the consequence was that a reviewer (or contributor) reading two sections of the Plan could come away with two incompatible mental models of the same mechanism.

The fix in every case was to align the language across locations, not to weaken any commitment. Trust-boundary invariants stayed locked; the streaming invariant got *more* precise rather than less. Provider routing went from implicit-and-conflicting to explicit-with-a-named-field. Hinge framework timing went from "claimed before built" to "ordinary tests with structured comments, framework lands at P10b." P11's open-ended ship gate got a concrete rubric without losing the external-pilot requirement.

The one Deferred finding (positioning section length) is a structural cleanup that does not affect correctness; it is logged for a future Plan Consolidation pass.

Plan is now at Draft 7 in spirit (R3 absorbed) though the header still reads Draft 6 — the convention used in prior rounds is to bump the draft number only on consolidation passes, not on every round. The Plan's *state* is post-R3.

**Rotation status:** per the locked termination condition (full-pool clean), the next step is the next reviewer in rotation seeing the post-R3 state. If the Coordinator judges the post-R3 state has converged on substance — given R3's findings were *contradiction-resolution* rather than new architectural surface — invoking human-arbiter convergence per *Convergence Safeguards* is a defensible call. The remaining open items at Plan-level (positioning structural pass, cryptographic tamper-proofing, hard-stop cost limits, global sidecar sharing) are all explicitly scoped as post-v1 or v1.x and do not block v1.
