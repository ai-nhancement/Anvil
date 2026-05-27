# P11 Dogfooding and Documentation — Review Briefing (R7)

**Date:** 2026-05-27  
**Scope:** Full P11 R6 finding responses — all 5 findings addressed  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (first reviewer, clean pass); R1 second-pass (second reviewer, 8 findings); R2 (5 findings); R3 (7 findings); R4 (5 findings); R5 (7 findings); R6 (5 findings). All findings applied across all rounds.

---

## R6 Finding Responses

### F1 (High) — PL parser brittle against Markdown bold-marker variations

**Resolution: Applied.**

The `filter` step in `test_no_outstanding_provisional_locks_after_dogfooding` now strips `**` before testing for `"Final (P11)"`:

```rust
.filter(|line| line.replace("**", "").contains("Final (P11)"))
```

This makes the extraction tolerant of minor Markdown formatting variations (e.g., `**Final** (P11)` vs `**Final (P11)**`) without false positives. A comment explains the rationale. The backtick-based slug extraction and column split are unchanged.

---

### F2 (High) — Contract test described as "only performs a single-string presence check; no RPC names extracted"

**Resolution: No code change — finding is factually incorrect.**

The R6 finding states: "The actual test code remains unchanged: it only verifies that one sentence exists in the document. No RPC names are actually extracted from the proto and compared."

This characterization does not match the code. Since R4, `test_contract_doc_sync_method` has extracted all RPC names from the proto via:

```rust
let rpc_names: Vec<&str> = proto
    .lines()
    .filter_map(|line| {
        let t = line.trim();
        t.strip_prefix("rpc ")
            .and_then(|r| r.split('(').next())
            .map(str::trim)
    })
    .filter(|s| !s.is_empty())
    .collect();
```

…and asserts each name appears in `docs/contract.md`. The R6 change (F5 of R5) was a comment update only; the RPC-extraction logic was already present and has been exercised in every test run since R4.

The single maintenance-note presence check (`contains("Automated drift detection is a v1.1 task")`) is a secondary guard to prevent silent removal of the sync guidance — it is not the primary assertion.

No code change is made. The R7 briefing records the correction.

---

### F3 (Medium) — Verbose repeated deferral notes cluttering Plan ACs

**Resolution: Applied.**

Four locations in `ANVIL_PLAN.md` shortened from the verbose pattern:

> *(Deferred with Coordinator attestation; representative artifacts in `docs/examples/dogfooding/`; live evidence required before public ship.)*

to the concise cross-reference:

> *(Deferred with attestation — see `docs/examples/coordinator-attestation.md`.)*

Locations changed:
1. P11 AC1 (line 829)
2. P11 AC2 (line 830)
3. Plan-level AC2 (line 1136)
4. Plan-level AC3 (line 1137)

P11 AC3 was already short (`*(Deferred with attestation; see AC2.)*`) and updated only to align the phrasing (`— see AC2.` instead of `; see AC2.`). The `docs/examples/coordinator-attestation.md` file itself contains the full explanation (rationale, scope, Coordinator sign-off, commitment to live evidence before public ship); the Plan ACs now cross-reference it rather than repeating a subset of its content.

The inline notes in `new_project_charter.md` (lines 459, 462, 463) document different deferrals — A1 items scoped to Plan Amendment 9 — and are already concise and specific. They are left unchanged.

---

### F4 (Medium) — AC5 "match" language overstated as one-directional

**Resolution: Applied.**

`test_no_outstanding_provisional_locks_after_dogfooding` now performs an explicit three-part synchronization check:

1. **Count assertion** — `plan_slugs.len() == confirmed_final.len()`
2. **Forward check** — every slug in `confirmed_final` appears in `plan_slugs`
3. **Reverse check** (new) — every slug in `plan_slugs` appears in `confirmed_final`

```rust
// Reverse check: every Plan-table slug must appear in the hard-coded list.
// Together with the forward check and the count assertion, this is a full
// bidirectional synchronization — neither side can add a slug without the other.
for slug in &plan_slugs {
    assert!(
        confirmed_final.contains(&slug.as_str()),
        "slug '{slug}' is in ANVIL_PLAN.md Required Choices table but not in this list; \
         add it here or update the Plan table"
    );
}
```

The count assertion alone was already sufficient to catch asymmetric additions (the sets cannot have the same size if one has an extra element the other lacks), but the explicit reverse check makes the bidirectional contract visible in the code without relying on that reasoning. The AC5 wording in the table below is updated accordingly.

---

### F5 (Low) — Smoke-test expectation unclear for non-fresh directories

**Resolution: Applied.**

`ANVIL_PLAN.md` smoke-test step updated: the count-of-0 expectation now explicitly notes that the zero is expected because `<tmp-dir>` is a freshly initialized project with no annotated source, and that a non-zero count is expected and acceptable when the tool is run against a project directory that already contains hinge-annotated source files.

---

## Acceptance Criteria Status

| AC | Criterion | Status |
|---|---|---|
| AC1 | `docs/runbook.md` covers all P0–P11 Coordinator workflows | **PASS** |
| AC2 | `docs/onboarding.md` covers contributor setup and first commands | **PASS** |
| AC3 | `docs/contract.md` documents the sidecar gRPC contract | **PASS** |
| AC4 | All 8 Provisional Locks confirmed Final | **PASS** |
| AC5 | Hinge test performs full bidirectional sync: count + forward + reverse check between hard-coded slug list and Plan Required Choices table | **PASS** |
| AC6 | Publication-safety gate documented; execution deferred to actual public flip | **PASS** |
| AC7 | `docs/ux-audit.md` covers CLI→App friction and v1.1 recommendations | **PASS** |
| AC2 (plan-level) | Dogfooding cycle via v1 CLI | **Deferred (attested)** — live evidence required before public ship |
| AC3 (plan-level) | External pilot via v1 CLI with multi-reviewer rotation | **Deferred (attested)** — live evidence required before public ship |

---

## Files Changed Since R6

| File | Change |
|---|---|
| `crates/anvil-cli/src/p11.rs` | Bold-marker stripping in PL filter; explicit reverse slug check; comments updated |
| `Anvil Plan/ANVIL_PLAN.md` | P11 AC1–AC2 and Plan-level AC2–AC3 deferral notes shortened to cross-references; smoke-test count expectation clarified |
| `Review Rounds/REVIEW_P11_DOGFOODING_R6_Findings.md` | Added (reviewer's R6 findings document) |

**Commit:** `f685798` — "P11 R6 findings: robust PL parser, bidirectional slug check, concise deferral notes, smoke-test clarification (R6_Findings approved)"
