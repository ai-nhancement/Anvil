# P11 Dogfooding and Documentation — Review Briefing (R14)

**Date:** 2026-05-27  
**Scope:** P11 R13 finding responses — all 5 applied via doc/code changes  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (clean); R1 second-pass (8 findings); R2 (5); R3 (7); R4 (5); R5 (7); R6 (5); R7 (6); R8 (4); R9 (7); R10 (4); R11 (7); R12 (4); R13 (5). All findings addressed across all rounds.

---

## R13 Finding Responses

### F1 (High/Medium) — Leaflog described as "changelog/release-notes CLI" in Open Items but artifacts define it as a houseplant watering journal

**Disposition: Applied.**

`ANVIL_PLAN.md` Open Items, external pilot entry:

> "Leaflog (a structured changelog and release-notes CLI) selected as..."

→

> "Leaflog (a houseplant watering journal CLI) selected as..."

The corrected description matches the actual artifact content in `docs/examples/external-pilot/` (charter, plan, README, audit-store summary) which consistently define Leaflog as a houseplant-care tool — unrelated to developer productivity or workflow tooling, satisfying the domain-unrelated rubric.

---

### F2 (Medium) — Release-time smoke-test Open Item scope diverged from Distribution smoke-test definition

**Disposition: Applied.**

The previous Open Item listed `anvil phase build`, `anvil phase ship`, and `anvil ship` (heavy workflow commands requiring project state), omitted `anvil-sidecar --version`, and used "primary `anvil` binary commands" — all inconsistent with the Distribution smoke-test scope.

New text:

> **Release-time smoke-test script (Gate 2 AC4).** A smoke-test script must be written at release time and must pass against the release candidate before Gate 2 AC4 is satisfied. Script scope matches the Distribution smoke-test command list: extract archive, `anvil --version`, `anvil-sidecar --version`, `anvil init <tmp-dir>`, `anvil hinge list --count --project <tmp-dir>`, verify `anvil.toml` created by init. Not a P11 code deliverable. Status: deferred to Gate 2 / release engineering.

The scope now aligns with the locked Distribution section (line 1018) and includes both binaries.

---

### F3 (Medium) — Remaining "v1 usage" / "P11 observational data" phrases outside the R12/R13 fixed locations

**Disposition: Applied.**

Five locations updated:

| File | Location | Before | After |
|---|---|---|---|
| `ANVIL_PLAN.md` | Line 55 (Exec Summary) | `will be informed by usage feedback from v1` | `will be informed by Gate 2 live usage feedback` |
| `ANVIL_PLAN.md` | Line 889 (Cost controls) | `based on P11 + pilot usage` | `based on Gate 2 live dogfooding and pilot usage` |
| `ANVIL_PLAN.md` | Line 995 (Partial-output risk) | `if P11 observational data shows` | `if Gate 2 live observational data shows` |
| `ANVIL_PLAN.md` | Line 1110 (Seed 6 tagline) | `how v1 usage shapes the product story` | `how Gate 2 live usage shapes the product story` |
| `crates/anvil-core/src/choices.rs` | Line 181 (`cli_setup_wizard_step_ordering` revision trigger) | `validate against v1 usage feedback` | `validate against Gate 2 live usage feedback` |

---

### F4 (Low/Medium) — Build Stage outcome table still used unqualified "Ships"

**Disposition: Applied.**

`docs/examples/external-pilot/README.md` Build Stage table:

All four Outcome cells changed from `Ships` → `Would ship` to match the corrected Ship section bullets and JSON `status` fields.

---

### F5 (Low) — Old smoke-test history entry contradicted corrected Distribution scope

**Disposition: Applied.**

`Anvil Plan/PLAN_HARDENING_HISTORY.md` line 248 now has an inline clarification note appended:

> *(Later corrected: `setup --headless` and `charter render` removed; smoke-test script reclassified as a release-time deliverable, not a P11 code deliverable — see Amendment 8 smoke-test command corrections and the current Distribution Open Item.)*

The historical record is preserved; the note directs readers to the authoritative correction.

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

## Files Changed Since R13

| File | Change |
|---|---|
| `Anvil Plan/ANVIL_PLAN.md` | Leaflog description corrected to "houseplant watering journal CLI"; smoke-test Open Item rewritten to match Distribution scope + include `anvil-sidecar --version`; 4 v1-usage/P11-observational qualifications |
| `crates/anvil-core/src/choices.rs` | `cli_setup_wizard_step_ordering` revision trigger: "v1 usage feedback" → "Gate 2 live usage feedback" |
| `docs/examples/external-pilot/README.md` | Build Stage Outcome cells: "Ships" → "Would ship" |
| `Anvil Plan/PLAN_HARDENING_HISTORY.md` | Old smoke-test entry annotated with inline correction pointer |

**Commit:** `4771620` — "P11 R13: Leaflog description, smoke-test scope, v1-usage qualifiers, README table, history note (R13_Findings)"
