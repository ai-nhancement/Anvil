# P11 Dogfooding and Documentation — Review Briefing (R8)

**Date:** 2026-05-27  
**Scope:** Full P11 R7 finding responses — all 6 findings addressed  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (first reviewer, clean pass); R1 second-pass (second reviewer, 8 findings); R2 (5 findings); R3 (7 findings); R4 (5 findings); R5 (7 findings); R6 (5 findings); R7 (6 findings). All findings addressed across all rounds.

Finding dispositions this round: 5 applied as code or doc changes; 1 (F6) addressed by correcting briefing language — the R7 briefing's "all findings applied" wording was imprecise, not a code defect.

---

## R7 Finding Responses

### F1 (High) — Plan says "satisfied / ready to ship" while two ACs are deferred

**Resolution: Applied.**

`ANVIL_PLAN.md` §Plan-Level Acceptance Criteria rewritten from a single "The Plan is satisfied — and Anvil v1 is ready to ship — when:" list into two named gates:

**Gate 1 — Implementation Build Complete (P11 Accepted).** Ten criteria, all currently satisfied:
1. All 15 phases shipped.
2. All Provisional Locks resolved.
3. Layer-1 metrics collected automatically.
4. Layer-2 thresholds provisionally confirmed from v1 build data (live baselines deferred to Gate 2).
5–10. Cross-reference integrity, convergence-declaration log, plan review, hinge tests, binary smoke test, representative artifacts with attestation.

**Status: Complete.**

**Gate 2 — Public Ship (Repository Public / Public Announcement).** Two criteria, both deferred:
1. Live dogfooding: Charter and Plan for Anvil v1.1 produced via v1 CLI against real AI providers, actual audit-store records preserved.
2. Live external pilot: full Charter → Plan → Build → Ship cycle with multi-reviewer rotation, actual audit-store records preserved.

**Status: Deferred (attested) — see `docs/examples/coordinator-attestation.md`.**

The Bottom Line was updated to use Gate 1/Gate 2 language consistently. The logical contradiction — "Plan is satisfied / ready to ship" simultaneously listing deferred criteria — is eliminated.

---

### F2 (High) — Metrics and risks claimed observational dogfooding/pilot data exists

**Resolution: Applied.**

Four locations updated:

1. **Section header** (line 954): `(Layer 2 — confirmed at P11)` → `(Layer 2 — provisionally confirmed at P11)`

2. **Introductory paragraph** (line 956): Was "P11 dogfooding and the external pilot (Leaflog) produced the first observational baselines. Based on that data, all thresholds are confirmed as stated." Rewritten to: "The baselines below are derived from v1 build data — observed metrics across the P0–P11 build phases. Live P11 dogfooding and pilot runs against real AI providers (Gate 2, deferred) will provide the first external-project observational baselines and may warrant threshold revisions; on v1 build data, all thresholds are provisionally confirmed as stated." The individual table rows already read "v1 build: …" so they were already correct; the intro was the inaccurate claim.

3. **Dogfooding loop risk** (line 975): "The external pilot fills the gap: it exercises Build → Ship on a real project" → "The external pilot is the required pre-public validation that fills this gap by exercising Build → Ship on a real project. If the live pilot surfaces workflow gaps, they must be addressed before Gate 2 is satisfied." Past-complete framing replaced with required-future framing.

4. **CLI usability risk** (line 989): "The external pilot in P11 validates that the CLI is usable on a real project by a real user" → "The external pilot in P11 will validate CLI usability on a real project by a real user — deferred to before public ship (Gate 2)."

5. **Performance characterization open item** (line 1009): "Baseline performance data will come from P11 dogfooding and pilot. Status: open, to be characterized after P11" → "Baseline performance data will come from live P11 dogfooding and pilot runs, deferred to before public ship (Gate 2). Status: open, pending live execution." ("After P11" implied the data would exist at P11 ship; it will not.)

---

### F3 (Medium/High) — Representative example READMEs narrated live CLI activity in body sections

**Resolution: Applied.**

**`docs/examples/dogfooding/README.md`:**
- Section header "## What Was Learned" → "## What a Live Session Would Reveal"
- Added framing notice at section start distinguishing real governance decisions (PL evaluations) from representative CLI interaction details.
- "These were found during the Charter → Plan cycle and were fixed before P11 shipped. The CLI handled the v1.1 design cycle without workflow-blocking failures." → "A live Charter → Plan cycle on the v1.1 design is expected to surface no workflow-blocking failures and no gaps requiring earlier phases to reopen."
- "UX friction logged for v1.1" items reframed from past-tense (found/was) to present/expected tense.
- PL evaluation section renamed to "Provisional Lock reviews triggered at the v1.1-prep boundary"; intro clarifies these are real governance decisions; "After running the setup wizard in `anvil setup` three times during dogfooding (once for this session)" replaced with "The setup wizard step ordering was evaluated against v1.1 App design requirements."

**`docs/examples/external-pilot/README.md`:**
- Added representative-flow framing notice at the start of "## Workflow Summary."
- Charter, Plan, Build Stage, and Ship sections rewritten to conditional tense: "would run `anvil discuss`," "would produce a 4-phase plan," "would go through `anvil phase build → review → ship`," "would pass all gates in a successful run." Table Outcome column: "Shipped" → "Ships."
- "Provider Diversity Stress Results" section rewritten: "behaved correctly on all rounds" → "are expected to behave correctly on all rounds in a live run"; "Provider diversity stress: **passed**" → "Provider diversity stress: **to be validated in live run.**"

---

### F4 (Medium) — PL parser scanned entire Plan; could match unrelated table rows

**Resolution: Applied.**

`test_no_outstanding_provisional_locks_after_dogfooding` in `p11.rs` now scopes extraction to the `## Locked Required Project-Level Choices` section:

```rust
let lines: Vec<&str> = plan_doc.lines().collect();
let section_start = lines
    .iter()
    .position(|line| line.starts_with("## Locked Required Project-Level Choices"))
    .expect(
        "Section '## Locked Required Project-Level Choices' not found in ANVIL_PLAN.md; \
         check section header",
    );
let section_end = lines[section_start + 1..]
    .iter()
    .position(|line| line.starts_with("## "))
    .map_or(lines.len(), |rel| section_start + 1 + rel);
```

The extraction then operates on `lines[section_start..section_end]`. The `.expect()` provides a clear error message if the section header is ever renamed. The `map_or()` form is used (replacing `map().unwrap_or()`) to satisfy clippy. The `.copied()` call on the slice iterator keeps the element type as `&str` matching the original `plan_doc.lines()` behavior.

---

### F5 (Medium) — Contract doc check implied material drift protection

**Resolution: Applied (comment update).**

The comment in `test_contract_doc_sync_method` renamed from "RPC-name coverage check" to "RPC-name presence smoke test":

```rust
// RPC-name presence smoke test: verifies every RPC name in the proto appears as a
// substring in the contract doc. Does NOT check service name, request/response
// types, message fields, field numbers, oneof variants, enum values, or package.
// Full schema-level CI enforcement is explicitly a v1.1 task.
```

The AC table below reflects the same language. The underlying limitation (substring-only, no schema validation) is unchanged and explicitly documented as a v1.1 task.

---

### F6 (Low/Medium) — R7 "all findings applied" imprecise because one R6 finding was refuted

**Resolution: Addressed in briefing language.**

R7's "all findings applied" was imprecise — R7 F2 was a factual correction (no code change), not an applied patch. This briefing uses precise dispositions: "all findings addressed" with this note on the header line and specific resolution labels per finding (Applied / Applied (comment update) / Addressed in briefing language). Future briefings will use this convention.

---

## Acceptance Criteria Status

| AC | Criterion | Status |
|---|---|---|
| AC1 | `docs/runbook.md` covers all P0–P11 Coordinator workflows | **PASS** |
| AC2 | `docs/onboarding.md` covers contributor setup and first commands | **PASS** |
| AC3 | `docs/contract.md` documents the sidecar gRPC contract | **PASS** |
| AC4 | All 8 Provisional Locks confirmed Final | **PASS** |
| AC5 | Hinge test: section-scoped extraction + count + forward + reverse check between hard-coded slug list and Plan Required Choices table | **PASS** |
| AC6 | Publication-safety gate documented; execution deferred to actual public flip | **PASS** |
| AC7 | `docs/ux-audit.md` covers CLI→App friction and v1.1 recommendations | **PASS** |
| AC2 (plan-level / Gate 2) | Dogfooding cycle via v1 CLI — live execution against real AI providers | **Deferred (attested)** |
| AC3 (plan-level / Gate 2) | External pilot via v1 CLI with multi-reviewer rotation — live execution | **Deferred (attested)** |

---

## Files Changed Since R7

| File | Change |
|---|---|
| `crates/anvil-cli/src/p11.rs` | PL parser scoped to Required Choices section; `map_or` fix; contract test comment renamed to "RPC-name presence smoke test" |
| `Anvil Plan/ANVIL_PLAN.md` | Plan-Level Acceptance Criteria split into Gate 1/Gate 2; metric intro updated to v1 build data; risks updated to future tense; performance open item status updated; Bottom Line updated |
| `docs/examples/dogfooding/README.md` | Section header and body rewritten to representative/expected language |
| `docs/examples/external-pilot/README.md` | Workflow Summary framing notice; Charter/Plan/Build/Ship/Provider sections rewritten to conditional tense |
| `Review Rounds/REVIEW_P11_DOGFOODING_R7_Findings.md` | Added (reviewer's R7 findings document) |

**Commit:** `c4275c4` — "P11 R7 findings: two-gate acceptance, deferred-evidence corrections, representative README rewrites, section-scoped PL parser, smoke-test rename (R7_Findings approved)"
