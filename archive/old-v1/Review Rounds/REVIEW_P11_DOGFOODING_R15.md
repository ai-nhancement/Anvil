# P11 Dogfooding and Documentation — Review Briefing (R15)

**Date:** 2026-05-27  
**Scope:** P11 R14 finding responses — F1 factual correction (no change); F2 and F3 applied via code/doc changes  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (clean); R1 second-pass (8 findings); R2 (5); R3 (7); R4 (5); R5 (7); R6 (5); R7 (6); R8 (4); R9 (7); R10 (4); R11 (7); R12 (4); R13 (5); R14 (3). All findings addressed across all rounds.

---

## R14 Finding Responses

### F1 (High) — Hinge test name still contains "dogfooding" and was never renamed

**Disposition: Factual correction — no change needed.**

The test was renamed in R12 (commit `8ffc2e6`, "P11 R12: rename PL hinge test..."). The R13 briefing documented the rename under "F2 (Medium)" with a seven-file change table. The current state of both files is:

- `crates/anvil-cli/src/p11.rs:5`: `// hinge_test: pins=pl-all-resolved, intended=test_no_outstanding_provisional_locks_at_p11_gate1, phase=P11`
- `crates/anvil-cli/src/p11.rs:7`: `fn test_no_outstanding_provisional_locks_at_p11_gate1()`
- `Anvil Plan/ANVIL_PLAN.md:840`: `- \`test_no_outstanding_provisional_locks_at_p11_gate1\``

The hinge registry (`anvil hinge list`) reports the renamed identifier. R14's validation section did not include a grep or source inspection for the test name, which led to the finding. No code or documentation change is required; the identifier has been consistent since R12.

---

### F2 (Medium) — AC5 "fully trim-normalized" language overstated the synchronization guarantee

**Disposition: Applied.**

`crates/anvil-cli/src/p11.rs` reverse-check comment updated:

```rust
// Before
// Together with the forward check and the count assertion, this is a full
// bidirectional synchronization — neither side can add a slug without the other.

// After
// Together with the forward check and the count assertion, this is a runtime
// bidirectional synchronization (enforced at `cargo test` time) — neither side
// can add a slug without the other, but divergence is only caught when tests run.
```

AC5 description in this briefing's table reflects the corrected language: "runtime bidirectional slug check with full trim normalization on header, filter, and extraction."

---

### F3 (Low) — Smoke-test Open Item referenced "Distribution command list" without disambiguating it from the Windows daemon scenarios

**Disposition: Applied.**

`ANVIL_PLAN.md` Open Items, release-time smoke-test entry rewritten to make the scope boundary explicit:

- Names the core five commands (extract, `anvil --version`, `anvil-sidecar --version`, `anvil init`, `anvil hinge list`, verify `anvil.toml`) as the release-archive smoke-test scope.
- Adds the Distribution section's unsigned-binary warning verification requirement as a second explicit scope item.
- Explicitly excludes the Windows daemon robustness scenarios (logoff, sleep, fast-user-switching, AV quarantine, ungraceful terminal close), naming them as first-week-support tests listed separately.

A release engineer can now determine the smoke-test script scope from the Open Item alone, with the Distribution section available as authoritative detail.

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

## Files Changed Since R14

| File | Change |
|---|---|
| `crates/anvil-cli/src/p11.rs` | Reverse-check comment: "full bidirectional synchronization" → "runtime bidirectional synchronization (enforced at `cargo test` time)" |
| `Anvil Plan/ANVIL_PLAN.md` | Release-time smoke-test Open Item: scope made explicit (core five commands + unsigned-binary warning verification); Windows daemon scenarios explicitly excluded with names |

**Commit:** `162bf1f` — "P11 R14: runtime bidirectional sync comment, smoke-test open item scope clarification (R14_Findings)"
