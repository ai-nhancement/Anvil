# Anvil — P10a Eval Metrics R1 Findings

**Source review doc:** `Review Rounds/REVIEW_P10A_EVAL_METRICS_R1.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo build --workspace` — **passes**
- `cargo test --workspace` — **passes** (171 tests)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **passes**
- `cargo fmt --all -- --check` — **passes**
- `go test ./...` from `C:\Anvil\sidecar` — **passes**

---

## 1. High — `reviewer_precision` subtracts both curated drops and arbiter overrides, risking double-count when curation follows arbiter

**Location:**

- `crates/anvil-eval/src/lib.rs:144`–`153` (`compute_reviewer_precision`)
- `Review Rounds/REVIEW_P10A_EVAL_METRICS_R1.md` §"What to Review" item 2

**Problem:**

The formula computes:
```rust
let total_removed = total_dropped.saturating_add(arbiter_overrides);
let upheld = total_findings.saturating_sub(total_removed);
```
`arbiter_overrides` is simply the count of `ArbiterFindingResolution` records (capped at total_findings). `total_dropped` comes from `CuratedFindingsRecord` entries whose `CurationAction::Drop`.

In the workflow, curation (`CuratedFindings`) is per-packet and arbiter resolution is per-finding. Nothing prevents a finding that was already arbiter-resolved from later appearing in a `CuratedFindingsRecord` drop list (or vice versa). The current additive subtraction therefore over-subtracts in that overlap case.

**Impact:**

- Precision can be artificially lowered (or even go negative before saturating_sub) when the same logical dismissal is recorded twice.
- The metric is intended to reflect "findings retained through curation," yet the arbiter term mixes two different dismissal authorities.

**Suggested fix / improvement:**

- Remove the arbiter-overrides term. Curated drops already capture the final retained set after all human judgment steps; arbiter resolutions are an earlier filter that the curation step should already reflect.
- If the two operations are intentionally additive for v1, add a code comment explaining why overlap is considered impossible or acceptable.

---

## 2. Medium — `cross_reviewer_agreement` measures count similarity, not semantic agreement; name may mislead

**Location:**

- `crates/anvil-eval/src/lib.rs:180`–`197` (`compute_cross_reviewer_agreement`)
- `Review Rounds/REVIEW_P10A_EVAL_METRICS_R1.md` §"What to Review" item 1

**Problem:**

Agreement per phase is `min_count / max_count` of `finding_count` values across reviewers of that phase. This is a pure volume-similarity score. Two reviewers who flag completely disjoint sets of issues but happen to report the same number will score 1.0; two reviewers who flag overlapping issues but differ in count will score <1.0.

The field docstring calls it "Similarity of finding counts … high agreement suggests reduced diversity," which is accurate, but the public name `cross_reviewer_agreement` implies semantic consensus.

**Impact:**

- Consumers of `anvil metrics show` or the alert `ExtremeAgreement` may misinterpret the number as "reviewers are saying the same things."
- The alert message already hedges ("This may indicate reduced reviewer diversity"), but the metric name does not.

**Suggested fix / improvement:**

- Rename the metric to `finding_count_agreement` (or `finding_volume_agreement`) and update the alert message and CLI column header accordingly.
- Keep the current implementation for v1; a true semantic-overlap metric would require finding-level deduplication not present in the audit records.

---

## 3. Medium — Direction indicator computed from only the last two shipped phases; almost always Flat or noisy with small history

**Location:**

- `crates/anvil-eval/src/lib.rs:506`–`517` (`evaluate_targets`)
- `crates/anvil-eval/src/lib.rs:585`–`590` (`direction_from`)
- `Review Rounds/REVIEW_P10A_EVAL_METRICS_R1.md` §"What to Review" item 3

**Problem:**

`direction_from` is applied only to `human_minutes` and `avg_round_count`, using `history.iter().rev().nth(1)` vs `history.last()`. All other metrics are hardcoded `Direction::Flat`. With the typical early-project history size (0–3 shipped phases) the indicator is either absent or flips on every new ship.

**Impact:**

- The ↑/↓ symbols add visual noise without conveying stable trend information until a project has accumulated ≥3–4 data points.
- The "last two phases" rule is simple but statistically weak.

**Suggested fix / improvement:**

- Suppress direction (always show →) until `history.len() >= 3`. This matches the review question suggestion and avoids presenting a "trend" derived from two noisy samples.
- The current implementation is acceptable for v1 if the intent is simply "compare the most recent pair."

---

## 4. Medium — `DeferralOpenTooLong` uses a fixed 5-shipped-phase cutoff on `ProvisionalLock.created_at`; becomes false-positive after P11 resolution

**Location:**

- `crates/anvil-eval/src/lib.rs:684`–`719` (`evaluate_alerts`, alert 4)
- `Review Rounds/REVIEW_P10A_EVAL_METRICS_R1.md` §"What to Review" item 4

**Problem:**

The alert fires for any `ProvisionalLock` whose `created_at <= shipped_times[n-5]`. After P11 introduces resolution records (or config-driven finalization), the audit-store `ProvisionalLock` record remains; the alert will continue to report the choice as "open" even though it has been resolved in the live config.

**Impact:**

- Post-P11 the alert becomes a persistent false positive for any project that ever used provisional choices.
- The alert message lists `choice_key` values but gives no age or ship-phase count, making triage harder.

**Suggested fix / improvement:**

- For v1 the current behavior is acceptable (the alert is a signal that a provisional choice has survived many ship cycles). Document the limitation explicitly in the alert message or in `ANVIL_PLAN.md` §P10a.
- A future P11 enhancement can add a `ProvisionalLockResolved` record type or read the live config to filter resolved locks.

---

## 5. Low — Substantial logic duplication between `compute_layer1` helpers and `compute_history`

**Location:**

- `crates/anvil-eval/src/lib.rs:105` (`shipped_phase_latest_times`)
- `crates/anvil-eval/src/lib.rs:200`–`235` (`compute_human_minutes`)
- `crates/anvil-eval/src/lib.rs:339`–`444` (`compute_history`) — re-implements first-RFP map, shipped times, rollback detection, dropped-count linkage

**Problem:**

`compute_history` duplicates the earliest-RFP-per-phase logic, the shipped-phase timestamp map, the latest-rollback map, and the CuratedFindings dropped-count walk that already exist (or are similar to) the Layer-1 helpers. The two functions are never composed; each walks the store independently.

**Impact:**

- Maintenance burden: a change to how "first RFP" or "shipped time" is defined must be made in two places.
- Minor performance cost on `anvil metrics show` (which calls both paths).

**Suggested fix / improvement:**

- Extract shared helpers (`earliest_rfp_per_phase`, `latest_shipped_disposition_per_phase`, etc.) so `compute_history` and the metric functions can share the expensive store walks. Not required for R1 correctness but worth noting for P10b/P11 evolution.

---

## Summary of R1 Code Health

- All four open review questions are addressed above with direct references to the implementation.
- The permissive deserialize-skip pattern is consistent and appropriate for a metrics read path.
- The two hinge-pinned count tests are present; no other unit tests exercise the metric formulas or alert conditions.
- The `MetricTargets` defaulting via `#[serde(default)]` works correctly and matches the documented behavior.
- Once the four design questions are resolved (or accepted), the P10a implementation is ready for commit. No correctness, clippy, or formatting issues found.