# P11 Dogfooding and Documentation — Review Briefing (R13)

**Date:** 2026-05-27  
**Scope:** P11 R12 finding responses — all 4 applied via code/doc changes  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (clean); R1 second-pass (8 findings); R2 (5); R3 (7); R4 (5); R5 (7); R6 (5); R7 (6); R8 (4); R9 (7); R10 (4); R11 (7); R12 (4). All findings addressed across all rounds.

---

## R12 Finding Responses

### F1 (High) — F5 "v1.1 evidence language" cleanup missed two instances at lines 823 and 841

**Disposition: Applied.**

`ANVIL_PLAN.md` line 823 (P11 action list, Provisional Lock resolution bullet):

> "evaluated against the CLI UX audit output and actual v1 usage and confirmed Final."

→

> "evaluated against the CLI UX audit output and build observations and confirmed Final."

`ANVIL_PLAN.md` line 841 (P11 Evaluation-metric impact):

> "Baseline values for all six metrics established from v1 usage; used to validate or revise Layer-2 numeric targets."

→

> "Baseline values for all six metrics will be established from Gate 2 live usage; used to validate or revise Layer-2 numeric targets."

The tense change ("established" → "will be established") is load-bearing: the baselines do not yet exist.

---

### F2 (Medium) — Hinge test name retained "dogfooding" phrasing after R7–R12 systematic cleanup

**Disposition: Applied.**

Renamed `test_no_outstanding_provisional_locks_after_dogfooding` → `test_no_outstanding_provisional_locks_at_p11_gate1` everywhere:

| File | Change |
|---|---|
| `crates/anvil-cli/src/p11.rs:5` | `intended=` field in hinge_test comment |
| `crates/anvil-cli/src/p11.rs:7` | function name |
| `Anvil Plan/ANVIL_PLAN.md:840` | P11 hinge-test list entry |
| `Anvil Plan/ANVIL_PLAN.md:944` | Named hinge registry table row |
| `Anvil Plan/PLAN_HARDENING_HISTORY.md:522` | Amendment 7 hinge reference |
| `docs/examples/dogfooding/README.md:40` | PL evaluation hinge reference |
| `docs/examples/coordinator-attestation.md:52` | Attestation "What Was Actually Validated" reference |

Hinge count verified unchanged: `anvil hinge list --count` returns 74.

Historical review-round documents (R1–R12 briefings/findings) were not updated; they are immutable records of past state.

---

### F3 (Medium) — "Final (P11)" filter ran before trim, leaving the pipeline inconsistently normalized

**Disposition: Applied.**

`crates/anvil-cli/src/p11.rs` line 69:

```rust
// Before
.filter(|line| line.replace("**", "").contains("Final (P11)"))

// After
.filter(|line| line.trim().replace("**", "").contains("Final (P11)"))
```

The full extraction pipeline is now consistently trim-normalized at every step: section-header matching, status-cell filter, row split, and slug extraction. A table row with any combination of leading/trailing whitespace cannot cause a spurious filter-miss or a bidirectional assertion failure.

---

### F4 (Low) — Gate 2 AC4 release-time smoke-test script had no Open Items tracking entry

**Disposition: Applied.**

Added to the Open Items section of `ANVIL_PLAN.md`, immediately after the external pilot selection entry:

> **Release-time smoke-test script (Gate 2 AC4).** A smoke-test script covering the primary `anvil` binary commands must be written at release time and must pass against the release candidate. Script scope: v1 binary surface only (install, `anvil init`, `anvil hinge list`, `anvil phase build`, `anvil phase ship`, `anvil ship`); not a P11 deliverable. Status: deferred to Gate 2 / release engineering.

---

## Acceptance Criteria Status

| AC | Criterion | Status |
|---|---|---|
| AC1 | `docs/runbook.md` covers all P0–P11 Coordinator workflows | **PASS** |
| AC2 | `docs/onboarding.md` covers contributor setup and first commands | **PASS** |
| AC3 | `docs/contract.md` contains every service and RPC name from `sidecar.proto` (substring smoke test only; full schema sync is v1.1) | **PASS** |
| AC4 | All 8 Provisional Locks confirmed Final (Gate 1) | **PASS** |
| AC5 | Hinge test: section-scoped, fully trim-normalized (header + filter + slug), count + forward + reverse bidirectional slug check | **PASS** |
| AC6 | Publication-safety gate documented; execution deferred to actual public flip | **PASS** |
| AC7 | `docs/ux-audit.md` covers CLI→App friction and v1.1 recommendations | **PASS** |
| Gate 2 AC1 | Live dogfooding via v1 CLI against real AI providers | **Deferred (attested)** |
| Gate 2 AC2 | Live external pilot: full cycle with multi-reviewer rotation | **Deferred (attested)** |
| Gate 2 AC3 | v1.1 Plan from live dogfooding validated as v1.1 App design input | **Deferred (attested)** |
| Gate 2 AC4 | Release archive, signed checksum, smoke-test script passes | **Deferred (release-time)** |

---

## Files Changed Since R12

| File | Change |
|---|---|
| `crates/anvil-cli/src/p11.rs` | Hinge comment `intended=` and fn name renamed to `test_no_outstanding_provisional_locks_at_p11_gate1`; `trim()` added before `replace("**", "")` in the "Final (P11)" filter |
| `Anvil Plan/ANVIL_PLAN.md` | Lines 823 and 841: "actual v1 usage" / "v1 usage" → "build observations" / "Gate 2 live usage"; hinge-test list and named registry updated to new test name; Open Items: release-time smoke-test script entry added |
| `Anvil Plan/PLAN_HARDENING_HISTORY.md` | Amendment 7 hinge reference updated to new test name |
| `docs/examples/dogfooding/README.md` | PL hinge reference updated to new test name |
| `docs/examples/coordinator-attestation.md` | "What Was Actually Validated" hinge reference updated to new test name |

**Commit:** `8ffc2e6` — "P11 R12: rename PL hinge test, trim filter, v1-usage cleanup, smoke-test open item (R12_Findings)"
