# Anvil — P5 Charter Stage Pipeline R3 Review

**Date:** 2026-05-26  
**Phase:** P5 — Charter Stage Pipeline  
**Branch:** master  
**Prior round:** R2 — `REVIEW_P5_CHARTER_PIPELINE_R2_Findings.md`  
**Reviewer:** jvcan (coordinator)

---

## What Changed in R3

All 4 R2 findings addressed. No regressions; 68 tests pass.

---

## Validation

| Check | Command | Result |
|---|---|---|
| Format | `cargo fmt --all -- --check` | **Pass** |
| Lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | **Pass** |
| Tests | `cargo test --workspace` | **Pass — 68 tests, 0 failures** |

---

## R2 Findings Disposition

### F1 — Medium: `run_charter_findings` too long, `#[allow]` suppress

**Status:** Fixed.

`run_charter_findings` was refactored into three private helpers:

- `load_and_pair(store: &AuditStore) -> Result<(ReviewerFindingPacket, VerifierResult), AnvilError>` — loads the latest RFP and VR, asserts `source_packet_id` match.
- `curate_findings(verified_findings: &[VerifiedFinding]) -> CurationResult` — the interactive per-finding selection loop; returns `CurationResult { actions, disposition_map, dispositions }`.
- `collect_narrative() -> NarrativeInputs` — collects the 5 disposition document text fields; returns `NarrativeInputs { narrative_summary, corrections, residual_notes, reproducibility, bottom_line }`.

`run_charter_findings` is now a ~60-line coordinator that calls these helpers in sequence. The `#[allow(clippy::too_many_lines)]` annotation is gone.

---

### F2 — Low: No error-path test for RFP/VR pairing mismatch

**Status:** Fixed.

Added `charter::tests::test_rfp_vr_pairing_mismatch_returns_error`. The test:

1. Initializes a real `AuditStore` in a `tempfile::TempDir`.
2. Appends a `ReviewerFindingPacket` with a generated `packet_id`.
3. Appends a `VerifierResult` with `source_packet_id = "rfp-WRONG-id"` (mismatched).
4. Calls `load_and_pair(&store)` and asserts the error message contains `"re-run"` and the RFP's actual `packet_id`.

`tempfile = "3"` added to `anvil-cli` dev-dependencies.

---

### F3 — Low: No negative test for `from_model_json` missing required fields

**Status:** Fixed.

Added `pipeline::tests::test_from_model_json_missing_required_fields` covering all four required fields:

| Case | Behavior |
|---|---|
| Missing `title` (no `#[serde(default)]`) | `from_model_json` returns `Err(ModelResponseBadJson)` |
| Missing `goals` (defaults to `[]`) | Parses OK; `validate()` returns `Err("goals")` |
| Missing `scope` (defaults to `""`) | Parses OK; `validate()` returns `Err("scope")` |
| Missing `success_criteria` (defaults to `[]`) | Parses OK; `validate()` returns `Err("success_criteria")` |

---

### F4 — Low: Heading predicate accepts `###NoSpace` (no space after hashes)

**Status:** Fixed.

The heading grounding predicate now requires at least one whitespace character after the `#` markers before the section name:

```rust
after_hashes != line
    && after_hashes.starts_with(|c: char| c.is_whitespace())
    && after_hashes.trim_start() == section.as_str()
```

`test_verify_section_heading_all_levels` extended to also verify that `###NoSpace` (no space separator) is NOT grounded (`CannotBeVerified`).

---

## Files Changed in R3

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-cli/src/charter.rs` | Modified | Extract `load_and_pair`, `curate_findings`, `collect_narrative`; add pairing mismatch test; add `tempfile` dev-dep |
| `crates/anvil-cli/Cargo.toml` | Modified | `tempfile = "3"` dev-dependency |
| `crates/anvil-core/src/pipeline.rs` | Modified | Tighten heading predicate; add `test_from_model_json_missing_required_fields`; extend heading test |

---

## Test Coverage Summary

| Crate | Tests | New in R3 |
|---|---|---|
| anvil-core | 27 | 1 (`test_from_model_json_missing_required_fields`); 1 extended (`test_verify_section_heading_all_levels`) |
| anvil-audit | 17 | 0 |
| anvil-cli | 11 | 1 (`test_rfp_vr_pairing_mismatch_returns_error`) |
| anvil-sidecar-client | 11 | 0 |
| anvil-graph | 2 | 0 |
| **Total** | **68** | **2** |

---

## Acceptance Criteria Status

All criteria from R2 remain satisfied. No regressions.

| # | Criterion | Status |
|---|---|---|
| 1 | `anvil discuss` produces `charter.md` from a real model run | Ready |
| 2 | `anvil charter review` invokes reviewer, stores RFP + VR with valid cross-refs | Ready |
| 3 | `anvil charter findings` curates and produces disposition doc | Ready |
| 4 | All three commands use authoritative FinalResult; no partial commits | Ready |
| 5 | Curation gestures persist as audit records, round-trip correctly | Ready |
| 6 | Provenance graph can locate P5 records by cross-reference | Ready |
| 7 | `cargo fmt --check` clean | Pass |
| 8 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean | Pass |
| 9 | `cargo test --workspace` passes | Pass — 68 tests |

---

## Residual / Deferred

- **F12 (files changed collection):** Deferred to P6. Label reads "_(not collected in this round)_".
- **F13 (progress feedback):** Deferred to P6.

---

## Bottom Line

All R2 follow-up items resolved. `run_charter_findings` no longer requires a lint suppress. The pairing mismatch error path is now test-covered against a real `AuditStore`. All required-field validation cases are regression-tested. The heading predicate correctly rejects non-standard `###NoSpace` lines. 68 tests pass; fmt and clippy are clean. P5 R3 is ready for approval.
