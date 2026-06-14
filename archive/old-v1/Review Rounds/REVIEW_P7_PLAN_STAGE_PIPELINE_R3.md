# P7 Plan Stage Pipeline — R3 Disposition

**Date:** 2026-05-26
**Phase:** P7 — Plan Stage Pipeline
**Reviewer:** R2 Findings (R3 fixes)
**Round:** R3

---

## What Changed in R3

Addressed all six R2 findings:

- **F1 (High — Charter gate not bound to artifact state):** Added `artifact_hash: Option<String>` field to `ConvergenceDeclaration` (`serde(skip_serializing_if = "Option::is_none")` for backward compat). Updated `run_declare_convergence` in `arbiter.rs` to compute SHA-256 of the artifact file at declaration time and store it. Updated `run_plan_invoke` gate to find the most recent charter declaration, read `charter.md` immediately, and compare its SHA-256 to the approved hash if one is present. If hashes differ the gate fails with "modified since the convergence declaration". Declarations without a hash (pre-R3 records) pass through with the existing behavior. Added regression test `test_plan_invoke_charter_gate_fails_with_modified_charter`: seeds a declaration with charter-A hash, writes charter-B to disk, asserts invoke fails.

- **F2 (High/Medium — typed errors erased in `run_plan_invoke`):** Changed `map_err(|e| { ... AnvilError::Io(...) })` to `map_err(|e| { eprintln!(...); e })` so `parse_planner_contract`'s original `AnvilError::PhaseMissingField` propagates through `run_plan_invoke` to callers unchanged. The human-readable stderr print is preserved.

- **F3 (Medium — consolidation mutates files before audit record):** Reordered `run_plan_consolidate` so `AuditStore::open` and `store.append(&record)` execute before any `std::fs::write`. If the audit store is unavailable, the plan file and hardening history are not modified. File mutations only occur after the provenance record is durable.

- **F4 (Medium — `plan.md` vs `ANVIL_PLAN.md` path mismatch):** Changed `DEFAULT_PLAN_FILE` from `"ANVIL_PLAN.md"` to `"plan.md"`. Updated `run_plan_review` and `run_plan_findings` local `plan_file` variables from hardcoded literals to `DEFAULT_PLAN_FILE`. Updated the remaining hardcoded error message string in `run_plan_review` to use the variable. Updated `run_plan_invoke` doc comment.

- **F5 (Low/Medium — dangling deps not surfaced in CLI):** Added `eprintln!` warning in both `run_graph_show` and `run_graph_blast_radius` when `graph.dangling_deps()` is non-empty. Warning prints the count and the dangling phase ID(s) before normal output.

- **F6 (Low — stale comments):** Updated `records.rs` line 6 comment from "All 14 audit record types" to "All 15 audit record types". Updated `run_graph_show` doc comment to accurately describe loading from `.anvil/plan_contract.json` rather than the stale audit-store/ANVIL_PLAN.md description.

---

## Verification of R3 Claims

| Finding | Verifiable Claim | Verified? | Notes |
|---|---|---|---|
| — | 125 tests pass (`cargo test --workspace`) | Grounded | Up from 119 (R2); 6 new tests added across R2+R3 |
| — | Zero clippy warnings (`-D warnings`) | Grounded | Confirmed |
| — | `cargo fmt --all -- --check` clean | Grounded | Confirmed |
| F1 | `ConvergenceDeclaration` has `artifact_hash: Option<String>` | Grounded | `records.rs` struct + `new()` signature |
| F1 | `arbiter.rs` computes and stores SHA-256 at declaration time | Grounded | `run_declare_convergence` uses `sha2::Sha256::digest` |
| F1 | Gate fails when `charter.md` hash differs from declaration | Grounded | `test_plan_invoke_charter_gate_fails_with_modified_charter` passes |
| F1 | Gate still passes for declarations without hash (backward compat) | Grounded | `test_plan_invoke_charter_gate_passes_with_declaration` passes |
| F2 | `run_plan_invoke` propagates `PhaseMissingField` unchanged | Grounded | `map_err` now returns `e` directly |
| F3 | Audit record appended before file mutations | Grounded | `store.append` precedes both `fs::write` calls |
| F4 | `DEFAULT_PLAN_FILE` is `"plan.md"` | Grounded | Confirmed; `plan_file` variable in review/findings functions updated |
| F5 | Dangling dep warning printed in `run_graph_show` | Grounded | `eprintln!` added before phase loop |
| F5 | Dangling dep warning printed in `run_graph_blast_radius` | Grounded | `eprintln!` added before blast-radius output |
| F6 | Record-type count comment updated to 15 | Grounded | `records.rs:6` |
| F6 | `run_graph_show` doc reflects actual loader | Grounded | Doc now says `.anvil/plan_contract.json` |

---

## Disposition of R3 Findings

| # | Severity | Finding | Disposition |
|---|---|---|---|
| F1 | High | Charter gate not bound to current `charter.md` artifact state | Fixed |
| F2 | High/Medium | `run_plan_invoke` erases typed `PhaseMissingField` errors | Fixed |
| F3 | Medium | Consolidation mutates files before provenance record appended | Fixed |
| F4 | Medium | `ANVIL_PLAN.md` vs `plan.md` path convention mismatch | Fixed |
| F5 | Low/Medium | Dangling deps surfaced in library only, not in CLI graph output | Fixed |
| F6 | Low | Stale comments in `records.rs` and `graph.rs` | Fixed |

---

## Files Changed Since R2

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-audit/src/records.rs` | Modified | F1: `artifact_hash` field + `new()` param; F6: count comment |
| `crates/anvil-cli/src/arbiter.rs` | Modified | F1: compute SHA-256 at declaration time; update test call |
| `crates/anvil-cli/src/plan.rs` | Modified | F1: hash-aware gate + regression test; F2: error propagation; F3: ordering; F4: `plan.md` |
| `crates/anvil-cli/src/graph.rs` | Modified | F5: dangling warnings; F6: doc comment |

---

## Corrections to Prior Narrative

None.

---

## Residual / Deferred

None. All findings closed.

---

## Reproducibility

```
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

---

## Bottom Line

All R2 findings addressed. 125 tests pass (up from 119), clippy clean, fmt clean. P7 is ready to commit.
