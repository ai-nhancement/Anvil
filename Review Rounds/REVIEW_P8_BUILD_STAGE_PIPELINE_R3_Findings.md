# Anvil — P8 Build Stage Pipeline R3 Findings

**Source review doc:** `Review Rounds/REVIEW_P8_BUILD_STAGE_PIPELINE_R3.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Pass**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass** (136 tests)

All reproducibility commands pass. Source inspection confirms every R2 finding fix is present and matches the documented behavior. 4 new regression tests added.

---

## 1. Low — `run_phase_ship` and `run_phase_review` still carry `#[allow(clippy::too_many_lines)]`

**Location:**

- `crates/anvil-cli/src/phase.rs:340` (`run_phase_ship`)
- `crates/anvil-cli/src/phase.rs:220` (`run_phase_review`)

**Problem:**

Both entry points remain above the clippy line threshold even after R3 additions (`run_phase_findings`, preflight logic, stale-briefing guard). The allow attributes suppress the signal.

**Impact:**

- Minor maintainability debt.
- Future extensions (additional gates, new preflight checks) will increase the count further.

**Suggested fix:**

- Extract private helpers for gate preflight, round counting, and disposition rendering so the public functions drop below the threshold.
- The duplication is not a correctness or ship-blocking issue.

---

## Overall Assessment

R3 resolves every R2 finding with precise, minimal changes that align with the Charter/Plan patterns established in prior phases:

- F1: Rotation now uses `round_number = rfp_count + 1` (1-indexed) so consecutive reviews select different pool members; regression test pins the correct behavior.
- F2: `count_phase_briefing_rounds` (from `phase-{id}-briefing-sent` gates) provides authoritative `build_round`; ship blocks when `build_round > review_round` and reads the correct latest briefing file for the hash check.
- F3: `run_phase_findings` implements full interactive curation + disposition rendering + three gate records; `run_phase_ship` now enforces a 5-gate preflight before allowing termination.
- F4: Existence guard on `BRIEFING_{id}_R{N}.md` prevents overwrite; round derived from gate count.
- F5: Post-validation `phase_id` mismatch check returns a clear error naming both IDs.
- F6: `reviewer_name` from the configured binding is the sole authoritative `reviewer_id`; model-supplied identity is ignored.

Validation remains clean. All previously deferred items (findings curation, disposition rendering, remaining gate types) are now implemented. No new High or Medium issues were introduced.

P8 R3 is ready for commit. The single Low observation (line-count allows) is polish only and does not affect correctness or the six-gate audit trail.