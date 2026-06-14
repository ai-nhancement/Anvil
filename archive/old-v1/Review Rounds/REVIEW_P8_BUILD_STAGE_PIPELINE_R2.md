# P8 — Build Stage Pipeline — R2 Disposition

**Date:** 2026-05-26
**Phase:** P8 — Build Stage Pipeline (Per-Phase Loop)
**Reviewer:** R2 Implementation (addresses R1 Findings)
**Round:** R2

---

## Findings Addressed

| # | Severity | Finding | Disposition |
|---|---|---|---|
| F1 | High | `run_phase_ship` termination check not phase-scoped | Fixed — artifact hash computed from latest briefing file; `packet.artifact_hash` set in `run_phase_review`; `current_hash` passed to `check_full_pool_clean` in `run_phase_ship` |
| F2 | Medium | `PhaseBriefingContract.status` is unconstrained string | Fixed — `BriefingStatus` enum with 6 serde-renamed variants; JSON Schema updated to `enum` array |
| F3 | Low | `PhaseBriefingMissingSection` not reachable from model output | Fixed — all 7 section fields gain `#[serde(default)]`; validator is now the single gate for `PhaseBriefingMissingSection` |
| F4 | Low | `compute_hex_hash` duplicates `utils::sha256_hex` | Fixed — `status.rs::compute_hex_hash` delegates to `crate::utils::sha256_hex`; local sha2/fmt imports removed |

---

## What Changed in R2

### F1 — Phase-scoped artifact hash for termination gate

**`crates/anvil-cli/src/phase.rs`**

- `run_phase_review`: changed briefing read from `read_to_string` to `read` (bytes); computes
  `briefing_hash = crate::utils::sha256_hex(&briefing_bytes)`; sets `packet.artifact_hash`
  after `apply_severity_tiering`.
- `run_phase_ship`: computes `round_count` and `current_hash` before `check_full_pool_clean`.
  `current_hash` reads the latest briefing file (`BRIEFING_{id}_R{round_count}.md`) and hashes
  its bytes; passes `current_hash.as_deref()` instead of `None`.
- `test_phase_ship_succeeds_with_clean_pass`: updated to write a briefing file, compute its
  hash, and set `packet.artifact_hash` to the matching value. Ship succeeds only when hashes
  agree.

### F2 — `BriefingStatus` enum

**`crates/anvil-core/src/phase_briefing.rs`**

- Added `BriefingStatus` enum with `Default = Draft` and serde renames: `"Draft"`,
  `"Awaiting Review"`, `"In Revision"`, `"Convergent"`, `"Approved"`, `"Superseded"`.
- `PhaseBriefingContract.status` field type changed from `String` to `BriefingStatus`.
- New tests: `test_briefing_status_roundtrip`, `test_invalid_status_value_produces_json_error`.

**`schemas/cli/phase_build.json`**

- `status` property updated from `{"type": "string", "minLength": 1}` to
  `{"type": "string", "enum": ["Draft", "Awaiting Review", "In Revision", "Convergent",
  "Approved", "Superseded"]}`.

### F3 — `PhaseBriefingMissingSection` reachable from model output

**`crates/anvil-core/src/phase_briefing.rs`**

- All 7 section fields on `PhaseBriefingContract` gain `#[serde(default)]`; a JSON object
  missing any section field now deserialises successfully (defaults to empty string / empty
  vec), then `validate_phase_briefing_contract` returns `PhaseBriefingMissingSection`.
- New test: `test_missing_section_field_produces_section_error_not_json_error` — seeds a JSON
  object with one section absent and asserts the error is `PhaseBriefingMissingSection`, not
  `ModelResponseBadJson`.

### F4 — Hash helper deduplication

**`crates/anvil-cli/src/status.rs`**

- `compute_hex_hash` body replaced with a single delegation call:
  `crate::utils::sha256_hex(content.as_bytes())`.
- Removed module-level `use std::fmt::Write as _` (no longer needed after delegation).

---

## Verification

| Claim | Verified? | Notes |
|---|---|---|
| 132 tests pass (`cargo test --workspace`) | Grounded | Counts: audit 17, cli 46, core 49, graph 9, sidecar 11 |
| Zero clippy warnings (`-D warnings`) | Grounded | Confirmed — clean pass |
| `cargo fmt --all -- --check` clean | Grounded | Confirmed — no diff |
| `packet.artifact_hash` set in `run_phase_review` | Grounded | `phase.rs` — set after `apply_severity_tiering` |
| `current_hash` passed to `check_full_pool_clean` in `run_phase_ship` | Grounded | `phase.rs` — computed from latest briefing bytes |
| `test_phase_ship_succeeds_with_clean_pass` proves hash-scoped termination | Grounded | Writes briefing, sets matching hash on RFP, asserts ship succeeds |
| `BriefingStatus` roundtrips all 6 variants | Grounded | `test_briefing_status_roundtrip` |
| Invalid status string → `ModelResponseBadJson` | Grounded | `test_invalid_status_value_produces_json_error` |
| Missing section field → `PhaseBriefingMissingSection` (not JSON error) | Grounded | `test_missing_section_field_produces_section_error_not_json_error` |
| JSON Schema `status` field is enum-constrained | Grounded | `schemas/cli/phase_build.json` — enum array with 6 values |
| `compute_hex_hash` delegates to `utils::sha256_hex` | Grounded | `status.rs` — one-line delegation, sha2/fmt imports removed |

---

## Files Changed Since R1

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-core/src/phase_briefing.rs` | Modified | `BriefingStatus` enum (F2), `#[serde(default)]` on section fields (F3), 3 new tests |
| `crates/anvil-cli/src/phase.rs` | Modified | Artifact hash in `run_phase_review` + `run_phase_ship` (F1), updated ship test |
| `crates/anvil-cli/src/status.rs` | Modified | `compute_hex_hash` delegates to `utils::sha256_hex` (F4) |
| `schemas/cli/phase_build.json` | Modified | `status` field changed to enum array (F2) |

---

## Residual / Deferred

- `anvil phase findings` (curation + disposition rendering) — deferred to P9/post-ship scope (carried from R1, unchanged).
- Gate types `phase-{id}-findings-curated`, `phase-{id}-disposition-rendered`, `phase-{id}-next-reviewer-or-ship` — not yet wired to CLI commands (carried from R1, unchanged).

---

## Reproducibility

```sh
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

---

## Bottom Line

P8 R2 addresses all four R1 findings. The phase-scoped termination gate (F1) is now
enforced end-to-end: `run_phase_review` records the briefing hash on the packet and
`run_phase_ship` computes the current briefing hash and requires agreement before approving
ship. `BriefingStatus` (F2) and `#[serde(default)]` section fields (F3) close the
vocabulary and error-surface gaps. The hash helper duplication (F4) is resolved. 132 tests
pass, clippy clean, fmt clean. P8 is ready to converge.
