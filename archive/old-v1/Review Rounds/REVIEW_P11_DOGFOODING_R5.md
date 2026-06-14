# P11 Dogfooding and Documentation — Review Briefing (R5)

**Date:** 2026-05-27  
**Scope:** Full P11 R4 finding responses — all 5 findings addressed  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (2026-05-27, first reviewer, clean pass); R1 second-pass (2026-05-27, second reviewer, 8 findings, all applied); R2 (2026-05-27, 5 findings, all applied); R3 (2026-05-27, 7 findings, all applied); R4 (2026-05-27, 5 findings, all applied).

---

## R4 Finding Responses

### F1 (Critical) — `test_contract_doc_sync_method` was a weak string-presence check

**Resolution: Applied.**

The `include_str!` + `contains("Automated drift detection is a v1.1 task")` check was replaced with a real drift detection test:

```rust
let proto = std::fs::read_to_string(
    workspace_root.join("proto").join("anvil").join("v1").join("sidecar.proto"),
)
.expect("proto/anvil/v1/sidecar.proto not found");

// All RPC names from the proto must appear in the contract doc.
let rpc_names: Vec<&str> = proto.lines()
    .filter_map(|line| {
        let t = line.trim();
        t.strip_prefix("rpc ")
            .and_then(|r| r.split('(').next())
            .map(str::trim)
    })
    .filter(|s| !s.is_empty())
    .collect();

for rpc in &rpc_names {
    assert!(contract_doc.contains(rpc), "docs/contract.md is missing proto RPC '{rpc}'");
}
```

This test will now fail if any RPC defined in `proto/anvil/v1/sidecar.proto` is absent from `docs/contract.md`. Currently the proto defines 6 RPCs: `Handshake`, `Invoke`, `InvokeStreaming`, `Cancel`, `Health`, `ReloadConfig` — all verified present. If the proto adds a new RPC without updating the contract doc, the test fails. The maintenance note assertion is retained alongside.

---

### F2 (High) — AC table "PASS (attested)" misrepresented plan-level AC2/AC3 status

**Resolution: Applied.**

`REVIEW_P11_DOGFOODING_R4.md` AC table rows for plan-level AC2 and AC3 updated from "PASS (attested)" to:

> **Deferred (attested)** — live evidence required before public ship; Coordinator attestation in `docs/examples/coordinator-attestation.md` documents what was validated and the commitment.

The label now accurately reflects the state: the substantive requirement (actual CLI cycles against real AI providers) was not performed and is deferred, with a Coordinator attestation recording the constraint and the commitment.

---

### F3 (High) — PL hinge test had no runtime verification against the Plan table

**Resolution: Applied.**

`test_no_outstanding_provisional_locks_after_dogfooding` now parses the live `ANVIL_PLAN.md` Required Choices table at test time and verifies the hard-coded slug list matches exactly:

```rust
let plan_slugs: Vec<String> = plan_doc
    .lines()
    .filter(|line| line.contains("**Final (P11)**"))
    .filter_map(|line| {
        let cols: Vec<&str> = line.split('|').collect();
        cols.get(1).and_then(|col| {
            let mut parts = col.split('`');
            parts.next();
            parts.next().map(std::string::ToString::to_string)
        })
    })
    .collect();

assert_eq!(plan_slugs.len(), confirmed_final.len(), ...);
for slug in confirmed_final {
    assert!(plan_slugs.iter().any(|s| s == slug), ...);
}
```

The test now fails in three additional cases: (1) a PL is added to the Plan table without updating the code list; (2) a PL is removed from the Plan table without updating the code list; (3) a PL slug is renamed in the Plan table without updating the code list. The hard-coded list is retained as a "deliberate edit" trigger; the runtime check makes it authoritative rather than aspirational. AC5 now has real teeth.

---

### F4 (Medium) — `include_str!` path was relative to the source file location

**Resolution: Applied (F1 and F4 addressed together).**

Both tests now use `env!("CARGO_MANIFEST_DIR")` to locate the workspace root, then construct paths with `std::path::Path::join`:

```rust
let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent().unwrap()  // crates/
    .parent().unwrap(); // workspace root
```

`CARGO_MANIFEST_DIR` resolves to the crate directory (`crates/anvil-cli/`) at compile time and is stable regardless of where in the source tree `p11.rs` lives. The tests use `std::fs::read_to_string` at runtime rather than `include_str!` at compile time — the failure mode is a clear `expect` panic with a descriptive message rather than a compile error.

---

### F5 (Medium) — Remaining "16 record types" reference in `ANVIL_PLAN.md` Amendment A1 section

**Resolution: Applied.**

`ANVIL_PLAN.md` line 18 (Amendment A1 downstream impact on P2) updated: the "Total v1 record types: **16**" claim is corrected to note that all three A1-contemplated record types were deferred to v1.1, that `CuratedFindings` and `PlanConsolidation` were added during Build, and that the actual v1 total is **15**.

The historical documents `CHARTER_AMENDMENT_A1.md` and `AMENDMENT_A1_HARDENING_HISTORY.md` are not changed — they accurately record the original A1 intent and are a legislative record, not normative specification.

---

## Acceptance Criteria Status

| AC | Criterion | Status |
|---|---|---|
| AC1 | `docs/runbook.md` covers all P0–P11 Coordinator workflows | **PASS** |
| AC2 | `docs/onboarding.md` covers contributor setup and first commands | **PASS** |
| AC3 | `docs/contract.md` documents the sidecar gRPC contract | **PASS** |
| AC4 | All 8 Provisional Locks confirmed Final | **PASS** |
| AC5 | Hinge test asserts PL count and slugs match Required Choices table | **PASS** (runtime Plan-table parser now enforces this) |
| AC6 | Publication-safety gate documented; execution deferred to actual public flip | **PASS** |
| AC7 | `docs/ux-audit.md` covers CLI→App friction and v1.1 recommendations | **PASS** |
| AC2 (plan-level) | Dogfooding cycle via v1 CLI | **Deferred (attested)** — live evidence required before public ship |
| AC3 (plan-level) | External pilot via v1 CLI with multi-reviewer rotation | **Deferred (attested)** — live evidence required before public ship |

---

## Files Changed Since R4

| File | Change |
|---|---|
| `crates/anvil-cli/src/p11.rs` | Both hinge tests rewritten: `env!("CARGO_MANIFEST_DIR")` paths; proto RPC-name drift check; Plan-table PL slug runtime verification |
| `Review Rounds/REVIEW_P11_DOGFOODING_R4.md` | Plan-level AC2/AC3 label corrected to "Deferred (attested)" |
| `Anvil Plan/ANVIL_PLAN.md` | P2 Amendment A1 section: "Total v1 record types: **16**" corrected to **15** with deferral note |
| `Review Rounds/REVIEW_P11_DOGFOODING_R4_Findings.md` | Added (reviewer's R4 findings document) |

**Commit:** `93a97a5` — "P11 R4 findings: runtime Plan-table PL verification, proto RPC drift check, env CARGO_MANIFEST_DIR paths, AC label correction, record-type count cleanup (R4_Findings approved)"
