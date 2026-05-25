# Anvil Plan — Review Round 1 Disposition

**Artifact:** `ANVIL_PLAN.md`
**Round:** R1
**Reviewer:** Codex-class (OpenAI) — pseudonymized per audit-store convention
**Authoring model (Coder):** Claude
**Date:** 2026-05-15
**Pre-revision draft:** Draft 4
**Post-revision draft:** Draft 5

---

## Summary

R1 raised eleven findings against Draft 4 of the Plan (4 × P1, 5 × P2, 2 × P3). The central diagnosis was accurate: several operationally critical decisions were half-deferred while the Plan already depended on them as settled. The four P1 findings identified the right set of must-fix items. All findings were addressed; one (Finding 11, encoding) was refuted on factual grounds consistent with prior Charter review rulings.

The Plan is structurally stronger for this round. It is now self-contained as an implementation contract: no "As Draft 2" placeholders remain, the sidecar lifecycle is locked, the trust-boundary rules are enforced rather than aspirational, and the acceptance model has two orthogonal validation streams.

---

## Verification of R1 Claims

Each claim is verified against the Draft 4 text.

| Finding | Claim | Verification |
|---|---|---|
| 1 (P1) | Plan treats trust-boundary rules as locked while formally deferring them | **Grounded.** `ANVIL_PLAN.md:65` and `:724` in Draft 4 listed them as "candidate charter amendments after Plan convergence"; `ANVIL_PLAN.md:308` and `:318` implemented them as contract invariants. Contradiction confirmed. |
| 2 (P1) | Sidecar lifecycle under-specified; "user starts manually" is not a small note | **Grounded.** `ANVIL_PLAN.md:716` in Draft 4 listed sidecar lifecycle as an open item with "user starts anvil-sidecar manually" as the stated v1 behavior. The sidecar is central to every workflow phase from P5 onward. |
| 3 (P1) | Acceptance model overweights dogfooding; does not validate Build/Review/Ship on a real project | **Grounded.** `ANVIL_PLAN.md:57`, `:581`, `:737` in Draft 4 specified dogfooding as the primary acceptance criterion; the dogfooding scope was Charter→Plan only, not Build→Ship. |
| 4 (P1) | "As Draft 2" entries make the Plan non-self-contained | **Grounded.** `ANVIL_PLAN.md:467` (P6 acceptance criteria), `:506` (P8 acceptance criteria), `:525` (P9 acceptance criteria), `:689` (Evaluation Metric Targets), `:750` (Plan Review Process), and multiple risk entries all contained "As Draft 2" placeholders. |
| 5 (P2) | Post-round-5 behavior for P2/P3 findings is undefined | **Grounded.** Neither the Required Choices table nor P6 specified what P2/P3 findings *become* after round 5 — auto-deferred, advisory with required human action, or otherwise. |
| 6 (P2) | App assumptions embedded in v1 without acknowledgment | **Grounded.** The Plan made v1 decisions explicitly for App coexistence (Vault as library, spawn logic placement, etc.) without naming them as such. A v1.1 designer reading the Plan would see these as arbitrary CLI choices. |
| 7 (P2) | Planner Contract compliance claimed but not visible; evaluation-metric-impact field absent from all phases | **Grounded.** The header at `ANVIL_PLAN.md:7` claimed compliance. No section mapped contract fields to Plan structure. All 14 phases were missing the evaluation-metric-impact field required by the Charter's Planner Contract. |
| 8 (P2) | P10 too dense; no split trigger defined | **Grounded.** P10 combined six distinct work streams. The defense for the merge was stated but no split trigger was pre-decided, leaving the phase as a schedule risk without a defined resolution mechanism. |
| 9 (P2) | `anvil init` vs `anvil setup` naming deferred but architecturally consequential | **Grounded.** `ANVIL_PLAN.md:718` in Draft 4 deferred the naming to P4. The distinction between idempotent scaffolding and interactive configuration has implications for automation, re-initialization behavior, and App parity. |
| 10 (P3) | Codex/Claude Code analogy as primary CLI-first justification | **Grounded.** `ANVIL_PLAN.md:15` and `:770` in Draft 4 cited the analogy as justification. The workflow-based argument (gate-heavy workflow, terminal as prompt surface) is stronger and was present but secondary. |
| 11 (P3) | "Encoding corruption throughout the file, including the title line" | **Refuted.** The title line contains an em-dash (U+2014), which is valid UTF-8. The Charter's Artifact Encoding invariant (invariant section) explicitly permits non-ASCII typography — em-dashes, smart quotes, mathematical symbols — as intentional, distinguishing them from invalid byte sequences. This is the third time this finding pattern has appeared: R1 #10 and R3 #12 of Charter Review were factually refuted on identical grounds. The reviewer's environment may be rendering valid UTF-8 as garbled output; the pre-flight check (P2) is the operational mitigation for this class of tool-configuration issue. No plan document change warranted. |

---

## Disposition of R1 Findings

| ID | Severity | Finding | Disposition | Notes |
|---|---|---|---|---|
| 1 | P1 | Trust-boundary rules deferred while already depended on | **Fixed** | New *Plan-Level Trust-Boundary Invariants* section added. Three rules locked. *Post-Convergence Charter Amendments* section explains the Charter-promotion plan. |
| 2 | P1 | Sidecar lifecycle under-specified | **Fixed** | Sidecar lifecycle locked as Final in Required Project-Level Choices table: workspace-scoped daemon, CLI-managed, spawn logic in `anvil-core`. P3c acceptance criteria updated. Removed from Open Items. |
| 3 | P1 | Acceptance model insufficient for external users | **Fixed** | External pilot added to P11 action list and acceptance criteria. Plan-Level Acceptance Criterion #3 added. Dogfooding criterion preserved. |
| 4 | P1 | "As Draft 2" entries make Plan non-self-contained | **Fixed** | All placeholders inlined: P6, P8, P9 acceptance criteria written in full; Evaluation Metric Targets expanded; Plan Review Process written in full; five risk entries written in full; five Open Items entries written in full. |
| 5 | P2 | Post-round-5 severity-tiering behavior undefined | **Fixed** | Advisory-finding model added to P6 action list and acceptance criteria. Three advisory disposition types defined (`Accept-Advisory`, `Drop-Advisory`, `Defer-Advisory`). Gate check behavior specified. Audit record flag (`advisory: true`) specified. |
| 6 | P2 | App assumptions embedded without acknowledgment | **Fixed** | New *App-Compatibility Design Decisions* section added with eight-row decision table. Executive Summary updated to reference it. |
| 7 | P2 | Planner Contract not visible; evaluation-metric-impact missing | **Fixed** | New *Planner Contract Compliance* section added. Evaluation-metric-impact field added to all 14 phases. |
| 8 | P2 | P10 too dense; no split trigger | **Fixed** | Split trigger pre-decided in P10 phase description: round 4 is the decision point; split into P10a/P10b if exceeded. Sub-phase structure documented. |
| 9 | P2 | `anvil init` vs `anvil setup` deferred | **Fixed** | Distinction locked: `anvil init` = idempotent scaffold, `anvil setup` = interactive wizard. P1 and P4 action lists updated. Removed from Open Items. New hinge `test_wizard_step_count` now clearly applies to `anvil setup`. |
| 10 | P3 | Codex/Claude Code analogy as primary justification | **Fixed** | Analogy language trimmed from Executive Summary and Bottom Line. Workflow argument (gate-heavy, terminal-as-prompt) leads. Competitive axis section retains Codex/Claude Code reference in the correct framing. |
| 11 | P3 | Encoding corruption in title line | **Refuted** | Em-dash is valid UTF-8 typography, explicitly permitted by Charter Artifact Encoding invariant. No document change. See Verification row above. |

---

## Files Changed Since Draft 4

| File | Action | Purpose |
|---|---|---|
| `ANVIL_PLAN.md` | Updated (Draft 4 → Draft 5) | Applied all 10 addressed R1 findings; 11 structural changes |
| `PLAN_HARDENING_HISTORY.md` | Created | Provenance log for Plan Review rounds; R1 hardening notes recorded |
| `REVIEW_PLAN_R1.md` | Created (this file) | R1 disposition document |

No source code changed; no Charter changed. The Plan is the only normative artifact modified in this round.

---

## Corrections to the R1 Narrative

**Finding 11 correction.** The reviewer's bottom-line summary stated "clean up encoding corruption before treating the plan as canonical." This is factually incorrect: there is no encoding corruption. The characters are valid UTF-8 typography that the Charter explicitly permits. The pre-flight environment check (P2) is the correct operational response to reviewer environments that render valid UTF-8 incorrectly. The Plan is canonical in its current encoding state.

No other corrections to the R1 narrative are required. The R1 bottom line ("tighten four things before review convergence") correctly identified the four P1 findings as the highest-priority items. All four are addressed in Draft 5.

---

## Residual / Deferred

Nothing from R1 is deferred within this round. All ten addressed findings are fully resolved in Draft 5. The one refuted finding (11) requires no further action.

Open items that existed before R1 and remain open:
- Audit store query language (post-v1 scope).
- Reviewer prompt management (v1.1 scope).
- Reviewer findings deduplication (v1.1 scope).
- Performance characterization (to be baselined in P11).
- External pilot project selection (decided in P11).

These are not R1 findings; they are pre-existing deferred decisions documented in the *Open Items* section.

---

## Reproducibility

The following checks verify the state of Draft 5 against the R1 disposition. Commands assume PowerShell (Windows) with the repo at `C:\Anvil\`. POSIX-shell equivalents use `grep -n`.

**Finding 1 — Trust-boundary invariants locked:**
```powershell
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "Plan-Level Trust-Boundary Invariants"
# Expected: at least 3 matches (section header + references)
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "Candidate Charter Amendments"
# Expected: 0 matches (section renamed to Post-Convergence Charter Amendments)
```

**Finding 2 — Sidecar lifecycle locked:**
```powershell
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "Sidecar lifecycle"
# Expected: match in Locked Required Project-Level Choices table with "Final" lock type
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "user starts anvil-sidecar manually"
# Expected: 0 matches
```

**Finding 3 — External pilot in P11:**
```powershell
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "external pilot"
# Expected: matches in P11 action list, P11 acceptance criteria, Plan-Level Acceptance Criteria
```

**Finding 4 — No "As Draft 2" placeholders:**
```powershell
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "As Draft 2"
# Expected: 0 matches
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "Unchanged from Draft"
# Expected: 0 matches
```

**Finding 5 — Advisory-finding model in P6:**
```powershell
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "Accept-Advisory"
# Expected: match in P6 action list
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "advisory: true"
# Expected: match in P6 action list
```

**Finding 7 — Evaluation-metric-impact in all phases:**
```powershell
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "Evaluation-metric impact"
# Expected: 14 matches (one per phase P0–P11, counting P3a/P3b/P3c separately)
```

**Finding 9 — `anvil init` / `anvil setup` distinction:**
```powershell
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "anvil init.*idempotent"
# Expected: match in P1 or Cross-Cutting Concerns
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "anvil setup.*interactive"
# Expected: match in P4 or Cross-Cutting Concerns
```

**Finding 11 — Em-dash in plan header (confirm valid UTF-8, not corruption):**
```powershell
# The following should show the status line with em-dash characters intact:
(Get-Content C:\Anvil\ANVIL_PLAN.md -TotalCount 5) -match "Draft 5"
# Expected: line with "Draft 5" and em-dash separators
```
