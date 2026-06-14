# Anvil Plan — Review Round 2 Disposition

**Artifact:** `ANVIL_PLAN.md`
**Round:** R2
**Reviewer:** Second pool slot (Gemini-class, Google) — pseudonymized per audit-store convention
**Authoring model (Coder):** Claude
**Date:** 2026-05-15
**Pre-revision draft:** Draft 5 (post-R1)
**Post-revision draft:** Draft 6

---

## Summary

R2 raised seven observations against Draft 5 (0 × P1, 5 × P2, 1 structural improvement recommendation, 1 positive observation). No new P1 structural gaps were found — this is the expected trajectory after R1 addressed the major issues. R2 focused on operational refinement: resource management, physical data integrity, headless usage, and UX forward-planning.

Finding 2 (checkpoint/resume for partial output) asked the Plan to relax a locked Plan-Level Trust-Boundary Invariant. The invariant is maintained; the cost trade-off is documented and a post-v1 path is registered.

The external pilot praise (Finding 7) is a positive observation; no action required.

---

## Verification of R2 Claims

| Finding | Claim | Verification |
|---|---|---|
| 1 (P2) | Multiple workspace daemons could accumulate resources; no global limit documented | **Grounded.** Draft 5's sidecar lifecycle lock described workspace-scoped daemon behavior but was silent on simultaneous multi-workspace scenarios. The single-active-project constraint reduces practical risk but the behavior was undocumented. |
| 2 (P2) | Discarding nearly-complete long streams is expensive in tokens; checkpoint/resume suggested | **Grounded as a concern; suggestion declined.** The cost trade-off is real. The suggestion requires relaxing Plan-Level Trust-Boundary Invariant #1, which is locked. Relaxing it in v1 without a verified partial-stream validation mechanism would undermine the invariant's purpose. |
| 3 (P2) | P4's keyring fallback to file-based encryption with passphrase blocks in non-interactive (CI) environments | **Grounded.** Draft 5 P4 acceptance criteria had no test case for headless or CI key supply. The keyring `crate` with a passphrase fallback would either block waiting for terminal input or fail in non-interactive shells. |
| 4 (P2) | No UX audit milestone for mapping CLI commands to GUI before v1.1 App design | **Grounded.** Draft 5 P11 converted Provisional Locks but produced no structured artifact mapping CLI commands to GUI equivalents. The v1.1 designers would derive this mapping from scratch without documented guidance. |
| 5 (P2) | `O_CREATE\|O_EXCL` prevents overwriting but not physical deletion of audit records | **Grounded.** Draft 5 P2 described append-only enforcement at API level (no `update`/`delete` methods) and filesystem level (`O_CREATE\|O_EXCL`). Physical deletion of files is not prevented or detected by either mechanism. |
| 6 (P3 improvement) | P10 should be split by default; evaluation and hinge-testing are different engineering tasks | **Grounded.** The split trigger in Draft 5 acknowledged the risk; making the split default is cleaner and reduces phase scope from the start. |
| 7 (positive) | External pilot is the single most important test for whether Anvil is a Product | **Noted positively.** No action required. |

---

## Disposition of R2 Findings

| ID | Severity | Finding | Disposition | Notes |
|---|---|---|---|---|
| 1 | P2 | Multi-workspace daemon resource accumulation undocumented | **Fixed** | Behavior documented in Cross-Cutting Concerns (sidecar lifecycle entry). Risk entry added. Global sidecar sharing added to Open Items as post-v1. No v1 behavior change. |
| 2 | P2 | Partial-output discard cost; checkpoint/resume suggested | **Acknowledged — invariant maintained; registered post-v1** | Plan-Level Trust-Boundary Invariant #1 is locked and not relaxed. Cost trade-off documented in Risks. Checkpoint/resume added to Open Items as post-v1 item, conditional on P11 data. |
| 3 | P2 | CI/headless keyring blocks on interactive passphrase | **Fixed** | Env-var bypass added to P4 acceptance criteria (criterion 11). Hinge `test_api_keys_env_var_bypass_works_headless` added. |
| 4 | P2 | No structured CLI→GUI mapping for v1.1 Provisional Lock reviews | **Fixed** | CLI UX audit action added to P11. Output document: `docs/ux-audit.md`. Becomes primary input to the two "v1.1 prep" Provisional Lock reviews. |
| 5 | P2 | Physical audit record deletion not detected | **Fixed** | `_index.json` updated atomically on every `append()`. Integrity check extended to compare index against physically present files; missing file = `BlockShip`. Hinge `test_audit_store_detects_deleted_records` added. P2 action list and acceptance criteria updated. |
| 6 | structural | P10 split should be default, not trigger-based | **Fixed** | P10 replaced by P10a (Evaluation Infrastructure) and P10b (Hinge-Test Framework). Both parallel with P9 and each other. Phase count 14 → 15. All hinge references, dependency graph, and registry updated. |
| 7 | positive | External pilot is the key product-vs-tool test | **Noted** | No action. The criterion is already in P11 acceptance criteria and Plan-Level Acceptance Criteria. |

---

## Files Changed Since Draft 5

| File | Action | Purpose |
|---|---|---|
| `ANVIL_PLAN.md` | Updated (Draft 5 → Draft 6) | Applied R2 findings; P10 split, P2/P4 additions, P11 UX audit, Risks and Open Items updates |
| `PLAN_HARDENING_HISTORY.md` | Updated | R2 hardening notes appended |
| `REVIEW_PLAN_R2.md` | Created (this file) | R2 disposition document |

---

## Corrections to the R2 Narrative

**Finding 2 correction.** The reviewer's suggestion to allow the Vault to preserve "the grounded portion of a long stream, provided it can be verified as structurally sound" implies that partial stream verification is straightforward. It is not: the invariant exists precisely because partial outputs can appear structurally sound while being semantically incomplete (e.g., a truncated Charter rendering that looks like a valid section but omits constraints). Verification of partial completeness requires a content-level schema that does not exist in the v1 contract. The invariant is not a purity preference; it is a guard against a class of bugs that are hard to detect after the fact. Post-v1, if this mechanism is designed, it requires defining the completeness schema and the verification protocol — not just a checkpoint flag.

No other corrections to the R2 narrative required.

---

## Residual / Deferred

**Finding 2 (partial-output checkpoint/resume):** Deferred to post-v1, conditional on P11 observational data showing it is a meaningful user pain point after retry/backoff is exhausted. The invariant is maintained in v1.

**Global sidecar sharing (Finding 1 post-v1 item):** Deferred to post-v1. v1's single-active-project constraint bounds practical risk.

All other R2 findings are fully resolved in Draft 6.

---

## Reproducibility

Commands assume PowerShell (Windows); POSIX-shell equivalents use `grep -n`.

**Finding 5 — Deletion detection in P2:**
```powershell
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "test_audit_store_detects_deleted_records"
# Expected: match in P2 hinge-test list and Deferred-Decision Registry

Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "physically present files"
# Expected: match in P2 action list
```

**Finding 3 — CI/headless env-var bypass in P4:**
```powershell
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "ANVIL_API_KEY_ANTHROPIC"
# Expected: match in P4 acceptance criteria

Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "test_api_keys_env_var_bypass_works_headless"
# Expected: match in P4 hinge-test list and Deferred-Decision Registry
```

**Finding 6 — P10 split:**
```powershell
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "P10a"
# Expected: multiple matches (phase header, dependency graph, registry, executive summary)

Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "P10b"
# Expected: multiple matches

Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "Total hinges: 46"
# Expected: 1 match in Deferred-Decision Registry
```

**Finding 4 — UX audit in P11:**
```powershell
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "ux-audit"
# Expected: match in P11 action list
```

**Finding 1 — Multi-workspace documentation:**
```powershell
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "Multi-workspace"
# Expected: matches in Cross-Cutting Concerns and Risks

Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "Global sidecar sharing"
# Expected: match in Open Items
```

**Finding 2 — Invariant maintained:**
```powershell
Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "Plan-Level Trust-Boundary Invariant #1"
# Expected: match in Risks section with "not relaxed in v1" language

Select-String -Path C:\Anvil\ANVIL_PLAN.md -Pattern "checkpoint.resume"
# Expected: match in Open Items (post-v1 item) and Risks
```
