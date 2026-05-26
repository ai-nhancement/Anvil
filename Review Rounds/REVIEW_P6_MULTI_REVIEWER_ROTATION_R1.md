# anvil-core / anvil-cli — P6 Multi-Reviewer Rotation + Convergence Safeguards — R1 Disposition

**Date:** 2026-05-26
**Artifact:** P6 Multi-Reviewer Rotation + Convergence Safeguards
**Round:** R1
**Reviewer:** reviewer-1

---

## What Changed in R1

Implemented the full P6 deliverable set across `anvil-core` and `anvil-cli`:

- `crates/anvil-core/src/rotation.rs` (new): `rotation_select`, `is_advisory_round`, `ADVISORY_THRESHOLD_ROUND = 5`, `FullPoolCheckResult`
- `crates/anvil-core/src/pipeline.rs`: `advisory: bool` field on `Finding`; `apply_severity_tiering`; `AdvisoryDispositionType`; `check_advisory_gate`; `CurationDisposition.advisory_disposition`
- `crates/anvil-core/src/config.rs`: `reviewer_pool: Vec<String>` and `single_clean_pass_override: bool` on `AnvilConfig`
- `crates/anvil-core/src/error.rs`: `EmptyReasoning` and `ReviewerPoolEmpty` variants
- `crates/anvil-audit/src/records.rs`: enriched `RotationLog`, `ConvergenceDeclaration`, `ArbiterFindingResolution` with typed fields and constructors
- `crates/anvil-cli/src/arbiter.rs` (new): `run_declare_convergence`, `run_resolve_finding`
- `crates/anvil-cli/src/status.rs` (new): `run_status`, `check_full_pool_clean`, `count_open_advisory`
- `crates/anvil-cli/src/charter.rs`: rotation pool integration, severity tiering, advisory curation flow, advisory gate check
- `crates/anvil-cli/src/main.rs`: `ArbiterCmd`, `Command::Arbiter`, `Command::Status` wired in

## Verification of R1 Claims

| Finding | Verifiable Claim | Verified? | Notes |
|---|---|---|---|
| — | No findings in R1 | — | — |

## Disposition of R1 Findings

| # | Severity | Finding | Disposition |
|---|---|---|---|
| — | — | No findings | — |

## Files Changed Since R0

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-core/src/rotation.rs` | Added | Rotation arithmetic, advisory threshold, FullPoolCheckResult |
| `crates/anvil-core/src/pipeline.rs` | Modified | advisory flag, severity tiering, advisory gate, advisory disposition type |
| `crates/anvil-core/src/config.rs` | Modified | reviewer_pool and single_clean_pass_override fields |
| `crates/anvil-core/src/error.rs` | Modified | EmptyReasoning, ReviewerPoolEmpty variants |
| `crates/anvil-core/src/lib.rs` | Modified | pub mod rotation |
| `crates/anvil-audit/src/records.rs` | Modified | Enriched RotationLog, ConvergenceDeclaration, ArbiterFindingResolution |
| `crates/anvil-cli/src/arbiter.rs` | Added | declare-convergence and resolve-finding commands |
| `crates/anvil-cli/src/status.rs` | Added | anvil status command with full-pool clean check |
| `crates/anvil-cli/src/charter.rs` | Modified | Rotation pool, severity tiering, advisory curation flow |
| `crates/anvil-cli/src/main.rs` | Modified | ArbiterCmd enum, Status variant, match arms |

## Corrections to R0 Narrative

_(none)_

## Residual / Deferred

_(none)_

## Reproducibility

```sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo test --workspace
```

All three commands pass clean (84 tests).

## Bottom Line

P6 is complete. Rotation, advisory tiering, convergence declaration, finding resolution, full-pool clean check, and `anvil status` are all implemented, tested, and clippy/fmt clean.
