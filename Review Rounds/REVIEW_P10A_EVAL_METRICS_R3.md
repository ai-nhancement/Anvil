# P10a Evaluation Criteria Infrastructure — Review Briefing (R3)

**Date:** 2026-05-26
**Scope:** R2 findings applied — F1 rollback epoch filtering, F2 phase ID normalization, F3 precision deduplication, F4 avg_round_count scoping, F5 regression tests, F6 MetricTargets validation
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P10a — Evaluation Criteria Infrastructure
**Tests:** 175 passing (19 audit, 56 cli, 49 core, 14 eval, 9 graph, 17 ship, 11 sidecar-client), 0 failed
**Clippy:** Clean (`-D warnings`, all targets)
**Fmt:** Clean (`cargo fmt --all -- --check`)

---

## R2 Findings Resolution

### F1 (High) — Resolved: rollback epoch filtering applied to all phase metrics

Phase metrics now use only records from the current shipping epoch (after the most recent `RollbackEvent` for the phase and at/before the current ship disposition). Pre-rollback review rounds are excluded.

**New helpers in `crates/anvil-eval/src/lib.rs`:**

- `all_rollbacks(store)` — reads all `RollbackEvent` records.
- `phase_epoch_starts(rollbacks, shipped)` — for each shipped phase, the timestamp of the most recent rollback that invalidated it strictly before the current ship time.
- `rfp_in_epoch(created_at, ship_time, epoch_start)` — predicate: is this RFP timestamp within the current epoch?

**Updated functions:**
- `compute_human_minutes` — earliest current-epoch RFP per phase (not earliest-ever).
- `compute_finding_count_agreement` — only current-epoch RFPs per phase.
- `compute_history` — finding counts, dropped counts, and human minutes all scoped to current epoch via `current_epoch_packets` map.

**Location:** `crates/anvil-eval/src/lib.rs` (helpers at ~:131–:176; usage in compute functions)

---

### F2 (High) — Resolved: phase ID normalized from artifact-ref format

`ReviewerFindingPacket.phase_id` is stored as `"phase:P8:R1"` and `ConvergenceDeclaration.phase_id` as `"phase:P8"`, not as the bare `PhaseDisposition.phase_id = "P8"` used as the join key.

**New helper:**
```rust
fn phase_id_from_artifact_ref(s: &str) -> Option<&str>
```
Parses `"phase:P8:R1"` → `"P8"`, `"phase:P8"` → `"P8"`. Returns `None` for non-phase artifacts (`"charter.md"`, `"plan-R1"`), excluding them from all phase-scoped metrics.

**Applied in:**
- `compute_reviewer_precision` — phase RFPs only (`phase_id_from_artifact_ref` returns `Some`).
- `compute_finding_count_agreement` — phase RFPs only, grouped by normalized phase ID.
- `compute_human_minutes` — phase RFPs only.
- `compute_avg_round_count` — convergence declarations normalized to match `PhaseDisposition.phase_id`.
- `compute_history` — RFPs normalized; convergence declarations normalized.

**Location:** `crates/anvil-eval/src/lib.rs:135`–`138` (`phase_id_from_artifact_ref`); applied throughout compute functions.

---

### F3 (Medium) — Resolved: precision uses latest curation per packet and (packet_id, finding_id) set

**New helper:**
```rust
fn latest_curations_for_packets(store, known_packets) -> HashMap<String, CuratedFindingsRecord>
```
For each packet in `known_packets`, selects the curation record with the latest `created_at`. Orphan curations (packet not in `known_packets`) are ignored.

**Updated `compute_reviewer_precision`:**
- Filters to phase-artifact RFPs only; `known_packets` is derived from those RFPs.
- Uses `latest_curations_for_packets` to get one authoritative curation per packet.
- Counts unique `(packet_id, finding_id)` pairs with `CurationAction::Drop` (a `HashSet`) to prevent double-counting from within-record duplicates.

**Location:** `crates/anvil-eval/src/lib.rs:182`–`224` (`latest_curations_for_packets`), `225`–`263` (`compute_reviewer_precision`)

---

### F4 (Medium) — Resolved: `avg_round_count` scoped to shipped phases only

`compute_avg_round_count` now:
- Reads `PhaseDisposition` records to build the shipped-phase map.
- Applies `phase_id_from_artifact_ref` to `ConvergenceDeclaration.phase_id` (e.g., `"phase:P8"` → `"P8"`).
- Only includes declarations whose normalized phase ID appears in the shipped-phase map.

Charter and Plan convergence declarations (no `"phase:"` prefix) are excluded.

**Location:** `crates/anvil-eval/src/lib.rs:312`–`344` (`compute_avg_round_count`)

---

### F5 (Medium/Low) — Resolved: four targeted regression tests added

| Test | What it pins |
|---|---|
| `test_precision_excludes_arbiter_resolutions` | `ArbiterFindingResolution` records do not affect precision; precision = 0.5 with 2 findings and 1 curated drop |
| `test_finding_count_agreement_row_name` | Row name is `"Finding Count Agreement"`, not `"Cross-reviewer Agreement"` |
| `test_direction_suppressed_below_three_phases` | Direction is `Flat` with 2 phases, `Up` with 3 strictly increasing phases |
| `test_deferral_alert_message_contains_v1_note` | `DeferralOpenTooLong` message contains `"v1 note"` |

Total eval tests: 14 (was 10).

**Location:** `crates/anvil-eval/src/lib.rs:1055`–end

---

### F6 (Low) — Resolved: `MetricTargets` validates threshold ranges

**New method on `MetricTargets`:**
```rust
pub fn validate(&self) -> Result<(), AnvilError>
```
- `precision_min`, `agreement_max`, `escape_rate_max`: must be finite and in `[0.0, 1.0]`.
- `human_minutes_max`, `round_count_max`: must be finite and non-negative.
- Returns `AnvilError::InvalidConfigValue` on the first failing field.

**Wired into `AnvilConfig::validate`** (already called by both `load_config` and `save_config`).

**Location:** `crates/anvil-core/src/config.rs:217`–`257` (`MetricTargets::validate`); call at `config.rs:59`

---

## What to Review

1. **Epoch boundary definition.** Epoch start is `max(rollback.created_at)` where `rollback.invalidated_phase == phase_id` AND `rollback.created_at < ship_time`. Confirm this is the correct boundary: records created exactly at the rollback time are excluded; records at/before ship_time are included. The P9 rollback model creates rollback records and then expects new review to proceed immediately — this boundary correctly excludes the old epoch's records.

2. **Phase ID normalization completeness.** `phase_id_from_artifact_ref("phase:P8:R1")` → `"P8"`. Confirm all sites where RFP `phase_id` or `ConvergenceDeclaration.phase_id` are used for join or aggregation now go through this helper. The four sites are: `compute_reviewer_precision`, `compute_finding_count_agreement`, `compute_human_minutes`, `compute_avg_round_count`, and `compute_history`.

3. **`latest_curations_for_packets` authority rule.** When multiple `CuratedFindingsRecord` entries reference the same `packet_id`, the one with the latest `created_at` is used. Confirm this is the correct authority rule for v1 (an explicit re-curation supersedes the previous curation).

4. **`MetricTargets::validate` placement.** Validation runs in both `load_config` and `save_config` (via `AnvilConfig::validate`). Confirm this is acceptable — projects that already have `anvil.toml` with valid defaults will be unaffected; only malformed thresholds are rejected.

5. **Test coverage for F1 (epoch filtering) and F2 (phase ID normalization).** The four regression tests added in F5 do not yet cover the epoch-filtering or ID-normalization paths with real store data. The existing tests still use empty stores. Consider whether integration-level tests are required for correctness confidence on F1/F2 before merge.

---

## Test Coverage Summary

**`crates/anvil-eval/src/lib.rs`** (14 tests):
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
- `test_precision_excludes_arbiter_resolutions` *(new)*
- `test_finding_count_agreement_row_name` *(new)*
- `test_direction_suppressed_below_three_phases` *(new)*
- `test_deferral_alert_message_contains_v1_note` *(new)*

**`crates/anvil-cli/src/metrics.rs`** (2 tests):
- `test_metrics_show_empty_project_succeeds`
- `test_metrics_history_empty_project_succeeds`

**Total: 175 tests passing, 0 failed, clippy clean, fmt clean.**
