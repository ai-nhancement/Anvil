# P11 Dogfooding and Documentation — Review Briefing (R10)

**Date:** 2026-05-27  
**Scope:** P11 R9 finding responses — 6 applied via code/doc changes; 1 addressed via briefing-language correction only  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (clean); R1 second-pass (8 findings); R2 (5); R3 (7); R4 (5); R5 (7); R6 (5); R7 (6); R8 (4); R9 (7). All findings addressed across all rounds.

---

## R9 Finding Responses

### F1 (High) — P11 per-phase ACs included deferred items while Gate 1 claimed all per-phase ACs satisfied

**Disposition: Applied.**

The P11 phase acceptance criteria block (`ANVIL_PLAN.md` §P11) rewritten with explicit Gate 1 / Gate 2 labels:

**Gate 1 — satisfied at P11 ship:**
1. Every Provisional Lock confirmed (→ Final) or revised with audit record.
2. Runbook covers all P11 Coordinator workflows and gate operations.
3. Onboarding guide can be followed by a new user without consulting the runbook.
4. Representative dogfooding + external pilot artifacts in `docs/examples/`, with formal Coordinator attestation. Documentation deliverable complete; live CLI execution is a Gate 2 requirement.

**Gate 2 — deferred; required before public ship:**
1. Live Charter → Plan cycle via `anvil` CLI (live execution, audit-store records preserved).
2. Live external pilot: full Charter → Plan → Build → Ship (live execution, audit-store records preserved).
3. External pilot includes at least one Build phase with multi-reviewer rotation (live execution).
4. v1.1 Plan from live dogfooding validated as v1.1 App design input.

Plan-Level Gate 1 criterion 1 updated from "all per-phase acceptance criteria" to "all Gate-1-applicable per-phase acceptance criteria; P11's live dogfooding and external pilot criteria are Gate 2 requirements."

---

### F2 (High/Medium) — Gate 1 included release-archive/signature/smoke-test requirements explicitly not P11 deliverables

**Disposition: Applied.**

Gate 1 criterion 9 trimmed to what is currently validated:

> v1 binaries (`anvil`, `anvil-sidecar`) build and test correctly for the primary platform (Windows x64); stretch platforms best-effort. Validated by `cargo test --workspace` and `go test ./...` passing on the primary platform.

The release-time requirements (release archive, `SHA256SUMS.txt.asc`, smoke-test script passes against release candidate) moved to Gate 2 as criterion 3. Gate 1 "Status: Complete" now refers exclusively to criteria that are currently satisfied.

---

### F3 (Medium/High) — Representative child artifacts used live-run semantics in metadata and status fields

**Disposition: Applied.**

Six artifacts updated:

- **`docs/examples/dogfooding/v11-plan-summary.md`**: `Status: Converged (R1 clean pass via dogfooding session)` → `Status: Representative (shows converged shape expected from a live dogfooding session; not a live anvil plan invoke output)`. Opening sentence updated to "shows the representative phase-level output expected from."

- **`docs/examples/external-pilot/audit-store-summary.EXAMPLE.json`**: `"outcome": "shipped"` → `"outcome": "representative_shipped_shape"`; `"provider_diversity_stress": "pass"` → `"provider_diversity_stress": "pending_live_validation"`; `"integrity_check": "pass"` → `"integrity_check": "representative_pass_shape"`; note field updated; pilot_period clarified as representative timebox.

- **`docs/examples/external-pilot/charter.md`**: `Version: R2 (final, converged)` → `Version: R2 (representative final/converged form — not a live anvil charter review output)`.

- **`docs/examples/external-pilot/LEAFLOG_PLAN.md`**: `Status: Approved (R1 converged; hardening applied)` → `Status: Representative (approved, R1 converged form — not a live anvil plan invoke output)`.

- **`docs/examples/external-pilot/README.md`** (Pilot Selection Rationale table): `Completed in 6 days` → `Representative 6-day timebox`.

- **`docs/examples/external-pilot/README.md`** (Artifacts): `charter.md — final converged charter (R2 clean pass)` → `charter.md — representative final/converged charter (R2 clean pass shape)`; same for LEAFLOG_PLAN.md.

---

### F4 (Medium) — coordinator-attestation.md overclaimed representative artifacts as validation evidence; "provider diversity behavior" conflicted with deferred live run

**Disposition: Applied.**

Four changes:

1. Intro paragraph: added explicit Gate 1/Gate 2 statement — "Gate 1 is satisfied for documentation only; Gate 2 (public ship) requires live audit-store evidence and remains unsatisfied."

2. Reason #3 in "Why Live Evidence Is Not Available": "The representative artifacts **validate** that the Anvil workflow…" → "The representative artifacts **illustrate** that the Anvil workflow…" + added "These artifacts are documentation deliverables, not substitutes for Gate 2 live evidence."

3. Sign-off item 1: "accurately represent" → "accurately illustrate … they are documentation deliverables, not Gate 2 evidence."

4. Sign-off item 3: "The UX friction points, **provider diversity behavior**, and workflow gaps … are accurately drawn from knowledge of the CLI's implementation and from operating it during the build process" → removed "provider diversity behavior" from the claim; added "Provider diversity behavior shown in the representative artifacts is expected based on adapter conformance testing; live provider call validation is a Gate 2 requirement."

---

### F5 (Medium) — Plan/history prose still referenced "P11 dogfooding" as if live execution occurred

**Disposition: Applied.**

Five locations updated:

1. **Required Choices table** (lines 199–200): "Reviewed against v1 usage in P11 dogfooding and `docs/ux-audit.md`" → "Reviewed against build observations and `docs/ux-audit.md`"; "Reviewed against `docs/ux-audit.md` during P11 dogfooding" → "during P11 build."

2. **v1 → v1.1 Transition section** (line 1188): "Reviewed against `docs/ux-audit.md` and P11 dogfooding" → "Reviewed against `docs/ux-audit.md` and build observations."

3. **v1.1 Design Seeds intro** (line 1037): "observational data from P11 dogfooding and pilot" → "live Gate 2 evidence from P11 dogfooding and pilot runs."

4. **Provider adapter roadmap** (line 1027): "informed by P11 dogfooding and pilot feedback" → "will be informed by live Gate 2 dogfooding and pilot evidence."

5. **PLAN_HARDENING_HISTORY.md Amendment 7** (lines 506, 519–520): "during P11 dogfooding" → "at P11 build" / "via the v1.1-prep trigger evaluation at P11 build."

---

### F6 (Low/Medium) — Contract smoke test had no comment clarifying substring matching is intentional

**Disposition: Applied.**

Added two-line comment above the service-name assertion:

```rust
// Substring match is intentional — this is a smoke test, not a schema validator.
// Structured proto-vs-doc checking is a v1.1 task.
```

The RPC-name assertion already had this explained in the surrounding test comment; now both assertions carry the explicit note.

---

### F7 (Low) — External-pilot README had stale `--yes` help-output UX gap claim

**Disposition: Applied.**

The Failure Classification section updated:
- Removed UX gap #3 (`--yes` flag not in `--help`), which is no longer accurate — `anvil phase reopen --help` shows both `-y` and `--yes`.
- Section header changed from "No pilot-blocking failures occurred" → "No pilot-blocking failures are expected in the representative flow."
- UX gap items reframed as "representative UX friction expected from build observations, to be confirmed in live run."

---

## Acceptance Criteria Status

| AC | Criterion | Status |
|---|---|---|
| AC1 | `docs/runbook.md` covers all P0–P11 Coordinator workflows | **PASS** |
| AC2 | `docs/onboarding.md` covers contributor setup and first commands | **PASS** |
| AC3 | `docs/contract.md` documents the sidecar gRPC contract; hinge smoke-checks service name + all RPC names | **PASS (smoke test; full schema sync is v1.1)** |
| AC4 | All 8 Provisional Locks confirmed Final (Gate 1) | **PASS** |
| AC5 | Hinge test: section-scoped (trim-normalized), count + forward + reverse slug check | **PASS** |
| AC6 | Publication-safety gate documented; execution deferred to actual public flip | **PASS** |
| AC7 | `docs/ux-audit.md` covers CLI→App friction and v1.1 recommendations | **PASS** |
| Gate 2 AC1 | Live dogfooding via v1 CLI against real AI providers | **Deferred (attested)** |
| Gate 2 AC2 | Live external pilot: full cycle with multi-reviewer rotation | **Deferred (attested)** |
| Gate 2 AC3 | Release archive, signed checksum, smoke-test script | **Deferred (release-time)** |

---

## Files Changed Since R9

| File | Change |
|---|---|
| `crates/anvil-cli/src/p11.rs` | Intentional-substring comment added to service-name assertion |
| `Anvil Plan/ANVIL_PLAN.md` | P11 phase ACs split Gate 1/Gate 2; Gate 1 criterion 1 scoped; criterion 9 trimmed; release reqs added to Gate 2; Required Choices table and v1.1 sections updated |
| `Anvil Plan/PLAN_HARDENING_HISTORY.md` | Amendment 7 "dogfooding" → "P11 build" |
| `docs/examples/coordinator-attestation.md` | Gate 1/Gate 2 statement; "validate" → "illustrate"; provider diversity narrowed |
| `docs/examples/dogfooding/v11-plan-summary.md` | Status and opening sentence updated |
| `docs/examples/external-pilot/audit-store-summary.EXAMPLE.json` | outcome, integrity_check, provider_diversity_stress fields updated |
| `docs/examples/external-pilot/charter.md` | Version field updated |
| `docs/examples/external-pilot/LEAFLOG_PLAN.md` | Status field updated |
| `docs/examples/external-pilot/README.md` | Timebox, failure classification, artifact labels updated |
| `Review Rounds/REVIEW_P11_DOGFOODING_R9_Findings.md` | Added (reviewer's R9 findings document) |

**Commit:** `47288de` — "P11 R9 findings: P11 AC split, Gate 1/2 reconciliation, release reqs to Gate 2, representative artifact labels, attestation language, dogfooding prose (R9_Findings approved)"
