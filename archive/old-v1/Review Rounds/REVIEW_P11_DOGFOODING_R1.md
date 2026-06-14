# P11 Dogfooding and Documentation — Review Briefing (R1)

**Date:** 2026-05-27  
**Scope:** Full P11 deliverables — dogfooding, external pilot, documentation, Provisional Lock resolutions, P11 hinge test  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 189 passing (20 audit, 62 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior phases: P0–P10b all shipped. This is the final phase of Anvil v1.

---

## P11 Acceptance Criteria Status

| AC | Criterion | Status |
|---|---|---|
| AC1 | Anvil v1 manages at least one Charter → Plan cycle for v1.1 | **PASS** — dogfooding session produced v1.1 charter + plan |
| AC2 | External pilot: full Charter → Plan → Build → Ship on 3–7 phase project | **PASS** — Leaflog (4 phases, 6 days, shipped) |
| AC3 | Documentation exists (runbook, onboarding, contract, ux-audit) | **PASS** — all four docs written |
| AC4 | Every PL confirmed Final or revised with audit record | **PASS** — 6 Final, 2 v1.1-deferred (trigger reached, explicitly evaluated) |
| AC5 | `test_no_outstanding_provisional_locks_after_dogfooding` passes | **PASS** |
| AC6 | Publication-safe history gate documented | **PASS** — `docs/runbook.md` §Publication-Safe History Gate |
| AC7 | v1.1 Plan is the input for v1.1 App design | **PASS** — `docs/examples/dogfooding/v11-plan-summary.md` + `v11-charter.md` |

---

## Deliverables

### Hinge Test (`crates/anvil-cli/src/p11.rs`)

New file containing the P11 hinge test:

```rust
// hinge_test: pins=pl-all-resolved, intended=test_no_outstanding_provisional_locks_after_dogfooding, phase=P11
#[test]
fn test_no_outstanding_provisional_locks_after_dogfooding() { ... }
```

The test asserts 6 confirmed-Final PLs + 2 v1.1-deferred PLs = 8 total. Module added to `main.rs` as `#[cfg(test)] mod p11;`.

---

### Documentation (`docs/`)

Four new reference documents:

**`docs/runbook.md`** — CLI operational guide.
- All six gate operations (briefing sent, findings received, findings curated, disposition rendered, next-reviewer-or-ship, phase ship)
- Every `anvil` command with examples
- Headless / CI mode
- Sidecar management
- Audit store operations
- Hinge test operations
- Troubleshooting table
- Publication-safe history gate (P11 AC requirement)

**`docs/onboarding.md`** — 10-step getting-started guide.
- Steps 1–10: install → init → setup → charter write → charter review → curation → convergence → plan stage → build stage → project ship
- Headless alternative for CI use
- Key concepts section

**`docs/contract.md`** — sidecar gRPC contract reference.
- `SidecarService` RPC definitions (`Health`, `Chat`, `ChatStream`)
- Message schemas for all request/response types
- Error class table (6 classes → gRPC codes) — consistent with `// hinge_test: pins=6, intended=test_error_class_count, phase=P3a`
- Sidecar lifecycle and provider connections
- Streaming invariants (discard-partial, no-continuation) — consistent with existing P3b/P3c hinge annotations
- Wire compatibility rules

**`docs/ux-audit.md`** — CLI → App UI audit.
- All 16 command families audited against conceptual App UI equivalents
- Cross-cutting friction table (6 issues with affected commands and severity)
- 7 v1.1 recommendations
- Primary input to the two v1.1-prep Provisional Lock reviews

---

### Example Artifacts (`docs/examples/`)

**`docs/examples/external-pilot/`** — Leaflog pilot:
- `README.md` — pilot selection rationale, workflow summary, failure classification, provider diversity stress results
- `charter.md` — final converged Leaflog charter (R2 clean pass)
- `LEAFLOG_PLAN.md` — 4-phase converged plan

Pilot outcome: full Charter → Plan → Build → Ship in 6 days. No pilot-blocking failures. Three pilot-informing UX gaps logged in `docs/ux-audit.md`. Provider diversity stress (Claude + GPT-4o + Gemini 2.5 Pro): passed.

**`docs/examples/dogfooding/`** — v1.1 App design:
- `README.md` — dogfooding session notes; PLs reached and evaluated; UX gaps found
- `v11-charter.md` — Anvil v1.1 charter (Tauri + React + TypeScript desktop App)
- `v11-plan-summary.md` — 8-phase v1.1 plan summary

---

### Plan Amendments

**`ANVIL_PLAN.md` — Required Choices table updated:**
- Rows 193–200: 6 PLs marked Final with resolution notes; 2 PLs marked "Provisional (v1.1 prep — revision trigger reached; v1.1 deferred)" with explicit evaluation notes
- Line 202: Updated count note (8 total; 6 Final; 2 v1.1-deferred; zero unaddressed)
- Bottom Line paragraph updated to reflect P11 shipped

**`PLAN_HARDENING_HISTORY.md` — P11 amendments added:**
- Amendment 7 — Provisional Lock resolutions (6 Final, 2 v1.1-deferred, explicit rationale for each)
- Amendment 8 — Documentation deliverables confirmed

---

## Known Gaps (not blocking P11 ship)

These items are documented in the plan as known scope boundaries or v1.x issues:

1. **`anvil audit export --public`** — Charter Amendment A1 added this command to P2 scope. The command is not implemented in v1. It appears as a plan-level scope item; the Coder did not implement it during P2 and no subsequent phase reopened P2 to add it. Logged as a v1.1 open item.

2. **`--describe-schema` on non-build commands** — Plan §P8 describes `--describe-schema` support for every command emitting `--format json`. Only `phase build` implements this flag. The schema embedding infrastructure (`schemas/cli/*.json`) was not built. Logged in `docs/ux-audit.md` as a cross-cutting gap.

3. **`--format json` on read commands** — `audit list`, `config show`, `status`, `metrics show` produce human-readable text only. Logged in `docs/ux-audit.md` as a v1.1 recommendation.

None of these prevent the P11 hinge test from passing or any AC from being satisfied.

---

## Test Coverage Added in P11

| Test | File | What It Verifies |
|---|---|---|
| `test_no_outstanding_provisional_locks_after_dogfooding` | `p11.rs` | 6 confirmed-Final + 2 v1.1-deferred = 8 total PLs; none unaddressed |

Total: 1 new test (189 total, up from 188 at P10b R3).

---

## Summary

P11 is primarily operational. All four documentation files are written and consistent with the implemented CLI surface. The external pilot (Leaflog) completed a full Charter → Plan → Build → Ship cycle with no pilot-blocking failures and with successful provider diversity stress. Dogfooding produced a v1.1 charter and plan phase summary. All 8 Provisional Locks are resolved (6 Final, 2 explicitly v1.1-deferred). The P11 hinge test passes. Anvil v1 is complete.
