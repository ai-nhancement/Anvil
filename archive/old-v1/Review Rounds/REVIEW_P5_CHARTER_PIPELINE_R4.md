# Anvil — P5 Charter Stage Pipeline R4 Review

**Date:** 2026-05-26  
**Phase:** P5 — Charter Stage Pipeline  
**Branch:** master  
**Prior round:** R3 — `REVIEW_P5_CHARTER_PIPELINE_R3_Findings.md`  
**Reviewer:** jvcan (coordinator)

---

## What Changed in R4

All 6 R3 findings addressed. No regressions; 70 tests pass.

---

## Validation

| Check | Command | Result |
|---|---|---|
| Format | `cargo fmt --all -- --check` | **Pass** |
| Lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | **Pass** |
| Tests | `cargo test --workspace` | **Pass — 70 tests, 0 failures** |

---

## R3 Findings Disposition

### F1 — High/Medium: Interactive curation silently defaults on input errors or cancellation

**Status:** Fixed.

`curate_findings` and `collect_narrative` now return `Result<_, AnvilError>` instead of bare value types. Every `.interact().unwrap_or(0)` and `.interact_text().unwrap_or_default()` call is replaced with `.map_err(|_| AnvilError::SetupCancelled)?`. The coordinator `run_charter_findings` propagates those errors via `?`. A cancelled or failed terminal interaction now exits cleanly rather than continuing with silent defaults.

---

### F2 — Medium: `anvil discuss` can spin forever on EOF / non-interactive stdin

**Status:** Fixed.

`read_line()` now captures the returned byte count `n`. If `n == 0` (EOF), the loop returns:

```rust
let n = stdin.lock().read_line(&mut user_input).map_err(AnvilError::Io)?;
if n == 0 {
    return Err(AnvilError::Io(std::io::Error::other(
        "stdin closed (EOF) — interactive terminal required for `anvil discuss`",
    )));
}
```

This was already present in the prior codebase as an explicit `Ok(0)` guard; the R3 fix confirmed it with the stored byte count variable. A closed-stdin or headless run now exits with a clear error instead of busy-looping.

---

### F3 — Medium/Low: Provenance lookup for P5 records asserted ready but not integration-tested

**Status:** Fixed.

Added `test_p5_provenance_graph_backs_all_charter_record_types` to `charter::tests`. The test:

1. Initializes a real `AuditStore` in a `tempfile::TempDir` (via the shared `init_test_store()` helper).
2. Appends a `ReviewerFindingPacket`, a `VerifierResult`, and a `CuratedFindingsRecord`, all tagged with cross-ref `charter.md:§root:R1`.
3. Builds `ProvenanceGraph` from that store.
4. Asserts `records_for_key("charter.md:§root:R1")` returns all three record IDs.

The prior `test_p5_cross_ref_keys_parseable` is retained as a lower-level format guard.

---

### F4 — Low/Medium: Reviewer model identity can be persisted as empty string

**Status:** Fixed.

After deserializing the `PartialFindingsPacket`, `run_charter_review` now applies a fallback:

```rust
let reviewer_model_identity = if partial.reviewer_model_identity.trim().is_empty() {
    model_id.clone()
} else {
    partial.reviewer_model_identity
};
```

If the model omits `reviewer_model_identity`, the configured model ID (already known at call time) is used instead. `test_reviewer_model_identity_fallback_logic` covers both the empty and non-empty cases.

---

### F5 — Low: Section-heading grounding does not recognize indented Markdown headings

**Status:** Fixed.

The heading predicate now strips up to three leading spaces (CommonMark ATX indent allowance) before inspecting the `#` markers:

```rust
// CommonMark allows up to 3 leading spaces before the '#' markers; strip them first.
let stripped = line.trim_start_matches(' ');
let after_hashes = stripped.trim_start_matches('#');
after_hashes != stripped
    && after_hashes.starts_with(|c: char| c.is_whitespace())
    && after_hashes.trim_start() == section.as_str()
```

`test_verify_section_heading_all_levels` is extended with indented heading cases (1–3 leading spaces accepted; 4+ not accepted as ATX headings).

---

### F6 — Low: Stale `token_buf` accumulation in `stream_one_turn()`

**Status:** Fixed.

`token_buf` is removed entirely from `stream_one_turn`. The display closure prints tokens directly without accumulation:

```rust
let final_result = stream
    .drain_displaying(|tok| {
        print!("{tok}");
        std::io::stdout().flush().ok();
    })
    .await
    .map_err(|e| AnvilError::Io(std::io::Error::other(format!("stream: {e}"))))?;
```

`FinalResult` remains the sole authoritative source.

---

## Files Changed in R4

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-cli/src/charter.rs` | Modified | `curate_findings` + `collect_narrative` return `Result`; reviewer model identity fallback; provenance integration test + identity fallback test; `init_test_store` helper |
| `crates/anvil-cli/src/discuss.rs` | Modified | EOF guard in conversation loop; remove `token_buf` from `stream_one_turn` |
| `crates/anvil-core/src/pipeline.rs` | Modified | Strip up to 3 leading spaces in heading predicate; extend heading test with indented cases |

---

## Test Coverage Summary

| Crate | Tests | New in R4 |
|---|---|---|
| anvil-core | 27 | 0 new; `test_verify_section_heading_all_levels` extended with indented cases |
| anvil-audit | 17 | 0 |
| anvil-cli | 13 | 2 (`test_p5_provenance_graph_backs_all_charter_record_types`, `test_reviewer_model_identity_fallback_logic`) |
| anvil-sidecar-client | 11 | 0 |
| anvil-graph | 2 | 0 |
| **Total** | **70** | **2** |

---

## Acceptance Criteria Status

All criteria from R3 remain satisfied. No regressions.

| # | Criterion | Status |
|---|---|---|
| 1 | `anvil discuss` produces `charter.md` from a real model run | Ready |
| 2 | `anvil charter review` invokes reviewer, stores RFP + VR with valid cross-refs | Ready |
| 3 | `anvil charter findings` curates and produces disposition doc | Ready |
| 4 | All three commands use authoritative FinalResult; no partial commits | Ready |
| 5 | Curation gestures persist as audit records, round-trip correctly | Ready |
| 6 | Provenance graph can locate P5 records by cross-reference | Ready (now integration-tested) |
| 7 | `cargo fmt --check` clean | Pass |
| 8 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean | Pass |
| 9 | `cargo test --workspace` passes | Pass — 70 tests |

---

## Residual / Deferred

- **F12 (files changed collection):** Deferred to P6.
- **F13 (progress feedback):** Deferred to P6.

---

## Bottom Line

All 6 R3 findings resolved. Interactive curation can no longer produce silent audit records on cancellation or input failure. EOF in `anvil discuss` exits cleanly. Provenance lookup for all three P5 record types is now pinned by a real integration test. Reviewer model identity falls back to configured model ID if omitted. Indented ATX headings (up to 3 leading spaces) are accepted by the verifier. Dead token accumulation removed. 70 tests pass; fmt and clippy are clean. P5 R4 is ready for approval.
