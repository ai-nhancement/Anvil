# P8 — Build Stage Pipeline — R3 Disposition

**Date:** 2026-05-26
**Phase:** P8 — Build Stage Pipeline (Per-Phase Loop)
**Reviewer:** R3 Implementation (addresses R2 Findings)
**Round:** R3

---

## Findings Addressed

| # | Severity | Finding | Disposition |
|---|---|---|---|
| F1 | High | `run_phase_review` selects same reviewer for R1 and R2 (off-by-one in `rotation_select`) | Fixed — `rotation_select(&pool, round_number)` where `round_number = RFP_count + 1`; prev_reviewer uses `round_number - 1` when `> 1`, matching Charter/Plan convention |
| F2 | High | `run_phase_ship` can ship older reviewed briefing while newer unreviewed briefing exists | Fixed — `build_round` derived from `count_phase_briefing_rounds` (briefing gate count); ship blocks when `build_round > review_round` with explicit "latest briefing R{N} has not been reviewed" reason; `current_hash` reads `BRIEFING_{id}_R{build_round}.md` not `R{review_round}.md` |
| F3 | High/Med | Missing `anvil phase findings`, six-gate completeness check, disposition rendering | Fixed — `run_phase_findings` implemented with interactive curation mirroring charter/plan; 5-gate preflight in `run_phase_ship`; 3 gate records written by `run_phase_findings`; `PhaseCmd::Findings` wired in `main.rs` |
| F4 | Medium | `run_phase_build` can overwrite existing briefing for same round | Fixed — round derived from `count_phase_briefing_rounds` (briefing gate count); existence guard rejects build if target file already exists; gate appended only when file will be new |
| F5 | Medium | Phase briefing `phase_id` not validated against CLI argument | Fixed — `briefing.phase_id.trim() != phase_id` check in `run_phase_build`; returns error naming both IDs |
| F6 | Medium | Reviewer binding identity not authoritative in phase review packets | Fixed — `reviewer_name.clone()` (configured binding) used as `reviewer_id` in `FindingsPacket::new`; `PartialFindingsPacket.reviewer_id` field removed; model-supplied identity is neither trusted nor stored |

---

## What Changed in R3

### F1 — Rotation off-by-one

**`crates/anvil-cli/src/phase.rs`**

- `run_phase_review`: `round_number = u32::try_from(phase_rfps.len()).unwrap_or(u32::MAX) + 1` (1-indexed).
- Reviewer selection: `rotation_select(&pool, round_number)` — R1 uses index 0, R2 uses index 1.
- `prev_reviewer`: `rotation_select(&pool, round_number - 1)` when `round_number > 1`.
- New regression test: `test_phase_rotation_uses_round_number_not_round_count` — two-reviewer pool asserts R1 → reviewer-1, R2 → reviewer-2, plus documents the broken pre-fix behaviour.

### F2 — Stale briefing ship

**`crates/anvil-cli/src/phase.rs`**

- New helper: `count_phase_briefing_rounds(store, phase_id)` — counts `phase-{id}-briefing-sent` gate records in the audit store, returning `u32`.
- `run_phase_ship`: `build_round = count_phase_briefing_rounds(...)`, `review_round = u32::try_from(phase_rfps.len())...`.
- Stale check: `if build_round > review_round { return Err(PhaseShipBlocked { reason: "latest briefing R{build_round} has not been reviewed..." }) }`.
- Hash: `current_hash` reads `BRIEFING_{id}_R{build_round}.md` bytes — the actual latest built artifact.
- New regression test: `test_phase_ship_blocked_by_stale_briefing` — one briefing gate, one clean RFP for R1, seeds second briefing gate for R2, asserts ship fails with message containing "R2".

### F3 — `anvil phase findings` and six-gate preflight

**`crates/anvil-cli/src/phase.rs`**

- New function `run_phase_findings(project_root, phase_id)`:
  - Loads latest phase RFP and VR from audit store.
  - Interactive curation loop via `dialoguer::{Input, Select}` — Keep/Drop/Annotate per finding; Disposition label for non-advisory kept findings.
  - Narrative inputs (author, summary, key decisions, open items).
  - Advisory gate advisory check via `check_advisory_gate`.
  - Renders disposition document to `reviews/REVIEW_phase-{id}_R{N}.md` via `render_disposition_doc`.
  - Persists `CuratedFindingsRecord` in audit store.
  - Creates three gate records: `phase-{id}-findings-curated`, `phase-{id}-disposition-rendered`, `phase-{id}-next-reviewer-or-ship`.
- `run_phase_ship` preflight: checks five gate records before proceeding; blocks with named list of missing gates.
  - Required: `phase-{id}-briefing-sent`, `phase-{id}-findings-received`, `phase-{id}-findings-curated`, `phase-{id}-disposition-rendered`, `phase-{id}-next-reviewer-or-ship`.
- New helper `phase_gate_exists(store, gate_name)` for O(n) gate presence check.
- New regression test: `test_phase_ship_preflight_blocks_missing_gates` — seeds only 2 of 5 gates, asserts `PhaseShipBlocked` listing the 3 missing gates.
- `test_phase_ship_succeeds_with_clean_pass` updated with `seed_preflight_gates` helper.
- New helper `seed_preflight_gates(store, phase_id)` — seeds all 5 required gates.

**`crates/anvil-cli/src/main.rs`**

- `PhaseCmd::Findings { id, project }` variant added to `PhaseCmd` enum.
- Dispatch: `PhaseCmd::Findings { id, project } => phase::run_phase_findings(&project, &id)`.

### F4 — Briefing overwrite prevention

**`crates/anvil-cli/src/phase.rs`**

- `run_phase_build`: round derived from `count_phase_briefing_rounds` not RFP count — consecutive builds before review increment round naturally.
- Existence guard: `if briefing_path.exists() { return Err(...) }` — rejects build if `BRIEFING_{id}_R{N}.md` already exists, requiring review before a repeated build.
- New regression test: `test_count_phase_briefing_rounds` — asserts count returns 0 before any gates, 1 after one `briefing-sent` gate.

### F5 — Phase ID mismatch check

**`crates/anvil-cli/src/phase.rs`**

- After `validate_phase_briefing_contract`: `if briefing.phase_id.trim() != phase_id { return Err(...) }`.
- Error names both the model-emitted ID and the requested ID.

### F6 — Reviewer identity authority

**`crates/anvil-cli/src/phase.rs`**

- `FindingsPacket::new(..., reviewer_name.clone(), ...)` — configured binding name is the authoritative `reviewer_id`.
- `PartialFindingsPacket.reviewer_id` field and `default_reviewer_id()` function removed; model-supplied identity is ignored entirely.

---

## Verification

| Claim | Verified? | Notes |
|---|---|---|
| 136 tests pass (`cargo test --workspace`) | Grounded | audit 17, cli 50, core 49, graph 9, sidecar 11 |
| Zero clippy warnings (`-D warnings`) | Grounded | Clean — `#[allow(clippy::too_many_lines)]` on `run_phase_ship` and `run_phase_review` |
| `cargo fmt --all -- --check` clean | Grounded | Confirmed — no diff |
| `rotation_select(&pool, round_number)` in `run_phase_review` | Grounded | `phase.rs` — `round_number = rfp_count + 1` |
| R1 → reviewer-1, R2 → reviewer-2 (two-reviewer pool) | Grounded | `test_phase_rotation_uses_round_number_not_round_count` |
| `count_phase_briefing_rounds` from briefing gate count | Grounded | `phase.rs` — filters `phase-{id}-briefing-sent` gate records |
| Stale briefing `build_round > review_round` blocks ship | Grounded | `test_phase_ship_blocked_by_stale_briefing` |
| Five-gate preflight in `run_phase_ship` | Grounded | `test_phase_ship_preflight_blocks_missing_gates` |
| `run_phase_findings` creates 3 gate records | Grounded | `phase.rs` — findings-curated, disposition-rendered, next-reviewer-or-ship |
| `PhaseCmd::Findings` wired in `main.rs` | Grounded | Dispatches to `phase::run_phase_findings` |
| Briefing existence guard in `run_phase_build` | Grounded | `phase.rs` — `if briefing_path.exists() { return Err(...) }` |
| Phase ID mismatch returns error | Grounded | `phase.rs` — `briefing.phase_id.trim() != phase_id` |
| `reviewer_name` is authoritative `reviewer_id` in packet | Grounded | `phase.rs` — `PartialFindingsPacket.reviewer_id` field removed |

---

## Files Changed Since R2

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-cli/src/phase.rs` | Modified | All 6 R2 findings; new `run_phase_findings`; 4 new tests; `seed_preflight_gates` helper |
| `crates/anvil-cli/src/main.rs` | Modified | `PhaseCmd::Findings` variant and dispatch |

---

## Residual / Deferred

None. All six R2 findings are addressed. The three previously deferred items (findings curation, disposition rendering, gate types `phase-{id}-findings-curated`, `phase-{id}-disposition-rendered`, `phase-{id}-next-reviewer-or-ship`) are now implemented.

---

## Reproducibility

```sh
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

---

## Bottom Line

P8 R3 addresses all six R2 findings. Reviewer rotation is now 1-indexed and matches the Charter/Plan convention. The stale-briefing ship path is closed by deriving round numbers from briefing gate counts rather than RFP counts. `anvil phase findings` is implemented with interactive curation and disposition rendering, completing the six-gate audit trail. Briefing overwrite is prevented by an existence guard. Phase ID and reviewer identity are now enforced by the Vault rather than trusted from model output. 136 tests pass, clippy clean, fmt clean.
