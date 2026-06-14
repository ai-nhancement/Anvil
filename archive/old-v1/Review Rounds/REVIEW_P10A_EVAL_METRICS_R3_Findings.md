# Anvil — P10a Eval Metrics R3 Findings

**Source review doc:** `Review Rounds/REVIEW_P10A_EVAL_METRICS_R3.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo build --workspace` — **passes**
- `cargo test --workspace` — **passes** (175 tests)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **passes**
- `cargo fmt --all -- --check` — **passes**
- `go test ./...` from `C:\Anvil\sidecar` — **passes**

---

## 1. High — Epoch boundary definition (`created_at > es && created_at <= ship_time`) is correct and matches P9 rollback model

**Location:**

- `crates/anvil-eval/src/lib.rs:152` (`phase_epoch_starts` skips `rb.created_at >= ship_time`)
- `crates/anvil-eval/src/lib.rs:173` (`rfp_in_epoch`: strict `> es`)
- `Review Rounds/REVIEW_P10A_EVAL_METRICS_R3.md` §"What to Review" item 1

**Problem / Confirmation:**

The implementation excludes any record whose timestamp is exactly the rollback instant (`created_at > es`) while including records created at or before the ship disposition (`<= ship_time`). This matches the documented intent: "records created exactly at the rollback time are excluded."

Because P9 writes the `RollbackEvent` before new review activity begins, a record with `created_at == rollback.created_at` belongs to the invalidated epoch and must be excluded. The strict-greater comparison achieves this without requiring a sentinel or inclusive start.

**Impact:**

- Correct scoping of review rounds, human minutes, and finding counts to the post-rollback epoch.
- Prevents pre-rollback data from inflating `avg_round_count` or `human_minutes_per_phase` after a reopen.

**Suggested fix / improvement:**

- No change required. The boundary is the right engineering choice for the P9 model. Add a one-line comment in `rfp_in_epoch` restating the rationale for the strict `>` operator.

---

## 2. High — Phase ID normalization via `phase_id_from_artifact_ref` is applied at all five compute sites plus history

**Location:**

- `crates/anvil-eval/src/lib.rs:135` (helper)
- Usage sites: `compute_reviewer_precision:217`, `compute_finding_count_agreement:262`, `compute_human_minutes:318`, `compute_avg_round_count:372`, `compute_history:493` and `537`
- `Review Rounds/REVIEW_P10A_EVAL_METRICS_R3.md` §"What to Review" item 2

**Problem / Confirmation:**

The helper is invoked in every function that joins RFP or `ConvergenceDeclaration` records against the `PhaseDisposition` shipped-phase map. Charter-level and Plan-level artifacts (no `"phase:"` prefix) are correctly filtered out by the `is_some()` guard. The five functions listed in the review document, plus the internal paths inside `compute_history`, all route through the helper.

**Impact:**

- Metrics no longer mix bare phase IDs with artifact-ref strings, eliminating the join failures that F2 addressed.
- Non-phase artifacts are silently excluded rather than producing `None` or panics.

**Suggested fix / improvement:**

- No action required. The normalization is complete and consistently applied. The helper is small, pure, and easy to audit.

---

## 3. Low — `latest_curations_for_packets` accepts `&HashSet<&str>` while packet IDs are `String`; minor lifetime friction

**Location:**

- `crates/anvil-eval/src/lib.rs:184` (signature)
- Call site: `compute_reviewer_precision:232` and `compute_history:514`

**Problem:**

The function takes `&HashSet<&str>` for `known_packets` but the RFP `packet_id` values are owned `String`s. Callers must therefore build a temporary set of `&str` references. This works but adds a small allocation and forces the known-packet collection to be built before the curation lookup.

**Impact:**

- Negligible at current scale; purely a code-style observation.
- No correctness or performance issue.

**Suggested fix / improvement:**

- Change the parameter to `&HashSet<String>` or `impl IntoIterator<Item = &str>` for ergonomics in future refactors. Not required for R3.

---

## 4. Low — `phase_epoch_starts` only populates epochs for phases that appear in the shipped map; rollbacks on never-re-shipped phases are ignored

**Location:**

- `crates/anvil-eval/src/lib.rs:149` (`if shipped.get(&rb.invalidated_phase).is_none() { continue }`)

**Problem:**

A `RollbackEvent` whose phase has no subsequent `PhaseDisposition` (i.e., the phase was reopened but never re-shipped) produces no entry in the epoch map. Such phases never contribute to any Layer-1 metric or history row, so the omission is harmless.

**Impact:**

- No observable effect on current metrics or alerts.
- The design correctly scopes epoch calculation to only the phases that matter for shipped-state reporting.

**Suggested fix / improvement:**

- No change needed. The guard is the natural consequence of computing epochs relative to shipped dispositions.

---

## Summary of R3 Code Health

- Both open review questions are confirmed: the epoch boundary (`> es && <= ship_time`) is correct, and phase-ID normalization is applied at every required site.
- All six R2 findings (rollback epoch filtering, normalization, precision deduplication via latest-curation + unique `(packet_id, finding_id)`, avg_round_count scoping, four new regression tests, and `MetricTargets::validate`) are implemented as described.
- New eval test count (14) matches the document.
- No correctness, clippy, or formatting issues. The R3 changes are high-quality and close the previous review items cleanly.