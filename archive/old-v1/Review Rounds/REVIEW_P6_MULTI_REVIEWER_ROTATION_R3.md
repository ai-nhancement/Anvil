# anvil-core / anvil-cli — P6 Multi-Reviewer Rotation + Convergence Safeguards — R3 Disposition

**Date:** 2026-05-26
**Artifact:** P6 Multi-Reviewer Rotation + Convergence Safeguards
**Round:** R3
**Reviewer:** reviewer-1

---

## What Changed in R3

Addressed all 4 findings from the R2 review:

**High finding fixed:**

- **F1 (allow_empty(false) absent from Drop/Defer prompts):** Added `.allow_empty(false)` to both the `DropAdvisory` and `DeferAdvisory` `Input` prompts in `curate_findings`. The TUI now enforces a non-empty entry before accepting the input, matching the documented R2 claim and the gate enforcement.

**Medium finding fixed:**

- **F2 (No test for advisory gate failure path):** Added `test_advisory_gate_rejects_drop_advisory_with_empty_annotation` to `charter.rs` tests. Exercises `check_advisory_gate` (the function called by `run_charter_findings` before any writes) with: Drop-Advisory with no annotation (must fail), Defer-Advisory with whitespace-only annotation (must fail), and Accept-Advisory with no annotation (must pass). This is the gate behavior the `run_charter_findings` failure path depends on.

**Low findings fixed:**

- **F3 (Pre-R2 packets silently pass same-state check):** `run_status` now scans the artifact RFPs after computing `current_hash`. When any reviewer's latest packet lacks `artifact_hash` and a current hash is available, a warning is printed: `"Note: N reviewer packet(s) predate artifact-hash tracking (pre-R2); same-state verification skipped for: <names>"`. The backwards-compat rule in `check_full_pool_clean` is unchanged; the limitation is now surfaced to the operator.
- **F4 (PacketNotFound / FindingNotFound lack display tests):** `test_resolve_finding_rejects_unknown_packet` strengthened to assert the exact `PacketNotFound` variant (via `matches!`) and that the error message contains the packet_id string. Added `test_resolve_finding_rejects_unknown_finding`: stores a real `ReviewerFindingPacket` with finding `"F1"` in the audit store, calls `run_resolve_finding` with a valid packet_id but non-existent finding `"NONEXISTENT"`, and asserts the `FindingNotFound` variant with both `packet_id` and `finding_id` fields correct.

## Verification of R3 Claims

| Finding | Verifiable Claim | Verified? | Notes |
|---|---|---|---|
| — | No findings in R3 | — | — |

## Disposition of R3 Findings

| # | Severity | Finding | Disposition |
|---|---|---|---|
| — | — | No findings | — |

## Files Changed Since R2

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-cli/src/charter.rs` | Modified | `.allow_empty(false)` on Drop/Defer prompts (F1); advisory gate test (F2) |
| `crates/anvil-cli/src/status.rs` | Modified | Warning for pre-R2 packets without artifact_hash (F3) |
| `crates/anvil-cli/src/arbiter.rs` | Modified | Strengthened PacketNotFound test; new FindingNotFound test (F4) |
| `Review Rounds/REVIEW_P6_MULTI_REVIEWER_ROTATION_R2_Findings.md` | Added | R2 findings file (reviewer artifact) |
| `Review Rounds/REVIEW_P6_MULTI_REVIEWER_ROTATION_R3.md` | Added | This document |

## Residual / Deferred

_(none)_

## Reproducibility

```sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo test --workspace
```

All three pass clean. 91 tests (up from 89; 2 new tests added for FindingNotFound error path and advisory gate failure path).

## Bottom Line

P6 R3 addresses all 4 R2 findings. The prompt-level enforcement for Drop/Defer-Advisory annotations now matches the gate (F1 fully resolved). The advisory gate failure path is covered by a dedicated test in `charter.rs`. Pre-R2 packets without artifact hashes are surfaced as a warning in `anvil status`. PacketNotFound and FindingNotFound error variants are now asserted by name and message content in tests.
