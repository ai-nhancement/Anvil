# P11 Dogfooding and Documentation — Review Briefing (R9)

**Date:** 2026-05-27  
**Scope:** P11 R8 finding responses — 3 applied via code/doc changes; 1 addressed via briefing-language correction only  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (clean); R1 second-pass (8 findings); R2 (5); R3 (7); R4 (5); R5 (7); R6 (5); R7 (6); R8 (4). All findings addressed across all rounds.

---

## R8 Finding Responses

### F1 (High) — Gate 1 "Status: Complete" claimed dogfooding AC satisfied via representative artifacts

**Disposition: Applied.**

Gate 1 criterion 10 rewritten from "Representative dogfooding and external pilot artifacts provided with Coordinator attestation" to:

> Dogfooding and external pilot documentation complete: representative artifacts in `docs/examples/dogfooding/` and `docs/examples/external-pilot/`, with formal Coordinator attestation in `docs/examples/coordinator-attestation.md`. This criterion satisfies the P11 documentation deliverable; live CLI execution against real AI providers remains a Gate 2 requirement and is not claimed here.

Gate 1 "Status: Complete" now refers unambiguously to the documentation deliverable only. Gate 2 retains the live-execution requirement without being contradicted by the Gate 1 label.

---

### F2 (Medium) — Section-scoped PL parser used `starts_with` without whitespace normalization

**Disposition: Applied.**

Both boundary detections updated to use `.trim()`:

- Section start: `line.starts_with("## Locked Required Project-Level Choices")` → `line.trim() == "## Locked Required Project-Level Choices"` (exact match after trimming; handles trailing whitespace and is more precise than prefix match).
- Section end: `line.starts_with("## ")` → `line.trim().starts_with("## ")` (consistent normalization for the next-header boundary).

The `.expect()` message remains unchanged — it still fires if the header is absent, regardless of formatting.

---

### F3 (Medium) — Contract test had no service-name check; AC implied more coverage than provided

**Disposition: Applied.**

`test_contract_doc_sync_method` now performs a two-part smoke test:

1. **Service-name check** (new): extracts all `service Foo {` declarations from the proto and asserts each service name appears in `docs/contract.md`.
2. **RPC-name check** (existing): extracts all `rpc Bar(` names and asserts each appears in `docs/contract.md`.

```rust
let service_names: Vec<&str> = proto
    .lines()
    .filter_map(|line| {
        let t = line.trim();
        if t.starts_with("service ") {
            t.strip_prefix("service ")
                .and_then(|r| r.split_whitespace().next())
        } else {
            None
        }
    })
    .collect();

assert!(!service_names.is_empty(), "sidecar.proto defines no services ...");
for service in &service_names { assert!(contract_doc.contains(service), ...); }
```

The comment updated to "Smoke test: verifies that (1) every service name and (2) every RPC name from the proto appear as substrings in the contract doc." The AC table below reflects this accurately.

---

### F4 (Low) — R8 briefing header still used "all findings addressed" imprecise phrasing

**Disposition: Addressed in briefing language.**

This R9 briefing uses precise per-finding language throughout. The header line now states "3 applied via code/doc changes; 1 addressed via briefing-language correction only" so the summary itself cannot be misread. Future briefings will maintain this convention.

---

## Acceptance Criteria Status

| AC | Criterion | Status |
|---|---|---|
| AC1 | `docs/runbook.md` covers all P0–P11 Coordinator workflows | **PASS** |
| AC2 | `docs/onboarding.md` covers contributor setup and first commands | **PASS** |
| AC3 | `docs/contract.md` documents the sidecar gRPC contract; hinge test smoke-checks service name and all RPC names against the proto | **PASS (smoke test; full schema sync is v1.1)** |
| AC4 | All 8 Provisional Locks confirmed Final | **PASS** |
| AC5 | Hinge test: section-scoped (trim-normalized boundary detection), count + forward + reverse slug check between hard-coded list and Plan Required Choices table | **PASS** |
| AC6 | Publication-safety gate documented; execution deferred to actual public flip | **PASS** |
| AC7 | `docs/ux-audit.md` covers CLI→App friction and v1.1 recommendations | **PASS** |
| AC2 (plan-level / Gate 2) | Dogfooding cycle via v1 CLI — live execution against real AI providers | **Deferred (attested)** |
| AC3 (plan-level / Gate 2) | External pilot via v1 CLI with multi-reviewer rotation — live execution | **Deferred (attested)** |

---

## Files Changed Since R8

| File | Change |
|---|---|
| `crates/anvil-cli/src/p11.rs` | Trim-normalized section-boundary detection; service-name smoke test added to contract test; comment updated |
| `Anvil Plan/ANVIL_PLAN.md` | Gate 1 criterion 10 relabeled as documentation deliverable |
| `Review Rounds/REVIEW_P11_DOGFOODING_R8_Findings.md` | Added (reviewer's R8 findings document) |

**Commit:** `8da98ac` — "P11 R8 findings: Gate 1 doc criterion, trim-based header matching, service-name smoke test (R8_Findings approved)"
