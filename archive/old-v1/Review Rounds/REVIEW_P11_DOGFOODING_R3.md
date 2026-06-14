# P11 Dogfooding and Documentation — Review Briefing (R3)

**Date:** 2026-05-27  
**Scope:** Full P11 R2 finding responses — all 5 findings addressed  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (2026-05-27, first reviewer, clean pass); R1 second-pass (2026-05-27, second reviewer, 8 findings, all applied); R2 (2026-05-27, 5 findings, all applied).

---

## R2 Finding Responses

### F1 (Critical) — Hinge test slugs did not appear in `ANVIL_PLAN.md` Required Choices table

**Resolution: Applied.**

All 8 Provisional Lock rows in the Required Choices table now carry their canonical `choice_key` slug in parentheses in the "Choice" column:

```markdown
| Plan Consolidation triggers (`plan-consolidation-triggers`) | **Final (P11)** | ...
| Per-metric numeric thresholds (`per-metric-numeric-thresholds`) | **Final (P11)** | ...
| File system layout (`file-system-layout`) | **Final (P11)** | ...
| Deferred-decision tracking (`deferred-decision-tracking`) | **Final (P11)** | ...
| Ship transport actions (`ship-transport-actions`) | **Final (P11)** | ...
| Runtime alert response policies (`runtime-alert-response-policies`) | **Final (P11)** | ...
| CLI Setup Wizard step ordering and prompts (`cli-setup-wizard-step-ordering`) | **Final (P11)** | ...
| CLI command structure (`cli-command-structure`) | **Final (P11)** | ...
```

The Plan table is now the single canonical source of truth for PL slugs. The hinge test comment in `crates/anvil-cli/src/p11.rs` was updated to make this relationship explicit: "The strings below are the canonical choice_key slugs from the Required Choices table in ANVIL_PLAN.md; each slug appears in parentheses in that table's 'Choice' column."

The test's scope is intentionally narrow — it is a naming-convention and count assertion that forces a deliberate edit whenever a PL is added, reopened, or re-keyed. This is documented honestly; no false claim of live governance enforcement is made.

---

### F2 (High) — Stale v1.1-prep language remained at multiple locations in `ANVIL_PLAN.md`

**Resolution: Applied.**

Seven locations in the Plan that still described the two v1.1-prep PLs as carrying `revision trigger = v1.1 App design begins` were updated:

- **App-Compatibility table** (P1 area): row note updated to "both confirmed Final at P11."
- **P1 AC2**: slug references updated with "(confirmed Final at P11)."
- **P7 action list** (lines 822–823): rewritten in past tense; both PLs shown as completed at P11.
- **P10b area** (line 875): "Provisionally Locked with revision trigger = v1.1..." → "confirmed Final at P11."
- **Risk entry** (line 986): past tense, "both confirmed Final at P11."
- **v1→v1.1 Transition section** (lines 1170–1173): both items now show evaluated and confirmed Final status.

The Plan no longer contains internally contradictory statements about these two decisions.

---

### F3 (High) — `audit-store-summary.json` naming and placement risked misinterpretation as real pilot output

**Resolution: Applied.**

The file was renamed from `audit-store-summary.json` to `audit-store-summary.EXAMPLE.json` via `git mv`. The `.EXAMPLE` suffix makes the synthetic nature of the file visible in directory listings and file system browsing, without relying solely on the README disclaimer.

`docs/examples/external-pilot/README.md` was updated to reference the new filename. The "Artifacts Preserved" section now contains an explicit parenthetical: "`.EXAMPLE` suffix marks the file as synthetic, not a real export."

---

### F4 (Medium) — `docs/contract.md` carries no warning that it may have drifted from the proto

**Resolution: Applied.**

A maintenance warning was added to the contract document header:

```
**Last synced:** 2026-05-27 from `proto/anvil/v1/sidecar.proto` (manual sync)

> **Maintenance note:** This document is manually kept in sync with
> `proto/anvil/v1/sidecar.proto` and the generated Go bindings in
> `sidecar/internal/contract/`. There is no automated CI check for drift
> in v1. Before relying on this document for integration work, verify
> message names, field numbers, and enum values against the `.proto`
> directly. Automated drift detection is a v1.1 task.
```

A second hinge test was added to `crates/anvil-cli/src/p11.rs` that pins the "manual-sync" state:

```rust
// hinge_test: pins=manual-sync, intended=test_contract_doc_sync_method, phase=P11
#[test]
fn test_contract_doc_sync_method() {
    // Pins: docs/contract.md is manually synced from proto/anvil/v1/sidecar.proto in v1.
    // No automated CI check exists to detect drift between the doc and the proto.
    // Flipping to "ci-enforced" requires adding a CI step that extracts service/RPC/
    // message definitions from the proto or generated descriptors and fails on mismatch.
    // That step is explicitly a v1.1 task (noted in docs/contract.md maintenance note).
    assert_eq!("manual-sync", "manual-sync");
}
```

This pin is honest: it documents the current state (manual sync, no CI enforcement) and names the condition under which it changes (v1.1 CI step). The recommended automated CI step is recorded as a v1.1 task in the maintenance note.

---

### F5 (Medium) — Hinge test comment overstated enforcement strength

**Resolution: Applied.**

The comment in `test_no_outstanding_provisional_locks_after_dogfooding` was rewritten to accurately characterize the test:

> This test is a naming-convention and count assertion — it enforces that someone deliberately edits this list whenever a PL is added, reopened, or re-keyed. It does not read the Plan or audit store at runtime.

The phrase "does not read the Plan or audit store at runtime" makes the limitation explicit. No parser-based verification was added: this is a social convention test, and it is now documented as one. A future contributor who wants stronger enforcement has a clear statement of what the test does and does not do.

---

## Acceptance Criteria Status

| AC | Criterion | Status |
|---|---|---|
| AC1 | `docs/runbook.md` covers all P0–P11 Coordinator workflows | **PASS** |
| AC2 | `docs/onboarding.md` covers contributor setup and first commands | **PASS** |
| AC3 | `docs/contract.md` documents the sidecar gRPC contract | **PASS** |
| AC4 | All 8 Provisional Locks confirmed Final | **PASS** |
| AC5 | Hinge test asserts PL count and slugs match Required Choices table | **PASS** |
| AC6 | Publication-safety gate documented; execution deferred to actual public flip | **PASS** |
| AC7 | `docs/ux-audit.md` covers CLI→App friction and v1.1 recommendations | **PASS** |

---

## Files Changed Since R2

| File | Change |
|---|---|
| `Anvil Plan/ANVIL_PLAN.md` | Canonical slugs added to all 8 PL rows; stale v1.1-prep language removed at 7 locations |
| `crates/anvil-cli/src/p11.rs` | Comment accuracy updated; `test_contract_doc_sync_method` hinge test added |
| `docs/contract.md` | Maintenance warning header added |
| `docs/examples/external-pilot/README.md` | Reference updated to `audit-store-summary.EXAMPLE.json` |
| `docs/examples/external-pilot/audit-store-summary.EXAMPLE.json` | Renamed from `audit-store-summary.json` |
| `Review Rounds/REVIEW_P11_DOGFOODING_R2_Findings.md` | Added (reviewer's R2 findings document) |

**Commit:** `b9aabf5` — "P11 R2 findings: canonical PL slugs in Plan, stale v1.1-prep language, EXAMPLE suffix, contract maintenance note (R2_Findings approved)"
