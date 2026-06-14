# P7 Plan Stage Pipeline — R2 Disposition

**Date:** 2026-05-26
**Phase:** P7 — Plan Stage Pipeline
**Reviewer:** R1 Findings (R2 fixes)
**Round:** R2

---

## What Changed in R2

Addressed all five R1 findings:

- **F1 (High — Charter gate currency):** Changed `conv_entries.iter().any()` to `conv_entries.iter().rev().any()` so the gate resolves the most recent `ConvergenceDeclaration` for `charter.md` (last entry in the append-only store = latest). Added `test_plan_invoke_charter_gate_passes_with_declaration` — seeds a real declaration and confirms the gate passes (fails at missing `charter.md`, not at the gate). Refuted — declares: once a charter is converged in an append-only store there is no mechanism to un-converge it, but using `.rev()` ensures we process the latest record first and is the correct idiom regardless.

- **F2 (Medium — Dangling deps):** Added `dangling: Vec<String>` field to `PhaseDepGraph`; populated during `build_from_contract` by collecting referenced phase IDs absent from the known-phases set. Deduplication via `dangling_seen: HashSet`. Added `pub fn dangling_deps(&self) -> &[String]`. Edges are still wired (graceful degradation). Added `test_phase_graph_dangling_dep_surfaced` and `test_phase_graph_dangling_dep_deduplicated`.

- **F3 (Low — Extraction validation):** Added `parse_planner_contract(json: &str) -> Result<PlannerContract, AnvilError>` to `anvil-core/src/plan.rs` — combines `serde_json` deserialization with `validate_planner_contract`, returning a precise `PhaseMissingField` error on field-level failure. Documented `extract_planner_contract_json` as "structural extraction only." Updated `run_plan_invoke` to call `parse_planner_contract` instead of duplicating parse + validate. Added tests: `test_parse_planner_contract_valid`, `test_parse_planner_contract_bad_json`, `test_parse_planner_contract_missing_field`.

- **F4 (Low — `render_plan_doc` too many lines):** Extracted three private helpers — `render_phase_section`, `render_phase_dep_graph_section`, `render_deferred_decision_section`. Removed `#[allow(clippy::too_many_lines)]`; `render_plan_doc` now well under threshold.

- **F5 (Low — Refuted):** `run_plan_invoke` uses `PLANNER_SYSTEM_PROMPT` (defined in `plan.rs` with full planner-specific schema instructions). `run_plan_review` uses `REVIEWER_SYSTEM_PROMPT` (shared with Charter review — correct deduplication). The finding misidentified which prompt is used where. No code change needed.

---

## Verification of R2 Claims

| Finding | Verifiable Claim | Verified? | Notes |
|---|---|---|---|
| — | 119 tests pass (`cargo test --workspace`) | Grounded | Up from 113; 6 new tests added |
| — | Zero clippy warnings (`-D warnings`) | Grounded | `#[allow(clippy::too_many_lines)]` removed from render_plan_doc |
| — | `cargo fmt --all -- --check` clean | Grounded | Confirmed |
| F1 | Gate uses `.rev().any()` — latest declaration checked first | Grounded | Confirmed in source |
| F2 | `dangling_deps()` returns missing phase IDs | Grounded | `test_phase_graph_dangling_dep_surfaced` passes |
| F3 | `parse_planner_contract` gives field-level error on missing fields | Grounded | `test_parse_planner_contract_missing_field` passes |
| F4 | `render_plan_doc` has no `allow` suppression | Grounded | Confirmed; helpers extracted |
| F5 | Planner invocation uses `PLANNER_SYSTEM_PROMPT`, not Reviewer prompt | Grounded | Confirmed in `run_plan_invoke` at line ~159 |

---

## Disposition of R2 Findings

| # | Severity | Finding | Disposition |
|---|---|---|---|
| F1 | High | Charter gate: `.any()` → `.rev().any()` + passing-gate test | Fixed |
| F2 | Medium | Dangling deps silently dropped → surfaced via `dangling_deps()` | Fixed |
| F3 | Low | `extract_planner_contract_json` no validation → `parse_planner_contract` helper | Fixed |
| F4 | Low | `render_plan_doc` `#[allow(clippy::too_many_lines)]` → helpers extracted | Fixed |
| F5 | Low | Reviewer prompt reused for Planner | Refuted — Planner uses `PLANNER_SYSTEM_PROMPT` |

---

## Files Changed Since R1

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-cli/src/plan.rs` | Modified | F1: `.rev().any()` gate; F3: use `parse_planner_contract`; new test |
| `crates/anvil-core/src/plan.rs` | Modified | F3: `parse_planner_contract` helper + 3 tests |
| `crates/anvil-graph/src/phase_graph.rs` | Modified | F2: `dangling` field + `dangling_deps()` + 2 tests |
| `crates/anvil-core/src/render.rs` | Modified | F4: extract 3 private helpers; remove `#[allow]` |

---

## Corrections to Prior Narrative

R1 self-review claimed "no findings" — corrected by R1 Findings doc (5 findings, 4 fixed, 1 refuted).

---

## Residual / Deferred

None. All findings closed.

---

## Reproducibility

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Bottom Line

All R1 findings addressed. 119 tests pass, clippy clean, fmt clean. P7 is ready to commit.
