# Anvil — P10a Eval Metrics R2 Findings

**Source review doc:** `Review Rounds/REVIEW_P10A_EVAL_METRICS_R2.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Pass**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass** (171 tests)
- `go test ./...` from `C:\Anvil\sidecar` — **Pass**

The R2 validation claims match the current workspace. The four R1 findings called out in the R2 briefing are applied: the arbiter override term is removed from reviewer precision, the count-similarity metric is renamed to `finding_count_agreement`, direction is suppressed until at least three shipped phases, and the stale-deferral alert now documents the v1 false-positive limitation.

---

## 1. High — Phase-level metrics still aggregate all historical RFPs/curations across rollback epochs, so re-opened phases can report stale findings and inflated minutes after re-ship

**Location:**

- `crates/anvil-eval/src/lib.rs:190-224` (`compute_human_minutes`)
- `crates/anvil-eval/src/lib.rs:329-434` (`compute_history`)
- `crates/anvil-eval/src/lib.rs:148-188` (`compute_finding_count_agreement`)
- `crates/anvil-eval/src/lib.rs:266-303` (`compute_defect_escape_rate`)
- P9 rollback semantics: `crates/anvil-ship/src/rollback.rs:94-146`

**Problem:**

P9 introduced rollback epochs: after `anvil phase reopen`, reviewer rotation and ship readiness use only records newer than the latest `RollbackEvent` for the phase. P10a metrics do not apply that same epoch boundary for several phase-level computations.

Examples:

- `compute_human_minutes` computes each shipped phase's minutes from the **earliest RFP ever recorded** for that phase to the latest ship disposition:

```rust
let mut phase_first_rfp: HashMap<String, DateTime<Utc>> = HashMap::new();
for rfp in rfps {
    let e = phase_first_rfp.entry(rfp.phase_id).or_insert(rfp.created_at);
    if rfp.created_at < *e {
        *e = rfp.created_at;
    }
}
```

After a rollback and re-ship, this measures from the original pre-rollback review rather than from the first review/build cycle in the current post-rollback epoch.

- `compute_history` similarly totals all `ReviewerFindingPacket` finding counts and all linked `CuratedFindingsRecord` drops ever recorded for the phase, regardless of whether those packets belong to an invalidated pre-rollback epoch.

- `compute_finding_count_agreement` groups all RFP counts by `phase_id` and includes old reviewer packets after rollback, which can produce an agreement score based on stale reviews that no longer apply to the current shipped state.

The code correctly computes `rolled_back` and defect-escape status by comparing rollback time to ship time, but it does not use rollback timestamps to filter RFP/curation data for current-state metrics.

**Impact:**

- `human_minutes_per_phase` can be materially inflated after a rollback because it measures from the first pre-rollback review to the latest re-ship.
- `metrics history` can show finding counts and dropped counts that include stale, invalidated review rounds.
- `finding_count_agreement` can fire or suppress alerts based on reviewer behavior in a prior invalidated epoch.
- This conflicts with the P9 rollback model where re-open invalidates affected phases and resets review rotation.

**Suggested fix:**

- For current shipped-state metrics, derive a per-phase current epoch boundary: latest `RollbackEvent.invalidated_phase == phase_id` before the current shipped disposition.
- Include only RFPs and curation records created after that boundary and before/at the current ship time where appropriate.
- If P10a intentionally wants lifetime metrics rather than current-state metrics, expose that explicitly in names/output, e.g. `lifetime_finding_count` vs. `current_epoch_finding_count`.
- Add regression tests for: ship → rollback → new review → re-ship; metrics should ignore pre-rollback RFPs for current history/minutes/agreement.

---

## 2. High — `ReviewerFindingPacket.phase_id` is not a normalized phase ID, so P10a groups phase-review rounds separately by round and includes non-phase artifacts in “phase” metrics

**Location:**

- `crates/anvil-audit/src/records.rs:184-214` (`ReviewerFindingPacket::from_packet`)
- `crates/anvil-cli/src/phase.rs:354-358` (phase RFP persistence)
- `crates/anvil-cli/src/charter.rs:203-207` and `crates/anvil-cli/src/plan.rs:372-376` (non-phase RFP persistence)
- `crates/anvil-eval/src/lib.rs:148-188` (`compute_finding_count_agreement`)
- `crates/anvil-eval/src/lib.rs:190-224` (`compute_human_minutes`)
- `crates/anvil-eval/src/lib.rs:329-434` (`compute_history`)

**Problem:**

P10a treats `ReviewerFindingPacket.phase_id` as the grouping key for phase metrics:

```rust
phase_counts.entry(rfp.phase_id).or_default().push(rfp.finding_count);
```

But the current audit record field is not consistently a normalized Plan phase ID (`P8`, `P9`, etc.). For phase reviews, the CLI writes:

```rust
ReviewerFindingPacket::from_packet(
    format!("{artifact_ref_prefix}:R{round_number}"),
    packet.clone(),
    cross_refs.clone(),
)
```

where `artifact_ref_prefix = "phase:{phase_id}"`, so persisted RFP `phase_id` values look like `phase:P8:R1`, `phase:P8:R2`, etc.

For Charter and Plan reviews, the same field stores values like `charter-R1` and `plan-R1`.

Consequences:

- Multiple review rounds for the same phase are grouped as separate “phases” (`phase:P8:R1` vs `phase:P8:R2`) instead of one phase (`P8` or `phase:P8`).
- `compute_human_minutes` and `compute_history` try to match RFP `phase_id` values against `PhaseDisposition.phase_id`, which is the normalized phase ID (`P8`). That means phase RFPs usually do **not** match shipped dispositions, so human-minutes can stay `None`/`-` even when phase review data exists.
- `compute_finding_count_agreement` can include Charter and Plan review packets as if they were phases, because it groups every RFP record without filtering to `phase:` artifact refs.

**Impact:**

- Human-minutes per phase is likely not computed for real phase workflows.
- Finding-count agreement is fragmented per phase round and polluted by Charter/Plan review packets.
- Metrics claiming “per shipped phase” are not actually using a stable phase identity.

**Suggested fix:**

- Normalize phase identity in one place. For P10a metrics, parse phase IDs from `rfp.packet.artifact_ref` values of the form `phase:<id>:R<n>` or store a true normalized phase ID in `ReviewerFindingPacket` for phase artifacts.
- Filter P10a phase metrics to phase artifacts only when computing Build/Ship phase metrics.
- Keep Charter/Plan review metrics separate if desired; do not mix them into phase-level agreement/minutes.
- Add tests using realistic RFP values from `run_phase_review` (`phase:P8:R1`, packet artifact_ref `phase:P8:R1`) plus a `PhaseDisposition("P8", "shipped")`; `compute_human_minutes` should produce data for P8.

---

## 3. Medium — Reviewer precision can be driven to 0 by stale or duplicate curation records because drops are summed globally without packet de-duplication or linkage validation

**Location:**

- `crates/anvil-eval/src/lib.rs:118-146` (`compute_reviewer_precision`)
- `crates/anvil-audit/src/records.rs:496-523` (`CuratedFindingsRecord`)

**Problem:**

R2 correctly removes the arbiter-override term, but `compute_reviewer_precision` still sums every `Drop` action across every `CuratedFindingsRecord` in the store:

```rust
let total_dropped: u32 = curated_entries
    .iter()
    .filter_map(|e| { ... count Drop actions ... })
    .sum();

let upheld = total_findings.saturating_sub(total_dropped);
```

There are two remaining problems:

1. The drop count is not validated against known RFP packet IDs. Any stale or orphan `CuratedFindingsRecord` whose `packet_id` no longer maps to a known RFP still contributes drops, while `total_findings` comes only from current RFP records.

2. Duplicate curation records for the same packet are all counted. Because the audit store is append-only, repeated curation of a packet or a retry that appends another `CuratedFindingsRecord` can double-count drops for the same logical findings.

The formula uses `saturating_sub`, so over-counted drops collapse precision to 0 rather than exposing the inconsistency.

**Impact:**

- Reviewer precision can be understated, potentially triggering false low-precision alerts.
- Audit-store duplicate/stale records affect metrics silently.
- This is especially likely in append-only workflows where retries create additional records rather than overwriting old ones.

**Suggested fix:**

- Build a set/map of known RFP `packet.packet_id` values and ignore curation records that do not reference a known packet.
- For multiple curation records referencing the same packet, decide and document an authority rule: latest curation wins, first curation wins, or all curation records are intentionally additive.
- Prefer counting dropped finding IDs as a set keyed by `(packet_id, finding_id)` to avoid double-counting duplicate records.
- Add tests for duplicate curation records and orphan curation records.

---

## 4. Medium — `avg_round_count` averages all `ConvergenceDeclaration` records, mixing Charter/Plan convergence with phase review rounds

**Location:**

- `crates/anvil-eval/src/lib.rs:227-249` (`compute_avg_round_count`)
- `crates/anvil-cli/src/arbiter.rs:74-84` (`ConvergenceDeclaration::new` call)
- `crates/anvil-eval/src/lib.rs:380-389` (`compute_history` round linkage)

**Problem:**

`compute_avg_round_count` reads every `ConvergenceDeclaration` record in the store and averages `round_count`:

```rust
let entries = store.list(RecordType::ConvergenceDeclaration)?;
...
total += decl.round_count;
count += 1;
```

`ConvergenceDeclaration.phase_id` is actually populated with the artifact argument to `anvil arbiter declare-convergence`, e.g. `charter.md`, `ANVIL_PLAN.md`, or potentially `phase:P8`. The metric is documented as “review rounds per phase” / “avg review rounds per phase”, but this implementation mixes all artifact convergence declarations.

`compute_history` then tries to attach declarations to shipped phases by exact key match:

```rust
phase_rounds.insert(decl.phase_id, decl.round_count);
...
review_rounds: phase_rounds.get(phase_id).copied(),
```

If the convergence declaration was recorded for `phase:P8` but `PhaseDisposition.phase_id` is `P8`, the round count will not appear in history. If declarations were recorded for `charter.md` or `ANVIL_PLAN.md`, they still influence the global `avg_round_count` even though they are not Build-phase shipped phases.

**Impact:**

- `Avg Review Rounds` can reflect Charter/Plan convergence instead of phase loop convergence.
- `metrics history` can show missing rounds for shipped phases despite convergence declarations existing under artifact refs.
- Layer-2 target evaluation for round count may warn/pass based on mixed artifact types.

**Suggested fix:**

- Decide whether `avg_round_count` is project-wide across all artifacts or phase-only. The Plan text and CLI label currently imply phase-oriented data.
- If phase-only, filter/normalize `ConvergenceDeclaration.phase_id` to shipped phase IDs before averaging.
- Normalize phase artifact refs (`phase:P8`, `phase:P8:R1`) to `P8` consistently before joining with `PhaseDisposition.phase_id`.
- Add tests with both `charter.md` and phase convergence declarations to ensure only intended records contribute.

---

## 5. Medium / Low — R2 fixes are not covered by targeted regression tests

**Location:**

- `crates/anvil-eval/src/lib.rs:118-146` (`compute_reviewer_precision`)
- `crates/anvil-eval/src/lib.rs:148-188` (`compute_finding_count_agreement`)
- `crates/anvil-eval/src/lib.rs:492-516` (`evaluate_targets` direction suppression)
- `crates/anvil-eval/src/lib.rs:709-718` (`DeferralOpenTooLong` message)
- `Review Rounds/REVIEW_P10A_EVAL_METRICS_R2.md:103-121`

**Problem:**

R2 states “Test Coverage (unchanged from R1).” That means the four R1 fixes have no new targeted regression tests:

- No test proves reviewer precision no longer reads/subtracts `ArbiterFindingResolution` records.
- No test asserts the public row name and alert message use “Finding Count Agreement.”
- No test asserts direction remains flat for two shipped phases and changes only when `history.len() >= 3`.
- No test asserts the deferral alert message includes the v1 false-positive note.

The existing tests are broad smoke tests and simple alert tests. They would not catch regressions for most of the R2-specific changes.

**Impact:**

- The exact issues fixed in R2 can regress silently.
- The code review doc is the only place pinning several behavior changes.
- P10a has metric formulas with non-trivial semantics but very limited executable coverage.

**Suggested fix:**

- Add targeted unit tests for each R2 fix.
- For precision, create a store with one RFP, one curation drop, and one arbiter resolution for the same finding; assert only the curation drop affects precision.
- For direction suppression, call `evaluate_targets` with two and three `PhaseMetrics` entries and assert the direction symbols/enum values.
- For the deferral note, seed enough shipped history and a `ProvisionalLock`; assert the alert message contains “v1 note.”

---

## 6. Low — `MetricTargets` accepts invalid numeric threshold values without validation

**Location:**

- `crates/anvil-core/src/config.rs:181-233` (`MetricTargets`)
- `crates/anvil-core/src/config.rs:235-240` (`load_config` docs)
- `crates/anvil-eval/src/lib.rs:518-580` (`evaluate_targets`)

**Problem:**

`MetricTargets` fields are raw `f64` values loaded from `anvil.toml`. There is no validation that percentage thresholds are within `0.0..=1.0`, that maximum values are non-negative, or that values are finite.

Examples that appear accepted by the model:

```toml
[metric_targets]
precision_min = 1.5
agreement_max = -0.2
human_minutes_max = -10
round_count_max = 0
escape_rate_max = 2.0
```

These values produce nonsensical targets and statuses rather than configuration errors.

**Impact:**

- Misconfigured projects can receive misleading pass/fail results.
- Layer-2 evaluation may be impossible to satisfy or trivially satisfied.
- This is low risk for the default config, but important once users edit thresholds.

**Suggested fix:**

- Extend config validation to check:
  - `precision_min`, `agreement_max`, and `escape_rate_max` are finite and in `0.0..=1.0`;
  - `human_minutes_max` and `round_count_max` are finite and non-negative, ideally positive.
- Add config parse/validation tests for invalid thresholds.

---

## Overall Assessment

R2 successfully applies the four R1 review dispositions and the workspace validates cleanly. The remaining concerns are mostly deeper semantic issues in how P10a metrics map audit records to current shipped phases. The most important fixes are to normalize phase identities and apply rollback epoch boundaries before treating metrics as authoritative current-state phase metrics.
