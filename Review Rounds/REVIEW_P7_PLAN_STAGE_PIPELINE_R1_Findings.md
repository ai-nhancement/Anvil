# Anvil — P7 Plan Stage Pipeline R1 Findings

**Source review doc:** `Review Rounds/REVIEW_P7_PLAN_STAGE_PIPELINE_R1.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Pass**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass** (113 tests)

All reproducibility commands pass as claimed in the self-review.

---

## 1. High — Charter convergence gate only checks existence, not currency or finality of the declaration

**Location:**

- `crates/anvil-cli/src/plan.rs:98` (`run_plan_invoke`)
- `Review Rounds/REVIEW_P7_PLAN_STAGE_PIPELINE_R1.md` (invariant 1 claim)

**Problem:**

The gate:

```rust
let charter_approved = conv_entries.iter().any(|e| { ... r.phase_id == "charter.md" });
```

only verifies that at least one `ConvergenceDeclaration` record exists with `phase_id == "charter.md"`. It does not:

- Confirm the declaration is the most recent one for the charter.
- Verify the declaration round matches the latest RFP round.
- Ensure the charter actually reached a converged state (vs. an earlier, now-stale declaration).

**Impact:**

- A user could invoke the Planner after an old or superseded convergence declaration, violating the intent that the Charter must be in its final approved state.
- The test `test_plan_invoke_rejects_unapproved_charter` only covers the zero-declaration case.

**Suggested fix:**

- Change the gate to load the latest `ConvergenceDeclaration` for `charter.md` (by created_at or cross-reference round) and reject if none exists or if it is not the terminal state.
- Update the test to seed an obsolete declaration and assert rejection.

---

## 2. Medium — `PhaseDepGraph::build_from_contract` accepts dangling dependency references without validation or warning

**Location:**

- `crates/anvil-graph/src/phase_graph.rs:38` (the dependency wiring loop)

**Problem:**

When building the graph, `phase.dependencies` entries are blindly added to the reverse map even if the referenced phase_id does not exist in `contract.phases`. No existence check, no error collection, and no log.

**Impact:**

- A malformed Planner Contract with typos in dependency IDs will produce a graph that silently drops those edges.
- `dependencies()` / `blast_radius()` queries will return incomplete results without any indication that the contract itself was invalid.
- The nine-field validation in `validate_planner_contract` does not extend to cross-phase referential integrity.

**Suggested fix:**

- During build, collect any dependency IDs not present in the phase set and either return them as errors or store them for later diagnostic queries.
- Add a test that supplies a contract with a dangling dependency and asserts the dangling edge is either rejected or surfaced.

---

## 3. Low — `extract_planner_contract_json` performs no structural validation at extraction time

**Location:**

- `crates/anvil-core/src/plan.rs:100`

**Problem:**

The extractor only finds the `<planner_contract>...</planner_contract>` delimiters and returns the raw substring. It never attempts `serde_json::from_str::<PlannerContract>` or calls `validate_planner_contract`. All validation is deferred to the caller in `run_plan_invoke`.

**Impact:**

- A model that emits syntactically valid JSON but missing required phase fields will succeed extraction and only fail later, producing a less precise error ("bad json" vs. "missing field X in phase Y").
- The hinge test `test_planner_contract_required_fields` covers the validator but not the end-to-end extraction + validation path under bad model output.

**Suggested fix:**

- Have `extract_planner_contract_json` return `Result<PlannerContract, AnvilError>` (or keep the &str form but add a companion `parse_and_validate_planner_contract` helper).
- Document that callers must validate immediately after extraction.

---

## 4. Low — `render_plan_doc` carries an unconditional `#[allow(clippy::too_many_lines)]`

**Location:**

- `crates/anvil-core/src/render.rs:315`

**Problem:**

The function is ~130 lines and re-uses the same pattern as `render_disposition_doc`. No helper extraction was performed for section rendering, even though similar Charter rendering functions were left with the allow in prior phases.

**Impact:**

- Maintainability debt carried forward into P7.
- Adding new Plan sections (e.g., risk matrix, resource table) will increase the count further.

**Suggested fix:**

- Extract private helpers for phase table rendering, dependency list formatting, and hardening-history append logic so the public entry point stays under the clippy threshold.

---

## 5. Low — Shared `REVIEWER_SYSTEM_PROMPT` may be semantically incorrect for the Planner specialist role

**Location:**

- `crates/anvil-core/src/pipeline.rs` (now pub const)
- `crates/anvil-cli/src/plan.rs` (imports and re-uses for Planner invocation)

**Problem:**

P7 re-uses the exact Reviewer system prompt for the Planner model. The prompt language is written for "rigorous architecture and document reviewer" producing Findings Packets; it is not tailored to a Planner that must emit a structured `PlannerContract` with nine specific phase fields.

**Impact:**

- The model may produce lower-quality contract output because the system prompt does not instruct it on phase schema, dependency semantics, or hinge-test expectations.
- Future Planner-specific prompt improvements are blocked by the "single source of truth" decision.

**Suggested fix:**

- Introduce a distinct `PLANNER_SYSTEM_PROMPT` (or parameterize the existing prompt) while keeping the Reviewer prompt unchanged.
- Update the deduplication claim in the R1 doc to reflect that only the Reviewer prompt was deduplicated; the Planner path now has its own prompt.

---

## Overall Assessment

P7 R1 implements a substantial new phase with clean validation results and 113 passing tests. The core artifacts (`PlannerContract`, `PhaseDepGraph`, `PlanConsolidationRecord`, four new CLI subcommands, and provenance-preserving consolidation) are present and functional.

However, the self-review nature of R1 combined with several substantive gaps means the "no findings" disposition is premature:

- The Charter gate is too permissive (existence vs. currency).
- The phase graph silently tolerates dangling dependencies.
- Extraction and rendering have minor robustness/maintainability shortfalls.
- Prompt reuse for the Planner specialist is semantically questionable.

P7 should receive an independent (non-author) review before being declared ready for commit, and the three higher-severity items above should be addressed or explicitly deferred with Plan updates. The phase is otherwise well-structured and reuses existing Charter machinery correctly.