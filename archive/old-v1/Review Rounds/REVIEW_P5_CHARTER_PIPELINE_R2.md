# Anvil — P5 Charter Stage Pipeline R2 Review

**Date:** 2026-05-26  
**Phase:** P5 — Charter Stage Pipeline  
**Branch:** master  
**Prior round:** R1 — `REVIEW_P5_CHARTER_PIPELINE_R1_Findings.md`  
**Reviewer:** jvcan (coordinator)

---

## What Changed in R2

All 13 findings from the R1 findings file were addressed. The changes span four crates:
`anvil-core` (`pipeline.rs`, `render.rs`), `anvil-audit` (`records.rs`),
`anvil-cli` (`charter.rs`, `discuss.rs`).

---

## Validation

| Check | Command | Result |
|---|---|---|
| Format | `cargo fmt --all -- --check` | **Pass** |
| Lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | **Pass** |
| Tests | `cargo test --workspace` | **Pass — 66 tests, 0 failures** |

---

## R1 Findings Disposition

### F1 — High: CI validation not clean (fmt + clippy failing)

**Status:** Fixed.

`cargo fmt --all` was run to normalize formatting. All 28 clippy errors in `anvil-core` were resolved:

- `clippy::should_implement_trait`: `FindingSeverity::from_str` renamed to `FindingSeverity::parse`.
- `clippy::must_use_candidate`: `#[must_use]` added to `verify_findings`.
- `clippy::doc_markdown`: backtick-wrapped identifiers in doc comments throughout.
- `clippy::manual_let_else`: file-read `match` block converted to `let Ok(...) = ... else { ... }`.
- `clippy::cast_possible_truncation`: `usize as u32` → `u32::try_from(...).unwrap_or(u32::MAX)` in three locations (`pipeline.rs`, `records.rs`, `charter.rs`).
- `clippy::format_push_string` (many): all `push_str(&format!(...))` patterns in `render.rs` replaced with `write!(...).ok()` via `use std::fmt::Write as _`.
- `clippy::write_with_newline` (8): all `write!(out, "...\n")` patterns replaced with `writeln!(out, "...")`.
- `clippy::map_unwrap_or` / `map_unwrap_or_else`: `.map(...).unwrap_or(...)` → `.map_or(...)`.
- `clippy::items_after_statements`: inline `use` inside function body after statements moved to function top.
- `clippy::too_many_lines`: `run_charter_findings` annotated `#[allow(clippy::too_many_lines)]` — the function is an interactive multi-step TUI workflow and cannot be split without introducing artificial indirection.

---

### F2 — High: `anvil discuss` cannot deserialize Charter packet (`produced_at` missing)

**Status:** Fixed.

Added a private `PartialCharterPacket` struct in `pipeline.rs` containing only model-supplied fields (no `produced_at`). All list fields carry `#[serde(default)]` to tolerate missing optional arrays.

Added `CharterPacket::from_model_json(json: &str) -> Result<Self, AnvilError>` which:
1. Deserializes into `PartialCharterPacket`.
2. Constructs `CharterPacket` with `produced_at = Utc::now()`.

`finalize_charter()` in `discuss.rs` now uses `CharterPacket::from_model_json(packet_json)?` instead of `serde_json::from_str::<CharterPacket>(packet_json)`.

**New test:** `pipeline::tests::test_charter_packet_from_prompt_example` — parses the exact JSON example from the Interlocutor system prompt (which has no `produced_at`) and verifies the result passes `validate()`.

---

### F3 — High: Invalid two-part cross-reference keys break provenance lookup

**Status:** Fixed.

All P5 cross-reference keys now use `CrossRefKey::new(...).to_key_string()` producing the valid three-part format `charter.md:§root:R{N}`.

Affected sites:
- `run_charter_review`: `ReviewerFindingPacket` and `VerifierResult` cross-refs.
- `run_charter_findings`: `CuratedFindingsRecord` cross-ref.

**New test:** `charter::tests::test_p5_cross_ref_keys_parseable` — constructs the cross-ref key for rounds 1–3 and asserts each parses successfully with `CrossRefKey::parse`.

---

### F4 — High/Medium: `stream_one_turn()` could commit from token buffer

**Status:** Fixed.

The `_ => token_buf` fallback arm was removed. `stream_one_turn()` now fails explicitly for every non-authoritative outcome:

- `Some(Chat(_))` with empty content → `Err` ("cannot commit partial stream").
- `None` (absent FinalResult) → `Err` ("stream did not complete cleanly").
- `Some(_)` (unexpected variant) → `Err` ("unexpected response variant").

Token accumulation in `token_buf` is retained only for terminal display; it is never returned as the function result.

---

### F5 — Medium/High: RFP/VR pairing not validated before curation

**Status:** Fixed.

Added `source_packet_id: String` field to `VerifierResult` in `records.rs`. `VerifierResult::from_verified()` now takes `source_packet_id: String` and stores it.

`run_charter_review` passes `packet.packet_id.clone()` as `source_packet_id` when constructing `VerifierResult`.

`run_charter_findings` checks:

```rust
if vr.source_packet_id != rfp.packet.packet_id {
    return Err(AnvilError::Io(...));
}
```

This fails fast with a clear message directing the user to re-run `anvil charter review`.

**New tests:**
- `records::tests::test_verifier_result_source_packet_id_stored` — verifies the field survives construction.
- `charter::tests::test_rfp_vr_pairing_struct` — constructs a `VerifierResult` with a known `source_packet_id` and asserts the field is preserved.

---

### F6 — Medium: `CurationAction::Edit` records no edited finding data

**Status:** Fixed (deferred to P6).

`Edit` is removed from the P5 interactive `Select` menu. The curation flow now offers only `Keep / Drop / Annotate`. `CurationAction::Edit` is preserved in the enum for P6+ with a doc comment marking it as reserved.

The `edited_finding: None` field persists for `Keep/Drop/Annotate` paths, which is correct — only `Edit` dispositions are supposed to populate it.

---

### F7 — Medium: Reviewer response handling silently swallows non-Chat results

**Status:** Fixed.

The `_ => String::new()` catchall in `invoke_reviewer()` was replaced with explicit arms:

- `None` result → `Err` ("no result — possible transport or timeout issue").
- `Some(_)` unexpected variant → `Err` ("unexpected result variant").

The caller's `ModelResponseMissingPacket` error is now only reachable if the model actually returned a chat response lacking `<findings_packet>` tags.

---

### F8 — Low/Medium: Section-heading grounding misses `###` and deeper

**Status:** Fixed.

The verifier no longer checks against fixed `# {section}` / `## {section}` patterns. It now scans all lines structurally:

```rust
let found = content.lines().any(|line| {
    let after_hashes = line.trim_start_matches('#');
    after_hashes != line && after_hashes.trim_start() == section.as_str()
});
```

This matches headings at any depth (`#`, `##`, `###`, `####`, …) while rejecting non-heading lines.

**New test:** `pipeline::tests::test_verify_section_heading_all_levels` — writes a file with headings at levels 1–4 and verifies each is grounded.

---

### F9 — Low/Medium: In-bounds line range marked `Grounded` without text verification

**Status:** Fixed.

Line-range verification changed from:

- In-bounds → `Grounded`
- Out-of-bounds → `Refuted`

To:

- In-bounds → `CannotBeVerified` (evidence note: "line range is within file bounds but content was not verified")
- Out-of-bounds → `Refuted`

This more accurately reflects that we have not checked the cited lines' actual content.

**New test:** `pipeline::tests::test_verify_line_range_returns_cannot_be_verified` — asserts in-bounds returns `CannotBeVerified` and out-of-bounds returns `Refuted`.

---

### F10 — Low: Corrections text hardcoded to R1 semantics

**Status:** Fixed.

Added `corrections: &'a str` field to `DispositionInput`. The corrections section in `render_disposition_doc` now:
- Renders `_(none)_` when blank.
- Renders the provided text otherwise.

`run_charter_findings` collects corrections interactively alongside the other narrative inputs.

---

### F11 — Low: Disposition headings use literal `R<N-1>` placeholder

**Status:** Fixed.

Sections 5 and 6 now interpolate the actual prior round number:

```rust
let prev = r.saturating_sub(1);
write!(out, "## Files Changed Since R{prev}\n\n")
write!(out, "## Corrections to R{prev} Narrative\n\n")
```

**Updated tests:**
- `render::tests::test_disposition_doc_required_sections` now checks `"## Files Changed Since R0"` and `"## Corrections to R0 Narrative"` (round 1 → prev 0).
- `render::tests::test_disposition_doc_round2_headings` (new) verifies that a round 2 document contains `"## Files Changed Since R1"` and no `"R<N-1>"` placeholder.

---

### F12 — Low: Files Changed section always empty

**Status:** Partially addressed.

Interactive file-change collection is deferred to P6 (requires git integration or interactive multi-entry input). The empty-files label was changed from `"_(no files changed)_"` to `"_(not collected in this round)_"` so generated documents are not misleading. The `DispositionInput.files_changed` field remains available for callers that can supply data.

---

### F13 — Low: No progress feedback during long reviewer invoke

**Status:** Deferred to P6.

The single-line "Invoking reviewer…" message remains. A spinner or elapsed-time indicator requires a terminal control library (e.g., `indicatif`) not yet in the dependency tree. This is a UX enhancement, not a correctness issue, and is deferred.

---

## Files Changed in R2

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-core/src/pipeline.rs` | Modified | PartialCharterPacket + from_model_json; section/line-range grounding fixes; FindingSeverity::parse rename; #[must_use]; cast fixes; 5 new tests |
| `crates/anvil-core/src/render.rs` | Modified | corrections field; R<N-1> interpolation; writeln! / write! fixes; Files label; 2 new/updated tests |
| `crates/anvil-audit/src/records.rs` | Modified | source_packet_id field on VerifierResult; cast fix; doc fix; 1 new test |
| `crates/anvil-cli/src/charter.rs` | Modified | 3-part cross-ref keys; RFP/VR pairing check; invoke_reviewer error arms; Edit removed from P5 menu; corrections input; 2 new tests |
| `crates/anvil-cli/src/discuss.rs` | Modified | from_model_json; explicit FinalResult error arms; doc fix |

---

## Test Coverage Summary

| Crate | Tests | New in R2 |
|---|---|---|
| anvil-core | 26 | 5 (`test_charter_packet_from_prompt_example`, `test_verify_section_heading_all_levels`, `test_verify_line_range_returns_cannot_be_verified`, `test_disposition_doc_round2_headings`, `test_render_charter_md_required_sections` updated) |
| anvil-audit | 17 | 1 (`test_verifier_result_source_packet_id_stored`) |
| anvil-cli | 10 | 2 (`test_p5_cross_ref_keys_parseable`, `test_rfp_vr_pairing_struct`) |
| anvil-sidecar-client | 11 | 0 |
| anvil-graph | 2 | 0 |
| **Total** | **66** | **8** |

---

## Acceptance Criteria Status

| # | Criterion | Status |
|---|---|---|
| 1 | `anvil discuss` produces `charter.md` from a real model run | Ready — `from_model_json` fixes the deserialization path |
| 2 | `anvil charter review` invokes reviewer, stores RFP + VR with valid cross-refs | Ready — 3-part keys; explicit error handling |
| 3 | `anvil charter findings` curates and produces disposition doc | Ready — corrections collected; R<N-1> fixed |
| 4 | All three commands use authoritative FinalResult; no partial commits | Ready — token-buffer fallback removed from all commit paths |
| 5 | Curation gestures persist as audit records, round-trip correctly | Ready — Edit removed from P5; Keep/Drop/Annotate persist correctly |
| 6 | Provenance graph can locate P5 records by cross-reference | Ready — 3-part keys; parseability test added |
| 7 | `cargo fmt --check` clean | Pass |
| 8 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean | Pass |
| 9 | `cargo test --workspace` passes | Pass — 66 tests |

---

## Residual / Deferred

- **F12 (files changed collection):** Deferred to P6. Current label accurately reflects uncollected state.
- **F13 (progress feedback):** Deferred to P6. Requires `indicatif` or equivalent terminal UI dependency.

---

## Bottom Line

All 6 minimum-approval blockers from R1 are resolved. Clippy and fmt are clean, the `anvil discuss` deserialization path is fixed, cross-reference keys are valid, the no-partial-output invariant is enforced, RFP/VR pairing is validated, and Edit is deferred cleanly. Eight new tests pin the fixed behaviors. P5 is ready for approval.
