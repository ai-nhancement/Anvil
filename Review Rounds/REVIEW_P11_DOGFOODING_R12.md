# P11 Dogfooding and Documentation — Review Briefing (R12)

**Date:** 2026-05-27  
**Scope:** P11 R11 finding responses — 6 applied via doc changes; 1 addressed via briefing-language confirmation only  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (clean); R1 second-pass (8 findings); R2 (5); R3 (7); R4 (5); R5 (7); R6 (5); R7 (6); R8 (4); R9 (7); R10 (4); R11 (7). All findings addressed across all rounds.

---

## R11 Finding Responses

### F1 (High) — P11 Gate 2 and Plan-Level Gate 2 still diverged despite R10 claiming they were identical

**Disposition: Applied.**

P11 Gate 2 (`ANVIL_PLAN.md` lines 833–837) previously had:
- Multi-reviewer rotation as a standalone item (item 3)
- v1.1 Plan validation as item 4
- No release archive / signed checksum / smoke-test criterion

Plan-Level Gate 2 had multi-reviewer rotation folded into the external pilot item, v1.1 Plan validation as item 3, and release archive as item 4.

Both lists now use identical item text. P11 Gate 2 now reads:

1. The dogfooding test in P11 has produced a Charter and Plan for Anvil v1.1 using the v1 CLI alone (live execution against real AI providers; actual audit-store records preserved). *(Deferred with attestation — see `docs/examples/coordinator-attestation.md`)*
2. At least one external, non-self-referential project has completed a full Charter → Plan → Build → Ship cycle using the v1 CLI alone, including at least one Build phase with multi-reviewer rotation (live execution; actual audit-store records preserved). *(Deferred with attestation — see `docs/examples/coordinator-attestation.md`)*
3. The v1.1 Plan from live dogfooding validated as the input for the v1.1 App design. *(Deferred with attestation — see `docs/examples/coordinator-attestation.md`)*
4. Release archive produced for the primary platform (Windows x64) with `SHA256SUMS.txt` + signed `SHA256SUMS.txt.asc`; smoke-test script (written at release time, not a P11 deliverable) passes against the release candidate. *(Deferred — release-time)*

Plan-Level Gate 2 items 1–4 are textually identical (without the deferred-status annotations, which belong in the per-phase section).

---

### F2 (High) — Gate 2 status "Deferred (attested)" for all 4 items, but attestation only covered dogfooding/pilot

**Disposition: Applied.**

Two changes:

**Plan-Level Gate 2 status** split by item:

> **Status:** AC1–AC3: Deferred (attested) — see `docs/examples/coordinator-attestation.md`. AC4: Deferred (release-time) — not covered by the dogfooding attestation.

**`docs/examples/coordinator-attestation.md`** updated:

- Scope header: `Plan-Level Acceptance Criteria 2 and 3 (dogfooding + external pilot)` → `Gate 2 AC1–AC3 (dogfooding, external pilot, and v1.1 Plan validation)`
- "What This Document Is" now references Gate 2 AC1 / AC2 / AC3 by name (including the newly covered AC3 v1.1 Plan validation)
- "What Remains" now enumerates all three Gate 2 items with explicit Coordinator commitments:
  - AC1: live dogfooding cycle producing Charter and Plan for v1.1
  - AC2: live external pilot, full cycle, multi-reviewer rotation, audit-store records
  - AC3: v1.1 Plan from live dogfooding validated as App design input, validation recorded
- Sign-off item 4 updated to cover AC1, AC2, and AC3 by designation

---

### F3 (Medium/High) — Live-dogfooding wording remained in child artifacts and Plan prose

**Disposition: Applied.**

Four locations updated:

1. `docs/examples/dogfooding/v11-charter.md` lines 3–4:
   - `Version: R1 (converged via dogfooding session, 2026-05-26)` → `Version: R1 (representative final/converged form — not a live \`anvil discuss\` / \`anvil charter review\` output)`
   - `Produced using: Anvil v1.0.0 CLI (anvil discuss + anvil charter review)` → `Produced using: Representative of output expected from Anvil v1.0.0 CLI (anvil discuss + anvil charter review)`

2. `ANVIL_PLAN.md:989` Cross-Cutting Concerns v1.1-prep risk:
   - `both confirmed Final at P11 after dogfooding and UX audit` → `both confirmed Final at P11 after UX audit and build observations`

3. `PLAN_HARDENING_HISTORY.md:533` Amendment 8 documentation list:
   - `v1.1 charter, plan phase summary, and dogfooding session notes` → `v1.1 charter, plan phase summary, and representative dogfooding artifacts`

Note: `PLAN_HARDENING_HISTORY.md:606` uses `"dogfooding acceptance test"` in the rationale for why live evidence is unavailable — this is accurate, explanatory language, not a claim that dogfooding occurred, and was not changed.

---

### F4 (Medium) — P11 action list and Open Items still implied external pilot was chosen and run

**Disposition: Applied.**

Three locations updated:

1. `ANVIL_PLAN.md` P11 goal line: added `*Gate 1 (complete): representative dogfooding and pilot artifacts, documentation deliverables, and Coordinator attestation. Gate 2 (deferred): live CLI execution against real AI providers before public ship.*`

2. P11 action list dogfooding and external pilot bullets: rewritten to explicitly state Gate 1 (representative artifacts in `docs/examples/`) as complete and Gate 2 (live execution) as deferred. The external pilot bullet names Leaflog as the representative scenario and notes the live execution rubric applies to the Gate 2 run.

3. `ANVIL_PLAN.md` Open Items, external pilot entry:
   - Previous: `chosen during P11 … Status: open, resolved in P11`
   - Now: `Leaflog selected as the representative pilot project and documented in docs/examples/external-pilot/. Live execution of the full cycle is deferred to Gate 2. Status: Leaflog selected (Gate 1 complete); live execution deferred to Gate 2.`

4. `PLAN_CONVERGENCE.md:51`:
   - `external pilot project selection resolved in P11` → `external pilot project selected in P11 (Leaflog, representative Gate 1 artifacts; live execution deferred to Gate 2)`

---

### F5 (Medium) — v1.1 evidence language implied v1 usage data existed before Gate 2

**Disposition: Applied.**

Five locations updated in `ANVIL_PLAN.md`:

| Location | Before | After |
|---|---|---|
| Line 57 (Why CLI-first) | `designed against this evidence` | `designed against Gate 2 live usage evidence` |
| Line 59 (Arch commitment) | `based on real CLI usage data` | `based on Gate 2 live CLI usage data` |
| Line 1187 (v1→v1.1 Transition) | `after v1 usage produces design evidence` | `after Gate 2 live usage produces design evidence` |
| Line 1191 (v1→v1.1 Transition) | `evaluated against v1 usage feedback` | `evaluated against build observations` |
| Line 1101 (Seed 5 cost limits) | `Data from P11 and pilot usage` | `Data from Gate 2 live dogfooding and pilot usage` |

Bottom Line line 1204:
- `v1 proves the discipline; v1.1 broadens the audience` → `v1 builds and tests the discipline; Gate 2 live usage will prove it; v1.1 broadens the audience`

---

### F6 (Low/Medium) — Representative JSON phase_outcomes still used unqualified `shipped: true`

**Disposition: Applied.**

`docs/examples/external-pilot/audit-store-summary.EXAMPLE.json`:
- All four phase_outcomes entries: `"shipped": true` → `"status": "representative_shipped_shape"`

`docs/examples/external-pilot/README.md` Ship section:
- `All 4 phases shipped` → `All 4 phases would ship (representative_shipped_shape)`
- `Audit integrity: pass` → `Audit integrity: representative_pass_shape`

---

### F7 (Low) — Contract smoke test intentionally substring-only; acceptable for v1

**Disposition: No change.** The AC3 briefing entry continues to label this a substring smoke test only. Structured proto-vs-doc validation remains a tracked v1.1 task. Implementation unchanged.

---

## Acceptance Criteria Status

| AC | Criterion | Status |
|---|---|---|
| AC1 | `docs/runbook.md` covers all P0–P11 Coordinator workflows | **PASS** |
| AC2 | `docs/onboarding.md` covers contributor setup and first commands | **PASS** |
| AC3 | `docs/contract.md` contains every service and RPC name from `sidecar.proto` (substring smoke test only; full schema sync is v1.1) | **PASS** |
| AC4 | All 8 Provisional Locks confirmed Final (Gate 1) | **PASS** |
| AC5 | Hinge test: section-scoped, trim-normalized (header + slug), count + forward + reverse bidirectional slug check | **PASS** |
| AC6 | Publication-safety gate documented; execution deferred to actual public flip | **PASS** |
| AC7 | `docs/ux-audit.md` covers CLI→App friction and v1.1 recommendations | **PASS** |
| Gate 2 AC1 | Live dogfooding via v1 CLI against real AI providers | **Deferred (attested)** |
| Gate 2 AC2 | Live external pilot: full cycle with multi-reviewer rotation | **Deferred (attested)** |
| Gate 2 AC3 | v1.1 Plan from live dogfooding validated as v1.1 App design input | **Deferred (attested)** |
| Gate 2 AC4 | Release archive, signed checksum, smoke-test script passes | **Deferred (release-time)** |

---

## Files Changed Since R11

| File | Change |
|---|---|
| `Anvil Plan/ANVIL_PLAN.md` | P11 Gate 2 list rewritten to match Plan-Level Gate 2 exactly (items 1–4 textually identical); P11 goal and action bullets distinguish Gate 1/Gate 2; Open Items external pilot entry updated to Leaflog + Gate 2 deferral; 6 v1.1 evidence language qualifications (Gate 2 live usage); Bottom Line "v1 proves" narrowed |
| `Anvil Plan/PLAN_HARDENING_HISTORY.md` | Amendment 8 doc list: "dogfooding session notes" → "representative dogfooding artifacts" |
| `Anvil Plan/PLAN_CONVERGENCE.md` | Outstanding items: external pilot selection now says Leaflog selected, live execution deferred to Gate 2 |
| `docs/examples/coordinator-attestation.md` | Scope header updated to Gate 2 AC1–AC3; "What This Document Is" references AC1/AC2/AC3 by name; "What Remains" enumerates explicit commitments for all three items; sign-off item 4 updated |
| `docs/examples/dogfooding/v11-charter.md` | Version and Produced-using lines: "dogfooding session" → representative disclaimer |
| `docs/examples/external-pilot/audit-store-summary.EXAMPLE.json` | phase_outcomes: `"shipped": true` → `"status": "representative_shipped_shape"` on all 4 entries |
| `docs/examples/external-pilot/README.md` | Ship section bullets: "All 4 phases shipped" / "Audit integrity: pass" → representative language |

**Commit:** `2edafab` — "P11 R11: Gate 2 list sync, attestation scope, live-dogfooding wording, representative JSON fields (R11_Findings)"
