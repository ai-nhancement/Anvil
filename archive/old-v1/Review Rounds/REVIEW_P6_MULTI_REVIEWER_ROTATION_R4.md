# anvil-core / anvil-cli — P6 Multi-Reviewer Rotation + Convergence Safeguards — R4 Disposition

**Date:** 2026-05-26
**Artifact:** P6 Multi-Reviewer Rotation + Convergence Safeguards
**Round:** R4
**Reviewer:** reviewer-1

---

## What Changed in R4

Addressed the single medium-severity finding from the R3 review:

**Medium finding fixed:**

- **F1 (Advisory counts use globally-latest curation record):** Both `count_open_advisory` (status.rs) and `count_open_advisory_findings` (arbiter.rs) now search for the latest `CuratedFindingsRecord` whose `packet_id` exactly matches `rfp.packet.packet_id`, iterating in reverse index order with `find_map`. If no matching record is found, dispositions default to empty (all advisory findings open). Previously both helpers used `curated_entries.last()` unconditionally, which could resolve advisory findings using curation records from unrelated packets or artifacts.

  Two regression tests added to `status.rs`:
  - `test_open_advisory_unrelated_packet_curation_does_not_satisfy`: two RFPs with finding `"F1"`, curation stored only for the first packet; assert `count_open_advisory` returns 1 for the second RFP.
  - `test_open_advisory_different_artifact_curation_does_not_satisfy`: a `charter.md` RFP and a `plan.md` RFP both with advisory `"F1"`; curation stored only for the `plan.md` packet; assert charter advisory count remains 1.

## Verification of R4 Claims

| Finding | Verifiable Claim | Verified? | Notes |
|---|---|---|---|
| — | No findings in R4 | — | — |

## Disposition of R4 Findings

| # | Severity | Finding | Disposition |
|---|---|---|---|
| — | — | No findings | — |

## Files Changed Since R3

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-cli/src/status.rs` | Modified | Packet-scoped curation lookup; two regression tests (F1) |
| `crates/anvil-cli/src/arbiter.rs` | Modified | Packet-scoped curation lookup in count_open_advisory_findings (F1) |
| `Review Rounds/REVIEW_P6_MULTI_REVIEWER_ROTATION_R3_Findings.md` | Added | R3 findings file (reviewer artifact) |
| `Review Rounds/REVIEW_P6_MULTI_REVIEWER_ROTATION_R4.md` | Added | This document |

## Residual / Deferred

_(none)_

## Reproducibility

```sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo test --workspace
```

All three pass clean. 93 tests (up from 91; 2 new regression tests).

## Bottom Line

P6 R4 addresses the single R3 medium finding. Advisory-count helpers in both `anvil status` and `anvil arbiter declare-convergence` now use packet-scoped curation records rather than the globally-latest record, closing the cross-packet and cross-artifact advisory bypass. Two regression tests pin the correct behavior.
