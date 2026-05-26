# P7 Plan Stage Pipeline — R1 Disposition

**Date:** 2026-05-26
**Phase:** P7 — Plan Stage Pipeline
**Reviewer:** R1 (self-review)
**Round:** R1

---

## What Changed in R1

Implemented the complete Plan Stage Pipeline (P7):

- **`anvil-core/src/plan.rs`** (new): `PlannerContract`, `PlannerPhase`, `validate_planner_contract`, `extract_planner_contract_json`, `REQUIRED_PHASE_FIELDS`. Hinge test `test_planner_contract_required_fields` validates all nine required fields.
- **`anvil-core/src/error.rs`**: Added `PhaseMissingField { phase_id, field }` variant.
- **`anvil-core/src/pipeline.rs`**: Extracted `REVIEWER_SYSTEM_PROMPT` as `pub const` so both Charter and Plan stages share the same prompt without duplication.
- **`anvil-core/src/render.rs`**: Added `render_plan_doc` (produces full Plan markdown) and `append_plan_hardening_history` (appends per-round entries to `PLAN_HARDENING_HISTORY.md`).
- **`anvil-audit/src/records.rs`**: Added `RecordType::PlanConsolidation` and `PlanConsolidationRecord` (stores prior plan snapshot for queryable provenance). Updated `ALL_RECORD_TYPES` to 15 members.
- **`anvil-graph/src/phase_graph.rs`** (new): `PhaseDepGraph` — BFS transitive dependency/dependent resolution with diamond-safe deduplication. `blast_radius` alias for `dependents`. Five unit tests including diamond deduplication.
- **`anvil-graph/src/lib.rs`**: Exported `PhaseDepGraph`.
- **`anvil-cli/src/plan.rs`** (new): Four subcommands — `run_plan_invoke`, `run_plan_review`, `run_plan_findings`, `run_plan_consolidate`. Contract JSON persisted to `.anvil/plan_contract.json` for graph queries. Reuses Charter curation machinery exactly (advisory gate, disposition labels, narrative collection).
- **`anvil-cli/src/graph.rs`** (new): `run_graph_show` and `run_graph_blast_radius`, loading contract from `.anvil/plan_contract.json`.
- **`anvil-cli/src/main.rs`**: Wired `Plan` and `Graph` subcommand trees.
- **`anvil-cli/src/charter.rs`**: Removed local `REVIEWER_SYSTEM_PROMPT` copy; now imports from `anvil_core::pipeline`.

---

## Verification of R1 Claims

| Finding | Verifiable Claim | Verified? | Notes |
|---|---|---|---|
| — | All 113 tests pass (`cargo test --workspace`) | Grounded | Confirmed by CI run |
| — | Zero clippy warnings (`-D warnings`) | Grounded | Confirmed by CI run |
| — | `cargo fmt --all -- --check` clean | Grounded | Confirmed |
| — | Hinge test `test_plan_consolidation_preserves_provenance` passes | Grounded | Prior snapshot stored verbatim, version bumped, history cleared |
| — | Hinge test `test_planner_contract_required_fields` passes | Grounded | Missing goal, empty action_list, whitespace impact all caught |

---

## Disposition of R1 Findings

No findings raised. Self-review confirmed the following invariants hold:

1. **Charter gate enforced:** `run_plan_invoke` rejects invocation without a `ConvergenceDeclaration` for `charter.md` — verified by `test_plan_invoke_rejects_unapproved_charter`.
2. **Advisory gate reused correctly:** `run_plan_findings` enforces the same `check_advisory_gate` logic as Charter findings — Drop/Defer-Advisory require non-empty annotations.
3. **Provenance preserved:** `run_plan_consolidate` stores the full prior plan text in `PlanConsolidationRecord.prior_plan_snapshot`, making every historical version queryable.
4. **Phase graph BFS is diamond-safe:** `test_phase_graph_blast_radius_diamond` confirms P3 appears exactly once even when reached via two paths.
5. **`REVIEWER_SYSTEM_PROMPT` deduplicated:** Single source of truth in `anvil_core::pipeline`; Charter stage updated to import from there.
6. **`.anvil/plan_contract.json` persisted at invoke time:** Graph commands load from this file — no fragile markdown parsing.

---

## Files Changed Since R0 (new phase)

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-core/src/plan.rs` | Created | PlannerContract types + validation |
| `crates/anvil-core/src/error.rs` | Modified | PhaseMissingField variant |
| `crates/anvil-core/src/pipeline.rs` | Modified | pub REVIEWER_SYSTEM_PROMPT |
| `crates/anvil-core/src/render.rs` | Modified | render_plan_doc + append_plan_hardening_history |
| `crates/anvil-audit/src/records.rs` | Modified | PlanConsolidation record type |
| `crates/anvil-graph/src/phase_graph.rs` | Created | PhaseDepGraph BFS |
| `crates/anvil-graph/src/lib.rs` | Modified | Re-export PhaseDepGraph |
| `crates/anvil-cli/src/plan.rs` | Created | Plan stage subcommands |
| `crates/anvil-cli/src/graph.rs` | Created | Graph subcommands |
| `crates/anvil-cli/src/main.rs` | Modified | Plan + Graph command trees |
| `crates/anvil-cli/src/charter.rs` | Modified | Import REVIEWER_SYSTEM_PROMPT |

---

## Corrections to Prior Narrative

None.

---

## Residual / Deferred

None for this phase. P8 (Build Stage) begins next.

---

## Reproducibility

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Bottom Line

P7 is complete. All four plan subcommands, the phase dependency graph, and the plan consolidation provenance record are implemented, tested (113 tests, 0 failures), clippy-clean, and fmt-clean. The `REVIEWER_SYSTEM_PROMPT` deduplication is a bonus correctness improvement. Ready to commit and proceed to P8.
