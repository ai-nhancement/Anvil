# Anvil — P6 Multi-Reviewer Rotation R2 Findings

**Source review doc:** `Review Rounds/REVIEW_P6_MULTI_REVIEWER_ROTATION_R2.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Pass**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass** (89 tests)

All validation commands pass as claimed.

---

## 1. High — `allow_empty(false)` claimed for DropAdvisory / DeferAdvisory prompts but not present in code

**Location:**

- `crates/anvil-cli/src/charter.rs:447` (DropAdvisory prompt)
- `crates/anvil-cli/src/charter.rs:451` (DeferAdvisory prompt)
- `Review Rounds/REVIEW_P6_MULTI_REVIEWER_ROTATION_R2.md` (F1 claim)

**Problem:**

R2 states: "`curate_findings` now uses `allow_empty(false)` for `DropAdvisory` and `DeferAdvisory` prompts."

The actual code uses plain `Input::new().with_prompt(...).interact_text()` for both required advisory cases. No `allow_empty(false)` call exists anywhere in the crate. The gate (`check_advisory_gate`) correctly rejects empty annotations after the fact, but the interactive prompt still permits empty input.

**Impact:**

- The hard blocker F1 ("Drop/Defer advisory require non-empty annotation") is only partially fixed.
- A user can still submit an empty reason/target for Drop/Defer-Advisory; the error surfaces later at the gate instead of at the prompt.
- The review document overstates the completeness of the F1 fix.

**Suggested fix:**

- Add `.allow_empty(false)` to the two advisory Input prompts that require a value.
- Update the R2 narrative or re-verify once the UI enforcement matches the gate.

---

## 2. Medium — No dedicated test exercises the advisory gate failure message path after curation

**Location:**

- `crates/anvil-cli/src/charter.rs:619` (post-curation `check_advisory_gate` call)
- `crates/anvil-core/src/pipeline.rs:1008` (existing gate unit tests)

**Problem:**

Existing tests cover `check_advisory_gate` in isolation. There is no integration-style test in `charter.rs` that runs a full curation session producing an incomplete advisory disposition and asserts that `run_charter_findings` emits the exact "advisory gate check failed" error and remediation hint before any writes occur.

**Impact:**

- The F4 reorder (gate before writes) and F1 enforcement are only verified at the unit level.
- A regression that moves the gate call after a write would not be caught by the current test suite.

**Suggested fix:**

- Add a behavior test (or mock the dialoguer inputs) that triggers an incomplete advisory disposition and asserts early exit with the gate error.

---

## 3. Low — `check_full_pool_clean` backwards-compat rule silently accepts pre-R2 packets even when a current hash is supplied

**Location:**

- `crates/anvil-cli/src/status.rs:252`

```rust
let is_current_state = match (&rfp.packet.artifact_hash, current_hash) {
    (Some(rfp_hash), Some(expected)) => rfp_hash == expected,
    _ => true, // No hash on either side: unknown state, allow
};
```

**Problem:**

When an old RFP (no `artifact_hash`) is evaluated against a supplied `current_hash`, the match arm falls to `_ => true`. The comment says "unknown state, allow", which preserves compat but means a clean pass from a pre-R2 reviewer is never invalidated by later state changes.

**Impact:**

- The same-state guarantee only applies to reviewers who have run after the R2 hash change.
- A mixed pool containing both old and new reviewers can reach "full clean" even if the old reviewer's charter content is now stale.

**Suggested fix:**

- Document the limitation explicitly in the function and in `anvil status` output.
- Consider emitting a warning when any RFP lacks `artifact_hash` during a clean-pool evaluation.

---

## 4. Low — New `PacketNotFound` / `FindingNotFound` error variants lack round-trip or display tests

**Location:**

- `crates/anvil-core/src/error.rs:113`
- `crates/anvil-cli/src/arbiter.rs:146` and `153` (usage sites)
- `crates/anvil-cli/src/arbiter.rs:263` (only malformed-ID test exists)

**Problem:**

Tests exist for malformed composite IDs, but no test asserts that the new `AnvilError` variants are produced with the expected messages when a valid composite ID references a missing packet or missing finding.

**Impact:**

- The F5 fix ("resolve-finding accepts arbitrary IDs") is only partially regression-protected.
- Future changes to error construction could silently alter user-visible messages.

**Suggested fix:**

- Add two focused tests: one that supplies a valid composite ID for a non-existent packet, and one for a valid packet but non-existent finding ID; assert the exact variant and message content.

---

## Overall Assessment

R2 resolves the majority of the 11 R1 findings. Validation is clean and 5 new tests were added. Core changes (artifact_hash + same-state check, advisory gate reorder, composite-ID validation, artifact-scoped status, reviewer_pool wiring, RotationLog Option, P3-always-advisory, explicit advisory labels in render) are correctly implemented.

However, the most important hard-blocker fix (F1) is incomplete at the interactive layer: the claimed `allow_empty(false)` enforcement for required advisory Drop/Defer notes is absent from the prompt code. This means the advisory gate can still be reached with empty annotations from the TUI, contradicting the R2 disposition.

P6 R2 is close to approval but should not be considered complete until the prompt-level enforcement matches the documented claim and the gate.

Minimum recommended before approval:
1. Add `.allow_empty(false)` to the two advisory Input prompts and re-verify.
2. Add at least the two missing error-path tests for PacketNotFound / FindingNotFound.
3. Consider a behavior test that exercises the full advisory-gate failure path inside `run_charter_findings`.
