# Anvil — P7 Plan Stage Pipeline R3 Findings

**Source review doc:** `Review Rounds/REVIEW_P7_PLAN_STAGE_PIPELINE_R3.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Pass**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass** (125 tests)

All R3 reproducibility commands pass. Source inspection confirms every R2 finding fix is present and matches the documented behavior.

---

## 1. Low — SHA-256 hex formatting logic duplicated between `arbiter.rs` and `plan.rs`

**Location:**

- `crates/anvil-cli/src/arbiter.rs:75` (declaration-time hash)
- `crates/anvil-cli/src/plan.rs:129` (gate-time hash comparison)

**Problem:**

Both sites contain the identical 6-line pattern:

```rust
let digest = sha2::Sha256::digest(...);
let mut hex = String::with_capacity(64);
for b in &digest {
    write!(hex, "{b:02x}").unwrap();
}
hex
```

No shared helper exists.

**Impact:**

- Minor maintenance duplication.
- Future changes to hex encoding (e.g., use `hex` crate or const generics) must be applied in two places.

**Suggested fix:**

- Extract a small private `fn sha256_hex(bytes: &[u8]) -> String` in a common module (or inside `anvil-cli` utils) and call it from both sites.
- The duplication is not a correctness issue and does not block commit.

---

## Overall Assessment

R3 resolves every R2 finding with precise, minimal changes:

- F1: `ConvergenceDeclaration.artifact_hash` + hash comparison in the plan-invoke gate now binds approval to the exact charter state at declaration time. Pre-R3 records remain compatible.
- F2: Typed `PhaseMissingField` errors now propagate unchanged while still emitting a human-readable stderr line.
- F3: `PlanConsolidationRecord` is appended before any `fs::write` of `plan.md` or hardening history.
- F4: `DEFAULT_PLAN_FILE` consistently `"plan.md"`; all call sites updated.
- F5: `PhaseDepGraph` now tracks dangling dependency IDs and both graph subcommands emit a warning when present.
- F6: Stale comments corrected.

Validation remains clean and six new tests were added. No new issues of High or Medium severity were introduced. The single Low observation (hash formatting duplication) is a polish item only.

P7 R3 is ready for commit. All prior review feedback has been addressed.