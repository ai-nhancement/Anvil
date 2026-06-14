# P10a Evaluation Criteria Infrastructure — Review Briefing (R2)

**Date:** 2026-05-26
**Scope:** R1 findings applied — F1 precision formula, F2 metric rename, F3 direction suppression, F4 deferral alert documentation
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P10a — Evaluation Criteria Infrastructure
**Tests:** 171 passing (19 audit, 56 cli, 49 core, 10 eval, 9 graph, 17 ship, 11 sidecar-client), 0 failed
**Clippy:** Clean (`-D warnings`, all targets)
**Fmt:** Clean (`cargo fmt --all -- --check`)

---

## R1 Findings Resolution

### F1 (High) — Resolved: arbiter-overrides term removed from `reviewer_precision`

`compute_reviewer_precision` previously subtracted both `CuratedFindingsRecord` drops and a count of `ArbiterFindingResolution` records, risking double-count when curation followed arbiter resolution.

**Change:** Removed the arbiter-overrides term entirely. The formula is now:

```rust
let upheld = total_findings.saturating_sub(total_dropped);
Ok(Some(f64::from(upheld) / f64::from(total_findings)))
```

Curated drops are the sole dismissal authority. The `ArbiterFindingResolution` record type is still used for store listing in other contexts but is no longer read in the precision path.

**Location:** `crates/anvil-eval/src/lib.rs:118`–`146` (`compute_reviewer_precision`)

---

### F2 (Medium) — Resolved: `cross_reviewer_agreement` renamed to `finding_count_agreement`

The metric measures count-similarity (min_count / max_count per phase), not semantic overlap. The old name implied semantic consensus.

**Changes:**
- `Layer1Metrics.cross_reviewer_agreement` → `finding_count_agreement`
- `compute_cross_reviewer_agreement` → `compute_finding_count_agreement`
- `evaluate_targets` row name: `"Cross-reviewer Agreement"` → `"Finding Count Agreement"`
- `evaluate_alerts` Alert 3 message: `"Cross-reviewer agreement"` → `"Finding count agreement"`
- All test struct initializations updated

The `AlertKind::ExtremeAgreement` enum variant and `[EXTREME-AGREEMENT]` CLI tag are unchanged (they refer to the alert kind, not the metric name).

**Locations:** `crates/anvil-eval/src/lib.rs` (struct, compute function, evaluate_targets, evaluate_alerts, tests)

---

### F3 (Medium) — Resolved: direction suppressed until ≥3 shipped phases

`evaluate_targets` previously compared the last two history entries for direction, producing noisy ↑/↓ indicators with <3 data points.

**Change:**

```rust
// Suppress direction until ≥3 shipped phases exist; two samples are too noisy.
let prev = if history.len() >= 3 {
    history.iter().rev().nth(1)
} else {
    None
};
let latest = if history.len() >= 3 { history.last() } else { None };
```

With 0–2 shipped phases, both `prev` and `latest` are `None`, so `direction_from` returns `Direction::Flat` (→) for all metrics. Direction becomes meaningful only at ≥3 data points.

**Location:** `crates/anvil-eval/src/lib.rs:493`–`500` (`evaluate_targets`)

---

### F4 (Medium) — Resolved: `DeferralOpenTooLong` alert message documents v1 limitation

The alert fires on `ProvisionalLock` audit records that predate the 5th-oldest ship event. These records persist in the audit store even after the choice is finalized in `anvil.toml` (resolution happens at P11).

**Change:** Alert message now appends:

```
(v1 note: ProvisionalLock audit records persist after config-level resolution;
 this alert may fire as a false positive after P11 finalizes provisional choices.)
```

**Location:** `crates/anvil-eval/src/lib.rs:709`–`718` (`evaluate_alerts`, alert 4)

---

### F5 (Low) — Accepted as no-action

Logic duplication between Layer-1 helpers and `compute_history` noted. Not required for v1 correctness; deferred to P10b/P11 evolution per R1 recommendation.

---

## What to Review

1. **F1 resolution correctness.** The arbiter-overrides term is now gone. Confirm that `reviewer_precision = (total_findings − curated_drops) / total_findings` is the intended final formula for v1, and that removing the arbiter term does not cause the precision to overstate retained quality in any edge case (e.g., a finding rejected by the arbiter that the curation step never saw).

2. **F2 rename completeness.** Confirm all public-facing occurrences of the old name have been updated: `Layer1Metrics` struct field, compute function, `evaluate_targets` row name, `evaluate_alerts` message. The `AlertKind::ExtremeAgreement` variant is intentionally unchanged.

3. **F3 threshold.** Direction is now suppressed until `history.len() >= 3`. Confirm this threshold is correct for v1. The alternative (always show →) was considered but rejected in favor of showing trend only when statistically meaningful.

4. **F4 message text.** Confirm the appended v1 note is clear and correctly describes the post-P11 behavior.

---

## Test Coverage (unchanged from R1)

**`crates/anvil-eval/src/lib.rs`** (10 tests):
- `test_layer_1_metric_count` — hinge, pins 6
- `test_alert_kinds_count` — hinge, pins 4
- `test_compute_layer1_empty_store_returns_none_for_optional_metrics`
- `test_compute_history_empty_store_returns_empty_vec`
- `test_evaluate_targets_all_no_data`
- `test_evaluate_targets_precision_violated`
- `test_evaluate_targets_precision_met`
- `test_evaluate_alerts_empty_store_no_alerts`
- `test_alert_low_precision_fires`
- `test_alert_extreme_agreement_fires`

**`crates/anvil-cli/src/metrics.rs`** (2 tests):
- `test_metrics_show_empty_project_succeeds`
- `test_metrics_history_empty_project_succeeds`

**Total: 171 tests passing, 0 failed, clippy clean, fmt clean.**
