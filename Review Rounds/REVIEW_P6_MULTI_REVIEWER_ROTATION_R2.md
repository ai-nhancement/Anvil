# anvil-core / anvil-cli — P6 Multi-Reviewer Rotation + Convergence Safeguards — R2 Disposition

**Date:** 2026-05-26
**Artifact:** P6 Multi-Reviewer Rotation + Convergence Safeguards
**Round:** R2
**Reviewer:** reviewer-1

---

## What Changed in R2

Addressed all 6 hard blockers and all 5 medium/low findings from the R1 review:

**Hard blockers fixed:**

- **F1 (Drop/Defer advisory require non-empty annotation):** `curate_findings` now uses `allow_empty(false)` for `DropAdvisory` and `DeferAdvisory` prompts. `check_advisory_gate` updated to also fail if Drop/Defer advisory dispositions have an empty annotation field.
- **F2 (Full-pool clean on same artifact state):** `FindingsPacket` gains an `artifact_hash: Option<String>` field. `run_charter_review` computes and stores the SHA-256 hash of `charter.md` at review time. `check_full_pool_clean` now accepts `current_hash: Option<&str>` and rejects clean passes whose stored hash differs from the current artifact hash (hash-absent packets pass through for backwards compat).
- **F3 (reviewer_pool and override not in CLI):** `anvil setup` now populates `reviewer_pool` from reviewer-named model bindings. `anvil config show` displays `reviewer_pool` and `single_clean_pass_override`. `anvil config set reviewer_pool <comma-sep>` and `anvil config set single_clean_pass_override <true/false>` added.
- **F4 (Advisory gate after file writes):** Reordered `run_charter_findings` — advisory gate check runs immediately after curation, before any `fs::write` or hardening-history append.
- **F5 (resolve-finding accepts arbitrary IDs):** `run_resolve_finding` now parses and validates the composite `<packet_id>:<finding_id>` form, loads the referenced `ReviewerFindingPacket` from the audit store, and verifies the finding exists within it. New error variants `PacketNotFound` and `FindingNotFound` added to `AnvilError`.
- **F6 (Status/convergence counts not scoped by artifact):** `anvil status` gains `--artifact <path>` (default `charter.md`). All counts (RFPs, convergence declarations, arbiter-decided findings) are now filtered to the specified artifact. `run_declare_convergence` filters RFPs and arbiter records by the supplied artifact.

**Medium/low findings fixed:**

- **F7 (Arbiter-Decided findings not in reviewer briefing):** `run_charter_review` loads all `ArbiterFindingResolution` records and includes them in the reviewer prompt with explicit "Arbiter-Decided" labels.
- **F8 (P3 blocking in rounds 1–5):** `apply_severity_tiering` updated so P3 findings are marked `advisory = true` in all rounds (not just rounds 6+), per `ARTIFACT_SPECIFICATIONS.md` §Standard Vocabularies. Test updated to reflect the corrected semantics.
- **F9 (Advisory dispositions not in rendered docs):** `DispositionInput` gains `advisory_dispositions: &BTreeMap<String, (AdvisoryDispositionType, Option<String>)>`. `render_disposition_doc` renders explicit labels (`Accept-Advisory`, `Drop-Advisory: <reason>`, `Defer-Advisory: <phase>`) instead of `—` for advisory findings.
- **F10 (RotationLog self-rotation):** `RotationLog.rotated_from` changed to `Option<String>`; first-round rotation logs carry `None` (omitted in serialized JSON) rather than a self-referential value.
- **F11 (Review docs location):** R1 review doc and findings file moved to `Review Rounds/`.

## Verification of R2 Claims

| Finding | Verifiable Claim | Verified? | Notes |
|---|---|---|---|
| — | No findings in R2 | — | — |

## Disposition of R2 Findings

| # | Severity | Finding | Disposition |
|---|---|---|---|
| — | — | No findings | — |

## Files Changed Since R1

| File | Action | Purpose |
|---|---|---|
| `crates/anvil-core/src/error.rs` | Modified | PacketNotFound, FindingNotFound variants |
| `crates/anvil-core/src/pipeline.rs` | Modified | artifact_hash field, P3 always advisory, gate enforces non-empty Drop/Defer annotation |
| `crates/anvil-core/src/render.rs` | Modified | advisory_dispositions in DispositionInput; explicit advisory labels in rendered table |
| `crates/anvil-audit/src/records.rs` | Modified | RotationLog.rotated_from → Option\<String\> |
| `crates/anvil-cli/src/charter.rs` | Modified | Hash computation, arbiter briefing, F1/F4/F9/F10 fixes |
| `crates/anvil-cli/src/arbiter.rs` | Modified | F5 ID validation; F6 artifact-scoped counts; filter_rfps_by_artifact helper |
| `crates/anvil-cli/src/status.rs` | Modified | F2 hash check; F6 artifact-scoped counts; simplified API (pre-loaded RFPs) |
| `crates/anvil-cli/src/main.rs` | Modified | --artifact flag on status; reviewer_pool + override in config show/set |
| `crates/anvil-cli/src/setup.rs` | Modified | Populate reviewer_pool from reviewer-named bindings in commit() |
| `Review Rounds/` | Added | Moved R1 review + findings docs here |

## Corrections to R1 Narrative

R1 review doc was placed at repo root instead of `Review Rounds/`; corrected in R2.

## Residual / Deferred

_(none)_

## Reproducibility

```sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo test --workspace
```

All three pass clean. 89 tests (up from 84; 5 new tests added for hash-based same-state check and advisory gate enforcement).

## Bottom Line

P6 R2 addresses all 11 review findings. All 6 hard blockers resolved. Advisory gate is now enforced at the core level, full-pool clean requires same artifact state via SHA-256 hash, multi-reviewer configuration is reachable through setup and config CLI, and status/convergence counts are scoped per artifact.
