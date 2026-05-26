# P8 — Build Stage Pipeline — R1 Disposition

**Date:** 2026-05-26
**Phase:** P8 — Build Stage Pipeline (Per-Phase Loop)
**Reviewer:** R1 Self-Review (Coder)
**Round:** R1

---

## What Was Built in R1

Implemented the full per-phase build → review → ship loop:

- **`crates/anvil-core/src/error.rs`** — Added two new error variants: `PhaseBriefingMissingSection { phase_id, section }` and `PhaseShipBlocked { phase_id, reason }`.
- **`crates/anvil-core/src/phase_briefing.rs`** (new) — `PhaseBriefingContract` typed JSON contract (7 required sections), `BriefingFileChange`, `BriefingComplianceItem`, `BriefingTestArea` structs, `REQUIRED_BRIEFING_SECTIONS` constant, `validate_phase_briefing_contract`, `parse_phase_briefing_contract` (extracts `<phase_briefing>` tags), `extract_phase_disposition_md`, `render_phase_briefing_doc`. Hinge test `test_phase_briefing_required_sections` verifies all 7 sections rejected when missing.
- **`crates/anvil-core/src/lib.rs`** — Added `pub mod phase_briefing;`.
- **`schemas/cli/phase_build.json`** (new) — JSON Schema for `PhaseBriefingContract` (Amendment A1 `--describe-schema` deliverable); embedded in binary via `include_str!`.
- **`crates/anvil-cli/src/phase.rs`** (new) — `OutputFormat` enum, `PHASE_BUILD_SCHEMA` constant, `run_phase_build` (Coder invocation, `--format json`, `--describe-schema`), `run_phase_review` (reviewer rotation, Finding Verifier, four audit records), `run_phase_ship` (full-pool clean termination check, PhaseDisposition + GateApproval). Hinge test `test_phase_cannot_ship_without_termination`.
- **`crates/anvil-cli/src/main.rs`** — Added `mod phase;`, `PhaseCmd { Build, Review, Ship }` enum, `Phase(PhaseCmd)` command variant, wired into `run()`.

---

## Verification of R1 Claims

| Claim | Verified? | Notes |
|---|---|---|
| 135 tests pass (`cargo test --workspace`) | Grounded | Up from 125 (10 new tests) |
| Zero clippy warnings (`-D warnings`) | Grounded | Confirmed |
| `cargo fmt --all -- --check` clean | Grounded | Confirmed after `cargo fmt --all` |
| `PhaseBriefingContract` has 7 required sections | Grounded | `phase_briefing.rs` lines 72–80 |
| `validate_phase_briefing_contract` rejects each missing section | Grounded | `test_phase_briefing_required_sections` covers all 7 cases |
| `run_phase_ship` blocks without full-pool clean | Grounded | `test_phase_cannot_ship_without_termination` |
| `run_phase_ship` succeeds with clean pass + creates records | Grounded | `test_phase_ship_succeeds_with_clean_pass` |
| `--describe-schema` returns without config | Grounded | `test_describe_schema_prints_schema` |
| `GateApproval` records use `phase-{id}-{type}` naming | Grounded | `phase-{id}-briefing-sent`, `phase-{id}-findings-received`, `phase-{id}-ship` |
| Gate records written before file ops (provenance safety) | Grounded | `store.append(&gate)?` before `fs::write` in both `run_phase_build` and `run_phase_review` |
| `schemas/cli/phase_build.json` embedded in binary | Grounded | `include_str!("../../../schemas/cli/phase_build.json")` in `phase.rs:45` |
| Finding Verifier runs in `run_phase_review` | Grounded | `verify_findings(&packet.findings, project_root)` + `VerifierResult` stored |
| Rotation log stored in `run_phase_review` | Grounded | `RotationLog::new(...)` + `store.append` |
| `--format json` outputs briefing contract JSON | Grounded | `serde_json::to_string_pretty(&briefing)` branch |

---

## Disposition of R1 Findings

No external reviewer engaged for R1. Proceeding to R2 review.

| # | Severity | Finding | Disposition |
|---|---|---|---|
| — | — | No findings (R1 self-review) | — |

---

## Files Changed Since R0

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-core/src/error.rs` | Modified | `PhaseBriefingMissingSection` + `PhaseShipBlocked` variants |
| `crates/anvil-core/src/phase_briefing.rs` | Created | `PhaseBriefingContract` types, validation, rendering, tests |
| `crates/anvil-core/src/lib.rs` | Modified | `pub mod phase_briefing;` declaration |
| `schemas/cli/phase_build.json` | Created | JSON Schema for `PhaseBriefingContract` (Amendment A1) |
| `crates/anvil-cli/src/phase.rs` | Created | `run_phase_build`, `run_phase_review`, `run_phase_ship` + tests |
| `crates/anvil-cli/src/main.rs` | Modified | `mod phase;`, `PhaseCmd`, `Phase(PhaseCmd)` variant, `run()` wiring |

---

## Corrections to R0 Narrative

None (no prior round).

---

## Residual / Deferred

- `anvil phase findings` (curation + disposition rendering) — deferred to P9/post-ship scope. Phase review findings are currently stored in the audit store via `run_phase_review` but there is no interactive curation CLI for per-phase disposition documents. The existing `anvil charter findings` and `anvil plan findings` patterns cover their respective artifacts; a unified per-phase findings curation loop is the natural extension but is not required for P8 AC.
- `status.rs::compute_hex_hash` still duplicates `utils.rs::sha256_hex` — flagged in an earlier review round; deferred to a future cleanup pass.
- Gate types `phase-{id}-findings-curated`, `phase-{id}-disposition-rendered`, `phase-{id}-next-reviewer-or-ship` not yet wired to CLI commands. Records can be written manually via `anvil audit` or will be wired in the curation CLI extension.

---

## Reproducibility

```sh
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

---

## Bottom Line

P8 R1 complete. 135 tests pass, clippy clean, fmt clean. Both P8 hinge tests pass: `test_phase_briefing_required_sections` (all 7 sections validated) and `test_phase_cannot_ship_without_termination` (termination condition enforced). `anvil phase build/review/ship` wired and operational.
