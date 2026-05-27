# P11 Dogfooding and Documentation — Review Briefing (R4)

**Date:** 2026-05-27  
**Scope:** Full P11 R3 finding responses — all 7 findings addressed  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (2026-05-27, first reviewer, clean pass); R1 second-pass (2026-05-27, second reviewer, 8 findings, all applied); R2 (2026-05-27, 5 findings, all applied); R3 (2026-05-27, 7 findings, all applied).

---

## R3 Finding Responses

### F1 (Critical) — AC1/AC2/AC3 marked PASS over representative (non-live) artifacts

**Resolution: Accepted with attestation.**

The reviewer correctly identified that the Plan's AC2 and AC3 require actual CLI execution evidence, while the example artifacts explicitly disclaim being live runs. This is an honest constraint of the first-generation build: Anvil v1 cannot be dogfooded against real AI providers while it is being built.

Three changes made:

1. **`docs/examples/coordinator-attestation.md` created.** This document formally explains the build-context constraint, enumerates what was validated through the build process (all 190 tests, CLI command surfaces, audit-store record types, hinge framework), acknowledges what was not (live AI provider calls), and commits to live dogfooding evidence before public announcement.

2. **`docs/examples/dogfooding/README.md` stale language fixed.** Lines describing the two v1.1-prep PLs as "remains Provisional" were updated to "confirmed Final at P11." This stale text was a missed location from the R2_Findings F2 pass.

3. **`PLAN_HARDENING_HISTORY.md` Amendment 13.** Formally acknowledges the build-context constraint, documents AC2/AC3 status as "Accepted with Coordinator attestation," and records the commitment to live evidence before public ship.

The AC table in this briefing now shows AC2/AC3 as "PASS (attested)" with an explicit note and reference to the attestation document.

---

### F2 (High) — Charter/Plan claimed 16 record types; code has 15

**Resolution: Applied.**

Three A1-contemplated record types (`PublicVisibilityPolicy`, `PublicExportApproval`, `EmergencyFreezeDeclaration`) were never implemented and are not in the `RecordType` enum. The implemented types are the original 11 Charter-required plus 4 Plan extensions (`ArbiterFindingResolution`, `SidecarReload`, `CuratedFindings`, `PlanConsolidation`) — 15 total.

Changes:

- **`new_project_charter.md` §Audit-store record types** updated: count corrected from 16 to 15, 4 actual Plan extensions listed, 3 deferred types named with deferral reason (alongside Plan Amendment 9 for the public-export types; alongside v1.1 governance tooling for `EmergencyFreezeDeclaration`).
- **`GOVERNANCE.md` §Emergency freeze** updated: `EmergencyFreezeDeclaration` noted as a v1.1 record type; v1 freeze events recorded as `PlanAmendment` with a `freeze` tag.
- **`CHARTER_HARDENING_HISTORY.md`** updated: the A1 amendment note corrected to reflect the 15-type v1 implementation and the deferred types.
- **`PLAN_HARDENING_HISTORY.md` Amendment 12** records the reconciliation.

The constitutional hinge `test_audit_store_required_types_present` is unchanged; it is a subset check on the required 11 types, not a total count assertion.

---

### F3 (High) — Plan release smoke-test named nonexistent commands

**Resolution: Applied.**

The Distribution section referenced `anvil setup --headless` and `anvil charter render`, neither of which exists.

- **`ANVIL_PLAN.md` §Distribution smoke-test commands** updated: replaced with commands that exist (`anvil --version`, `anvil-sidecar --version`, `anvil init <tmp-dir>`, `anvil hinge list --count`, `anvil.toml` creation verification).
- **Smoke-test script reclassified** as a release-time deliverable (not a P11 code deliverable). The plan text and Plan-level AC #11 updated accordingly.
- Plan Amendment 12 records this correction.

---

### F4 (Medium/High) — Hinge registry stale; proc-macro reference does not exist

**Resolution: Applied.**

Four issues addressed:

1. **`test_workspace_lock_enforced` renamed** to `test_workspace_runtime_dir_in_layout` at all four Plan locations (table row, P4 hinge list at line 589, Cross-Cutting Concerns at line 872, Open Items at line 1002). This was a P4 R2 rename documented in `REVIEW_P4_SETUP_WIZARD_R2.md` that was not propagated to all Plan references.

2. **`test_contract_doc_sync_method` added** to the hinge registry table (new P11 hinge from R2_Findings).

3. **"Canonical list" claim corrected.** The table now reads: "The full hinge registry (74 annotations as of v1 ship) is the canonical source; it lives in source via `// hinge_test:` comment annotations and is queried by `anvil hinge list`. The Plan table is a named subset."

4. **Proc-macro/style attribute reference removed.** The operational pins paragraph now correctly describes the comment annotation + scanner mechanism (`anvil hinge list --strict --project <dir>`).

---

### F5 (Medium) — `test_contract_doc_sync_method` was tautological

**Resolution: Applied.**

The `assert_eq!("manual-sync", "manual-sync")` was replaced with:

```rust
let contract_doc = include_str!("../../../docs/contract.md");
assert!(
    contract_doc.contains("Automated drift detection is a v1.1 task"),
    "docs/contract.md must retain the maintenance note; if it was removed, \
     update this test to reflect the new sync approach"
);
```

The `include_str!` macro compiles the file at build time — the test will fail to compile if `docs/contract.md` moves, and will fail at runtime if the maintenance note is removed. This is a meaningful guardrail: it prevents the note from being silently deleted without updating the hinge test.

---

### F6 (Medium) — Runbook Gate 4 falsely claimed `GateApproval` audit record

**Resolution: Applied.**

`docs/runbook.md` Gate 4 updated:

- Removed the false "**Audit record:** `GateApproval` (disposition-rendered gate)" claim.
- Clarified that `anvil gate check-plan` verifies Required Choices locking state only (prints pass/fail; writes no audit record).
- Noted that disposition document authoring is a manual step.

The new audit-record line reads: "**Audit record:** none (manual step; `anvil gate check-plan` is a verification-only command)."

---

### F7 (Low/Medium) — R3 briefing round history misdescribed R1 as a clean pass

**Resolution: Applied.**

`REVIEW_P11_DOGFOODING_R3.md` prior rounds line updated from:

> Prior rounds: R1 (2026-05-27, clean pass), R1 second pass (2026-05-27, 8 findings, all applied) ...

to:

> Prior rounds: R1 first-pass (2026-05-27, first reviewer, clean pass); R1 second-pass (2026-05-27, second reviewer, 8 findings, all applied) ...

This distinguishes the two reviewer passes that both reviewed the R1 state.

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
| AC2 (plan-level) | Dogfooding cycle via v1 CLI | **PASS (attested)** — representative artifacts + Coordinator attestation in `docs/examples/coordinator-attestation.md`; live evidence committed before public ship |
| AC3 (plan-level) | External pilot via v1 CLI with multi-reviewer rotation | **PASS (attested)** — same attestation; Leaflog representative artifacts in `docs/examples/external-pilot/` |

---

## Files Changed Since R3

| File | Change |
|---|---|
| `Anvil Plan/ANVIL_PLAN.md` | Hinge table: renamed test + new P11 entry; "canonical list" claim corrected; proc-macro removed; smoke-test commands fixed; AC #11 updated; 4 `test_workspace_lock_enforced` → `test_workspace_runtime_dir_in_layout` |
| `Anvil Plan/new_project_charter.md` | Audit-store record types: 16 → 15; 3 deferred types named |
| `Anvil Plan/CHARTER_HARDENING_HISTORY.md` | A1 amendment note corrected to reflect 15-type v1 implementation |
| `Anvil Plan/PLAN_HARDENING_HISTORY.md` | Amendments 12 and 13 added |
| `GOVERNANCE.md` | `EmergencyFreezeDeclaration` noted as v1.1; v1 interim recording documented |
| `crates/anvil-cli/src/p11.rs` | `test_contract_doc_sync_method`: tautological assert replaced with `include_str!` maintenance-note check |
| `docs/runbook.md` | Gate 4: false `GateApproval` claim removed; command description corrected |
| `docs/examples/dogfooding/README.md` | Stale "remains Provisional" PL language → "confirmed Final at P11" |
| `docs/examples/coordinator-attestation.md` | New — Coordinator attestation for AC2/AC3 |
| `Review Rounds/REVIEW_P11_DOGFOODING_R3.md` | Prior rounds line clarified |
| `Review Rounds/REVIEW_P11_DOGFOODING_R3_Findings.md` | Added (reviewer's R3 findings document) |

**Commit:** `e51c169` — "P11 R3 findings: hinge registry, record-type reconciliation, smoke-test commands, runbook Gate 4, tautological hinge, attestation doc (R3_Findings approved)"
