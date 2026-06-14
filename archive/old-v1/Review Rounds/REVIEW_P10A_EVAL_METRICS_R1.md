# P10a Evaluation Criteria Infrastructure — Review Briefing (R1)

**Date:** 2026-05-26
**Scope:** Full implementation — Layer-1 metric collection, Layer-2 target evaluation, Layer-3 alert engine, `anvil metrics show` / `anvil metrics history` CLI
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P10a — Evaluation Criteria Infrastructure
**Tests:** 171 passing (19 audit, 56 cli, 49 core, 10 eval, 9 graph, 17 ship, 11 sidecar-client), 0 failed
**Clippy:** Clean (`-D warnings`, all targets)
**Fmt:** Clean (`cargo fmt --all -- --check`)

---

## Implementation Summary

### New crate: `anvil-eval`

`crates/anvil-eval/src/lib.rs` — full implementation.

#### Layer-1 metrics

Six metrics computed from audit-store records. Each returns `None` when insufficient data exists rather than erroring; the metrics read path is deliberately permissive.

| Metric field | Data source | Formula |
|---|---|---|
| `reviewer_precision` | `ReviewerFindingPacket` + `CuratedFindings` + `ArbiterFindingResolution` | (total findings − dropped − arbiter-overridden) / total findings |
| `cross_reviewer_agreement` | `ReviewerFindingPacket` grouped by phase | avg(min_count / max_count) across phases with ≥2 reviewers |
| `human_minutes_per_phase` | `ReviewerFindingPacket` (start) + `PhaseDisposition` (ship) | avg wall-clock minutes from first RFP to ship disposition per shipped phase |
| `avg_round_count` | `ConvergenceDeclaration.round_count` | avg across all phases with a convergence declaration |
| `deferred_resolved_count` | `HingeFlip` | count of unique `hinge_test_name` values flipped (denominator requires P10b registry) |
| `defect_escape_rate` | `PhaseDisposition` + `RollbackEvent` | (phases with rollback after ship) / (total ever-shipped phases) |

Two public constants pin these counts:
- `LAYER1_METRIC_COUNT = 6` — hinge-pinned by `test_layer_1_metric_count`
- `ALERT_KIND_COUNT = 4` — hinge-pinned by `test_alert_kinds_count`

#### Layer-2 target evaluation — `evaluate_targets`

Takes `Layer1Metrics`, per-phase history, and `MetricTargets` (from `anvil.toml`). Returns a `Vec<MetricRow>` with value, direction (↑/↓/→), target range, and `TargetStatus` (Met / Violated / NoData) for each metric. Direction is computed by comparing the last two shipped phases in the history. The `Deferred Resolved` row always shows `NoData` status until P10b provides the denominator.

#### Layer-3 alert engine — `evaluate_alerts`

Four alert kinds, all four Charter-defined:

| Alert kind | Condition |
|---|---|
| `LowPrecision` | precision < `precision_min` (default 0.70) |
| `RisingHumanMinutesTrend` | last 3 phases (with minutes data) strictly increasing |
| `ExtremeAgreement` | agreement > `agreement_max` (default 0.90) |
| `DeferralOpenTooLong` | any `ProvisionalLock` created before the 5th-oldest ship event AND ≥5 shipped phases exist |

#### Per-phase history — `compute_history`

Returns `Vec<PhaseMetrics>` sorted by `shipped_at` ascending. Each entry covers: `phase_id`, `shipped_at`, `review_rounds` (from `ConvergenceDeclaration`), `human_minutes` (first RFP → ship disposition), `finding_count`, `dropped_count` (via `CuratedFindingsRecord` linked by `packet_id` → `ReviewerFindingPacket.packet.packet_id`), `rolled_back`.

### `anvil-core/src/config.rs` — `MetricTargets`

New struct added to `AnvilConfig` under `#[serde(default)]`. Five configurable thresholds, all with project-sensible defaults. Missing `[metric_targets]` in `anvil.toml` transparently uses defaults.

### CLI additions

**`anvil metrics show`** (`crates/anvil-cli/src/metrics.rs`):
- Displays all six metric rows with value, direction indicator, target, and status symbol
- Lists `[WARN]` lines for any violated metrics
- Lists alerts by kind tag (`[LOW-PRECISION]`, `[RISING-MINUTES]`, `[EXTREME-AGREEMENT]`, `[STALE-DEFERRAL]`)

**`anvil metrics history`**:
- Table of shipped phases: phase ID, ship date, review rounds, human minutes, finding count, rolled-back flag

---

## Acceptance Criteria Status

| AC | Status |
|---|---|
| AC1: All six Layer-1 metrics computed from audit-store data (no manual entry) | ✓ All six computed in `compute_layer1` |
| AC2: Layer-2 evaluation compares against project thresholds; `anvil metrics show` flags out-of-range metrics | ✓ `evaluate_targets` + `[WARN]` lines in CLI |
| AC3: Layer-3 alerts fire on the four alert kinds | ✓ `evaluate_alerts` implements all four |
| AC4: `anvil metrics show` displays values with direction indicators and target status | ✓ ↑/↓/→ + ✓/✗/- symbols |
| AC5: `anvil metrics history` shows per-metric values across shipped phases | ✓ `compute_history` + `run_metrics_history` |
| AC6: Deferred-Decision Rate reads from HingeFlip records; displays correctly if P10b is not complete | ✓ Shows raw resolved count with "P10b registry" as target; never errors on missing registry |

---

## Hinge Tests

| Test | Location | What It Pins |
|---|---|---|
| `test_layer_1_metric_count` | `anvil-eval/src/lib.rs` | Exactly 6 Layer-1 metrics |
| `test_alert_kinds_count` | `anvil-eval/src/lib.rs` | Exactly 4 alert kinds |

---

## What to Review

1. **`cross_reviewer_agreement` definition.** Agreement is computed as `avg(min_count / max_count)` across per-phase reviewer groups. This captures count-similarity (both reviewers find roughly the same number of issues) but not semantic overlap (they might flag entirely different issues). A true agreement metric requires string-matching or a finding-deduplication step beyond what audit-store records currently support. Is this approximation acceptable for v1, or should the metric be renamed to `finding_count_variance` to better describe what's actually measured?

2. **`reviewer_precision` double-counts dropped findings.** The current formula subtracts both `CuratedFindingsRecord` drops AND `ArbiterFindingResolution` count from total findings. An arbiter-resolved finding may already be reflected in the curated findings if the curation happened after the arbiter decision. In the current workflow these are separate operations (curation is per-packet, arbiter resolution is per-finding), but the overlap is possible. Should the arbiter-override term be removed (curated drops already capture the relevant dismissals), or is the current additive approach intentional?

3. **`evaluate_targets` direction computation.** Direction (↑/↓/→) compares the last two shipped phases in the history for `human_minutes` and `avg_round_count`; all other metrics show → (flat). For v1 with few shipped phases, the direction will almost always be → or toggling. Is this acceptable, or should direction be suppressed (not shown) until ≥3 data points exist?

4. **`DeferralOpenTooLong` cutoff logic.** The alert fires when a `ProvisionalLock` has `created_at ≤ shipped_times[n-5]` — i.e., the lock existed before the 5th-oldest ship event. "Open >5 shipped phases" is interpreted as "was present when at least 5 phases shipped after it." There is no resolution record for a ProvisionalLock (resolution happens in config at P11), so a ProvisionalLock that was converted to Final by editing `anvil.toml` will still trigger this alert because the record remains in the audit store. Is this acceptable for v1 (the alert becomes a false positive after P11 cleanup), or should the alert cross-reference the current config's choice lock states?

---

## Test Coverage Summary

**`crates/anvil-eval/src/lib.rs`** (10 new tests):
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

**`crates/anvil-cli/src/metrics.rs`** (2 new tests):
- `test_metrics_show_empty_project_succeeds`
- `test_metrics_history_empty_project_succeeds`

**Total: 171 tests passing, 0 failed, clippy clean, fmt clean.**
