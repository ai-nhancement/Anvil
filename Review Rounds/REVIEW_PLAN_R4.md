# Plan — R4 Disposition

**Date:** 2026-05-19
**Scope:** Response to R4 findings on the Anvil Implementation Plan (post-R3 state). R4 raised seven findings, all Fixed.
**Spec:** `ANVIL_PLAN.md` (updated this round; hardening notes appended to `PLAN_HARDENING_HISTORY.md`).
**Prior rounds:** `REVIEW_PLAN_R1.md`, `REVIEW_PLAN_R2.md`, `REVIEW_PLAN_R3.md`.
**R4 reviewer:** fourth rotation slot from the configured pool; different model family from Coder per Adversarial Diversity floor. Same reviewer family as R2 if rotation pattern is strict alternation.

**Shell assumption for verification commands:** POSIX shell utilities (`grep`, `awk`).

---

## What changed since R3

R4's character was operational-edge-case stress-testing. Where R3 found internal contradictions in language, R4 found *boundary conditions* the locked architecture hadn't fully specified: what happens when the daemon's config goes stale, when reviewers contradict each other, when rollback resets the workflow, when hinges disagree across languages, when the keyring isn't available, when the pilot doesn't exercise the multi-provider abstraction.

Two P1 findings closed real architectural gaps:

- **Split-brain state-drift.** The Plan's Invariant #2 (sidecar stateless) and the persistent workspace-scoped daemon were inconsistent at the `provider-config` layer. A user editing `anvil.toml` between CLI invocations would leave the daemon serving stale routing. The fix adds a `config_epoch` SHA-256 hash to the Handshake handshake-pair, a new `ReloadConfig` RPC to the proto service, and a force-restart fallback when reload fails. Both reload and force-restart emit a `SidecarReload` audit record (a new record type).
- **Per-finding arbiter resolution.** Artifact-level convergence declaration existed; per-finding override did not. R4 surfaced the ping-pong failure mode where two reviewers raise contradictory findings and the full-pool-clean termination becomes structurally impossible. The fix adds `anvil arbiter resolve-finding <finding-id> --reason "<text>"` and the `ArbiterFindingResolution` audit record. Arbiter-Decided findings are excluded from the blocking set on subsequent termination checks; reviewers see them flagged in their input briefing.

The remaining five findings closed operational gaps:

- **Daemon zombie management.** Global `~/.anvil/global-registry.json` tracks active sidecars across workspaces; `anvil sidecar status --all` and `anvil sidecar kill --stale` operate on the registry. Stale-daemon scenarios added to P11 smoke tests.
- **Asymmetric hinge consensus.** `anvil hinge list --strict` runs a cross-language consensus check; asymmetric states are `BlockShip` violations. CI invokes the check on every build.
- **Keyring fallback removed.** File-based encryption was a fragile security surface area; v1 now offers OS keychain (preferred) or env-var-only (floor when keychain unavailable). No homegrown encryption in v1.
- **Rollback rotation reset.** `RollbackEvent` records `rotation_reset_phases`. Re-opening a phase resets rotation to position 0 for the re-opened phase and all transitive dependents.
- **Pilot provider-diversity.** The P11 external pilot rubric now requires at least two distinct provider types in use, validating the multi-provider abstraction under real conditions.

A useful side-effect: R3's pin convention had labeled `test_audit_store_record_types_count` as constitutional, pinned at 11. R4's two new record types revealed that the *constitutional* form is "the 11 required types are all present" (subset check), not "exactly 11 types exist" (count check) — because the Charter's wording is "minimum set; Plan may extend." The hinge is renamed `test_audit_store_required_types_present` and converted to a subset check, which is the correct constitutional reading.

---

## Verification of R4 finding premises

R4 was conceptual rather than line-cited. Each finding's premise was checked against the current Plan state before any edits.

| Finding | Verifiable premise | Verified? | Notes |
|---|---|---|---|
| 1 — Split-brain state drift | Daemon persists; loads `provider-config` at startup; no mechanism to detect or refresh on `anvil.toml` edit | ✓ | Real architectural gap — daemon could serve requests against stale routing |
| 2 — Daemon accumulation | Per-workspace idle-timeout; no global awareness; no stale-daemon recovery | ✓ | Real operational gap — zombie processes accumulate silently |
| 3 — Convergence deadlock | Artifact-level arbiter convergence exists; per-finding override does not | ✓ | Real architectural gap — ping-pong reviewer contradictions could block ship |
| 4 — Asymmetric hinges | Bi-language registry merges hinges; no consensus rule for disagreement | ✓ | Real cross-language integrity gap |
| 5 — Keyring fallback | File-based encryption named as fallback path; fragile security surface area | ✓ | Real surface-area concern; custom encryption is significant scope |
| 6 — Rollback rotation | Rollback invalidates dependents; rotation reset unspecified | ✓ | Real operational gap — late-stage fix could re-ship after one clean pass |
| 7 — Pilot provider-diversity | P11 rubric covers scope, timebox, external user, domain, failure triage — but not provider diversity | ✓ | Real validation gap — Claude-only pilot misses adapter abstraction stress |

Result: 7/7 premises verified.

---

## Disposition of R4 findings

| # | Severity | Finding (one-line) | Disposition |
|---|---|---|---|
| 1 | P1 | Split-brain state drift between Vault and sidecar | **Fixed.** Handshake adds `vault_config_epoch` and `sidecar_config_epoch` SHA-256 fields. New `ReloadConfig` RPC added to proto service. Vault detects mismatch, calls `ReloadConfig`, force-restarts on failure. `SidecarReload` added as audit record type (12th type; Plan extension). P3a, P3b, P3c, P2 updated. |
| 2 | P2 | Sidecar zombie process accumulation | **Fixed.** `~/.anvil/global-registry.json` tracks active daemons across workspaces. `anvil sidecar status --all` and `anvil sidecar kill --stale` operate on registry. Stale-daemon detection added to P11 smoke tests. P3c updated. |
| 3 | P1 | Convergence deadlock from reviewer contradictions | **Fixed.** Per-finding arbiter resolution added to P6. `anvil arbiter resolve-finding <finding-id> --reason "<text>"` creates `ArbiterFindingResolution` audit record (13th type; Plan extension). Arbiter-Decided findings excluded from full-pool-clean blocking set. Reviewers see Arbiter-Decided findings flagged in input briefing. |
| 4 | P2 | Asymmetric hinge-test failure modes across languages | **Fixed.** P10b registry runs consensus check via `anvil hinge list --strict`. Cross-language hinge disagreement (different pinned values, different intended states, missing in one language when registry flags cross-language) is `BlockShip`. CI invokes the check on every build. |
| 5 | P2 | Keyring fallback fragility (file-based encryption) | **Fixed by removal.** File-based encryption removed from v1. P4 now supports OS keychain (preferred) or env-var-only mode (floor when keychain unavailable). Env-var-only choice persists as `ProvisionalLock` with security-warning rationale. No homegrown encryption in v1; reconsider in v1.x if env-var floor proves too friction-heavy. |
| 6 | P2 | Rollback silent re-ship — rotation reset unspecified | **Fixed.** `RollbackEvent` audit record includes `rotation_reset_phases: string[]`. Re-opening a phase resets rotation to position 0 for the re-opened phase and all transitive dependents. New hinge `test_rollback_resets_rotation_on_dependents`. P9 acceptance updated. |
| 7 | P2 | Pilot diversity stress missing | **Fixed.** P11 pilot rubric extends with *Provider diversity stress*: at least two distinct provider types in use. Typical reviewer pool configuration (Coder + reviewers from different vendors) already satisfies this; no extra setup needed in the common case. Provider-diversity adapter failures added to the pilot-blocking failure class. |

---

## Files changed since R3

| File | Action | Purpose | Approximate delta |
|---|---|---|---|
| `ANVIL_PLAN.md` | MODIFY | Apply 7 R4 fixes. P3a adds `ReloadConfig` RPC and config-epoch handshake fields. P3b adds config-epoch validation flow. P3c adds global registry and stale-daemon management. P4 removes file-based encryption fallback; adds env-var-only floor with security-warning provisional lock. P6 adds per-finding arbiter resolution mechanism and acceptance criteria. P9 adds rotation reset on rollback. P10b adds consensus check for bi-language hinges. P11 pilot rubric adds provider-diversity stress. P2 record-types list updated to 13 (added `ArbiterFindingResolution`, `SidecarReload`); audit-store hinge renamed to subset check. Registry table updated. | +~190 lines net |
| `PLAN_HARDENING_HISTORY.md` | MODIFY | Append `Hardening Notes (R4 — Consolidated)` section covering 7 findings with disposition summaries. | +~95 lines |
| `REVIEW_PLAN_R4.md` | CREATE | This document. | ~220 lines |
| `REVIEW_PLAN_R1/R2/R3.md` | (UNTOUCHED) | Per single-writer artifact discipline. |

---

## Corrections to prior dispositions

None this round. R4 did not flag any R3 disposition language as wrong. R3's tightened disposition vocabulary (Fixed / Locked in Charter, enforcement pending Plan / Refuted / Deferred) is preserved.

One subtle clarification on R3's pin convention: R3 labeled `test_audit_store_record_types_count` as *constitutional*. That label was correct in spirit (the hinge is tied to a Charter invariant) but the formulation was wrong — exact-equality at 11 is fragile against the Charter's "minimum set; Plan may extend" wording. R4 corrects this by reformulating the hinge as a subset check (`test_audit_store_required_types_present`). This is not a contradiction with R3; it's the correct expression of the constitutional commitment that R3 named. No correction to the R3 disposition text is needed; the hinge rename is documented in R4's hardening notes.

---

## Residual / deferred

- **File-based credential encryption (post-v1).** Removed from v1 as fragile. May return in v1.x if env-var-only floor proves too friction-heavy for users on no-keychain systems. Any future implementation must use established libraries (e.g., `age`-rs or similar) rather than a homegrown scheme.
- **Provider-diversity stress automation.** P11's rubric requires manual verification that the pilot crosses provider types. An automated check (`anvil pilot validate --diversity`) is a v1.x consideration.
- **Per-finding arbiter resolution UX.** The CLI surface (`anvil arbiter resolve-finding`) is specified; the App's equivalent surface (a per-finding "arbiter override" button with mandatory reasoning field) lands with the v1.1 App.
- **Cryptographic tamper-proofing of audit store** (from R3). Still open as v1.x consideration.
- **Hard-stop cost limits** (from R3). Still warn-only default in v1; enforce requires opt-in.
- **Global sidecar sharing across workspaces.** R4's daemon registry brings the *visibility* improvements (stale detection, per-workspace listing); the deeper *coordination* features (one daemon serving multiple workspaces with rate-limit pooling) remain post-v1.

---

## Reproducibility

**Shell assumption:** POSIX shell utilities (`grep`, `awk`).

```bash
# --- R4 #1 — Split-brain: config epoch fields and ReloadConfig RPC present ---
awk '/^### \*\*P3a/,/^### \*\*P3b/' ANVIL_PLAN.md | grep -E "vault_config_epoch|sidecar_config_epoch|ReloadConfig|Configuration Epoch"
# Expected: ≥4 matches in P3a section.

awk '/^### \*\*P3b/,/^### \*\*P3c/' ANVIL_PLAN.md | grep -E "Configuration-epoch validation|SidecarReload"
# Expected: ≥2 matches in P3b section.

# --- R4 #2 — Global sidecar registry and stale-daemon management ---
awk '/^### \*\*P3c/,/^### \*\*P4/' ANVIL_PLAN.md | grep -E "global-registry|sidecar status --all|sidecar kill --stale|Global-aware sidecar management"
# Expected: ≥4 matches in P3c section.

# --- R4 #3 — Per-finding arbiter resolution ---
awk '/^### \*\*P6/,/^### \*\*P7/' ANVIL_PLAN.md | grep -E "Per-finding arbiter resolution|arbiter resolve-finding|ArbiterFindingResolution|Arbiter-Decided"
# Expected: ≥4 matches in P6 section.

# --- R4 #4 — Asymmetric hinge consensus check ---
awk '/^### \*\*P10b/,/^## /' ANVIL_PLAN.md | grep -E "Registry consensus check|--strict|BlockShip"
# Expected: ≥2 matches in P10b section.

# --- R4 #5 — Keyring fallback removed; env-var floor ---
awk '/^### \*\*P4/,/^### \*\*P5/' ANVIL_PLAN.md | grep -E "File-based encryption with user passphrase is explicitly NOT in v1|env-var-only|Credential storage \(R4 hardening\)"
# Expected: ≥2 matches in P4 section.

# --- R4 #6 — Rollback resets rotation ---
awk '/^### \*\*P9/,/^### \*\*P10a/' ANVIL_PLAN.md | grep -E "rotation_reset_phases|Rotation reset on rollback|test_rollback_resets_rotation"
# Expected: ≥2 matches in P9 section.

# --- R4 #7 — Pilot provider-diversity ---
awk '/^### \*\*P11/,/^## /' ANVIL_PLAN.md | grep -E "Provider diversity stress|at least two distinct provider"
# Expected: ≥1 match in P11 section.

# --- Audit-store record types extended to 13 ---
grep -E "ArbiterFindingResolution|SidecarReload" ANVIL_PLAN.md
# Expected: ≥4 matches (declared in P2 record-types list; referenced in P6 and P3b).

# --- Audit-store hinge converted to subset check ---
grep "test_audit_store_required_types_present" ANVIL_PLAN.md
# Expected: ≥2 matches (P2 hinge list and registry table).

# --- R4 hardening notes appended ---
grep -n "^## Hardening Notes (R4 — Consolidated)" PLAN_HARDENING_HISTORY.md
# Expected: 1 match.
```

---

## Bottom line

R4 was an operational-stress round. The reviewer pressed on edge cases the architecture had not fully specified — what happens when the daemon's config goes stale, when reviewers disagree irreconcilably, when a rollback happens, when hinges disagree across languages, when the keyring is unavailable, when the pilot uses only one vendor — and the Plan now has explicit answers in each case. None of the locked architectural commitments were weakened. Two were *extended* (sidecar statelessness now includes config-epoch validation; convergence safeguards now include per-finding arbiter resolution); five were *operationalized* (daemon accumulation, hinge consensus, keyring policy, rollback rotation, pilot diversity).

The Plan's audit-store record-type count is now 13 (11 Charter-required + 2 Plan extensions: `ArbiterFindingResolution`, `SidecarReload`), with the constitutional hinge correctly reformulated as a subset check that enforces the Charter's "minimum set" wording without constraining legitimate Plan-level growth.

**Rotation status:** the Plan has now been through four review rounds (R1, R2, R3, R4) across multiple drafts. The trajectory across rounds:

- R1: structural gaps (11 findings, lots of P1 architectural surface)
- R2: refinements (6 findings, fewer P1, more P2)
- R3: contradiction-resolution (14 findings, language alignment)
- R4: operational edge cases (7 findings, boundary conditions)

This is the trajectory of a converging artifact. R4 did not surface new architectural surface area; it tightened the operational specification of decisions already made. The pattern matches what the Coordinator identified during the Charter rounds as the signal for human-arbiter convergence: when findings shift from "missing structure" to "edge-case refinement," further rounds produce diminishing returns relative to the cost of running them.

The Coordinator may reasonably invoke human-arbiter convergence on the Plan at this point, or send to the next reviewer for R5. Either is defensible. The remaining open items (positioning structural pass, cryptographic tamper-proofing, hard-stop cost limits, global sidecar sharing, post-v1 keyring revisit) are all explicitly post-v1 or v1.x and do not block.
