# Anvil — P8 Build Stage Pipeline R1 Findings

**Source review doc:** `Review Rounds/REVIEW_P8_BUILD_STAGE_PIPELINE_R1.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Pass**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass** (135 tests)

All reproducibility commands pass. 10 new tests added for the phase briefing contract and ship termination gate.

---

## 1. High — `run_phase_ship` termination check is not phase-scoped

**Location:**

- `crates/anvil-cli/src/phase.rs:383` (`run_phase_ship`)
- `Review Rounds/REVIEW_P8_BUILD_STAGE_PIPELINE_R1.md` (hinge test claim)

**Problem:**

`run_phase_ship` calls:

```rust
let pool_result = check_full_pool_clean(&pool, &artifact_rfps, ...);
```

It re-uses the global pool clean logic (originally designed for charter/plan reviewer pools) without:

- Filtering RFPs to the specific phase briefing artifact (`phase-{id}.md` or equivalent).
- Requiring clean passes specifically on the phase briefing under review.
- Checking that prior phases have already shipped (per-phase sequencing).

The termination condition for a phase should be "all reviewers have clean passes on *this phase's* briefing since the last rotation", not the global pool state.

**Impact:**

- A phase could be shipped even when its own reviewer pool has not reached clean on the current briefing.
- The hinge test `test_phase_cannot_ship_without_termination` only exercises the global clean failure path; it does not prove phase-specific termination semantics.

**Suggested fix:**

- Extend `check_full_pool_clean` (or add a `check_phase_termination`) to accept an explicit phase/artifact filter and require clean passes scoped to that phase's briefing packets.
- Update the ship gate test to seed reviewer clean passes for the target phase only.

---

## 2. Medium — `PhaseBriefingContract.status` is an unconstrained string

**Location:**

- `crates/anvil-core/src/phase_briefing.rs:53`

**Problem:**

The contract contains:

```rust
pub status: String,
```

No enum, no validation against the Standard Vocabularies ("Draft", "In Review", "Approved", …), and `validate_phase_briefing_contract` does not inspect it.

**Impact:**

- A model can emit any status value; downstream rendering and queries cannot rely on a known vocabulary.
- Inconsistent with how `FindingSeverity` and `DispositionLabel` are strongly typed elsewhere.

**Suggested fix:**

- Introduce a `BriefingStatus` enum (or reuse an existing vocabulary type) and validate it inside `validate_phase_briefing_contract`.
- Update the JSON Schema and the 7-section hinge test to cover status values.

---

## 3. Low — `PhaseBriefingMissingSection` error variant defined but only exercised via the validator, not end-to-end from model output

**Location:**

- `crates/anvil-core/src/error.rs:122`
- `crates/anvil-core/src/phase_briefing.rs:87`

**Problem:**

The error is returned by `validate_phase_briefing_contract`, but `run_phase_build` never surfaces it to the user because the contract is produced by the Coder model and the CLI only prints success or a generic I/O error on parse failure.

**Impact:**

- The new error variant improves the domain model but provides no user-visible improvement in the current `anvil phase build` flow.
- Future curation or review commands that consume briefings will benefit, but the P8 R1 surface does not.

**Suggested fix:**

- Wire `PhaseBriefingMissingSection` into the error path of `parse_phase_briefing_contract` or `run_phase_build` so a malformed briefing produces the precise missing-section message.

---

## 4. Low — Residual note about duplicated hash helper remains unaddressed

**Location:**

- `Review Rounds/REVIEW_P8_BUILD_STAGE_PIPELINE_R1.md` (Residual section)

**Problem:**

The document itself flags that `status.rs::compute_hex_hash` still duplicates `utils.rs::sha256_hex` (introduced in earlier rounds). P8 R1 did not consolidate the helper.

**Impact:**

- Minor ongoing duplication in the hash-to-hex path used by both plan and phase flows.
- Not a correctness issue for P8.

**Suggested fix:**

- Extract the helper once (as previously recommended) in a follow-up cleanup pass; no action required for P8 approval.

---

## Overall Assessment

P8 R1 delivers the core per-phase loop (`build` → `review` → `ship`) with clean validation and 10 new tests. The `PhaseBriefingContract`, embedded JSON Schema, Finding Verifier integration, rotation logging, and GateApproval provenance records are all present and correctly ordered (gate before file writes).

However, the self-review nature of R1 plus two substantive gaps mean the "no findings" claim is overstated:

1. The ship termination gate re-uses global pool logic instead of enforcing phase-scoped clean passes — the most critical correctness issue for the new "per-phase" loop.
2. `status` field lacks vocabulary enforcement.

The deferred items (`anvil phase findings`, curation records) are explicitly noted and acceptable for P8 AC.

P8 should receive an independent review before commit. Minimum recommended before approval: address the phase-scoped termination check (F1) and add a `BriefingStatus` type (F2). Once those are resolved, the phase pipeline is ready to proceed to P9.