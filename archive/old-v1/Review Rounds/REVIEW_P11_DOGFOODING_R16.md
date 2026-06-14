# P11 Dogfooding and Documentation — Review Briefing (R16)

**Date:** 2026-05-27  
**Scope:** P11 R15 finding responses — both applied via doc changes  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (clean); R1 second-pass (8 findings); R2 (5); R3 (7); R4 (5); R5 (7); R6 (5); R7 (6); R8 (4); R9 (7); R10 (4); R11 (7); R12 (4); R13 (5); R14 (3); R15 (2). All findings addressed across all rounds.

---

## R15 Finding Responses

### F1 (Medium) — v1.1 Design Seeds had untracked "P11 must record" data obligations outside the Gate 2 acceptance list

**Disposition: Applied.**

Three Design Seed "v1 data points" sections updated in `ANVIL_PLAN.md`:

**Seed 1** (checkpoint/resume, line 1054):
> "P11 must record observed mid-stream error rates..."

→

> "Gate 2 live usage observations should include mid-stream error rates... These are not Gate 2 blockers; they are design-input guidance for v1.1."

**Seed 2** (global sidecar sharing, line 1067):
> "P11 must record the actual multi-workspace usage patterns..."

→

> "Gate 2 live usage observations should capture actual multi-workspace usage patterns... These are not Gate 2 blockers; they are design-input guidance for v1.1."

**Seed 4** (credential encryption, line 1091):
> "P11 and post-v1 user feedback on how many users actually hit the no-keychain case..."

→

> "Gate 2 and post-v1 user feedback on how many users actually hit the no-keychain case... This is not a Gate 2 blocker; it is design-input guidance for v1.1."

The boundary is now explicit in each seed: these are opportunistic observations to capture during Gate 2 live usage, not additional Gate 2 acceptance criteria.

Seed 3 (cryptographic tamper-proofing) and Seed 5 (cost limits) did not use "P11 must record" language and were not changed.

---

### F2 (Low) — Smoke-test Open Item omitted Linux from the unsigned-binary warning parenthetical

**Disposition: Applied.**

`ANVIL_PLAN.md` Open Items, release-time smoke-test entry:

> "the script must also verify unsigned-binary warning text per OS (Windows SmartScreen, macOS Gatekeeper)."

→

> "the script must also verify unsigned-binary warning text per OS (Windows SmartScreen, macOS Gatekeeper, Linux distribution-specific warnings when Linux stretch artifacts are produced)."

The Open Item now matches the Distribution section's full warning-text scope (line 1018).

---

## Acceptance Criteria Status

| AC | Criterion | Status |
|---|---|---|
| AC1 | `docs/runbook.md` covers all P0–P11 Coordinator workflows | **PASS** |
| AC2 | `docs/onboarding.md` covers contributor setup and first commands | **PASS** |
| AC3 | `docs/contract.md` contains every service and RPC name from `sidecar.proto` (substring smoke test only; full schema sync is v1.1) | **PASS** |
| AC4 | All 8 Provisional Locks confirmed Final (Gate 1) | **PASS** |
| AC5 | Hinge test: section-scoped, runtime bidirectional slug check with full trim normalization on header, filter, and extraction | **PASS** |
| AC6 | Publication-safety gate documented; execution deferred to actual public flip | **PASS** |
| AC7 | `docs/ux-audit.md` covers CLI→App friction and v1.1 recommendations | **PASS** |
| Gate 2 AC1 | Live dogfooding via v1 CLI against real AI providers | **Deferred (attested)** |
| Gate 2 AC2 | Live external pilot: full cycle with multi-reviewer rotation | **Deferred (attested)** |
| Gate 2 AC3 | v1.1 Plan from live dogfooding validated as v1.1 App design input | **Deferred (attested)** |
| Gate 2 AC4 | Release archive, signed checksum, smoke-test script passes | **Deferred (release-time)** |

---

## Files Changed Since R15

| File | Change |
|---|---|
| `Anvil Plan/ANVIL_PLAN.md` | Design Seeds 1, 2, 4: "P11 must record" / "P11 and post-v1" → "Gate 2 live usage observations should" with explicit "not a Gate 2 blocker" note; smoke-test Open Item: Linux warning-text verification added to unsigned-binary parenthetical |

**Commit:** `4748c02` — "P11 R15: qualify Design Seed data obligations as Gate 2 guidance, add Linux to smoke-test warning scope (R15_Findings)"
