# Anvil — P11 Dogfooding R12 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R12.md`  
**Review date:** 2026-05-27  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- Gate 2 item lists now textually identical between P11 section (833-837) and Plan-Level section (1158-1161).
- p11.rs slug extraction now uses `line.trim()` + `s.trim().to_string()`; header matching uses `trim() ==` and `trim().starts_with`.
- coordinator-attestation.md updated to enumerate Gate 2 AC1–AC3 commitments.
- v11-charter.md and audit-store-summary.EXAMPLE.json updated to representative language.
- Remaining "v1 usage" / "actual v1 usage" phrasing found in ANVIL_PLAN.md:823 and :841 despite F5 claims.

---

## 1. High — F5 "v1.1 evidence language" cleanup missed two instances that still claim "v1 usage" data exists

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:823` (P11 action list)
- `Anvil Plan/ANVIL_PLAN.md:841` (P11 Evaluation-metric impact)

**Problem:**

F5 response updated five locations to replace unqualified "v1 usage" / "real CLI usage data" with "Gate 2 live usage" qualifiers. Two instances were left unchanged:

- Line 823: "evaluated against the CLI UX audit output and actual v1 usage and confirmed Final."
- Line 841: "Baseline values for all six metrics established from v1 usage; used to validate or revise Layer-2 numeric targets."

These directly contradict the intent of F5 and the updated AC table language that treats all live usage evidence as Gate 2 deferred.

**Impact:**

- The Evaluation-metric impact paragraph (immediately below the P11 AC block) still presents Layer-2 baselines as already derived from real v1 usage when the project simultaneously states that such data will only come from Gate 2.
- A reader of the P11 section can reasonably conclude that "actual v1 usage" data already informed the Final confirmation of the two v1.1-prep locks.
- The R12 briefing's claim that "Five locations updated" is factually incomplete.

**Suggested fix:**

- Change both instances to "build observations and UX audit output" (consistent with the other F5 edits) or add the "Gate 2 live" qualifier.

---

## 2. Medium — Hinge test name still contains the word the entire review series has been systematically removing from documentation

**Location:**

- `crates/anvil-cli/src/p11.rs:5` (test attribute comment)
- `crates/anvil-cli/src/p11.rs:840` (P11 hinge-test list in ANVIL_PLAN.md)

**Problem:**

The test is named `test_no_outstanding_provisional_locks_after_dogfooding`. Every other reference to "dogfooding" in the context of live execution has been qualified as "representative," "deferred," or replaced with "build observations." The test name and its `// hinge_test:` pin comment remain untouched. R12 F3 cleaned prose references but did not touch the canonical test identifier.

**Impact:**

- The test name is part of the public hinge registry (`anvil hinge list` output) and appears in PLAN_HARDENING_HISTORY.md and cross-reference comments.
- It perpetuates the exact terminology the R7–R12 series has treated as misleading.
- Any future automated scan for "dogfooding" language will continue to surface this as a live instance.

**Suggested fix:**

- Rename the test to `test_no_outstanding_provisional_locks_at_p11_gate1` (or similar) and update the hinge pin comment and all references in the Plan and history files.

---

## 3. Medium — AC5 claims full "trim-normalized (header + slug)" but the "Final (P11)" filter still operates on the untrimmed line

**Location:**

- `crates/anvil-cli/src/p11.rs:69`

**Problem:**

The R11/R12 change added trimming to the `split('|')` path and to the extracted slug. However, the preceding filter still does:

```rust
.filter(|line| line.replace("**", "").contains("Final (P11)"))
```

This filter runs before any `.trim()` on the line. A table row with leading whitespace (possible after certain markdown renderers or copy-paste) would fail the `contains` check even though the later extraction path would have trimmed it. The normalization story is therefore incomplete.

**Impact:**

- The AC5 description "trim-normalized (header + slug)" overstates the current implementation.
- The bidirectional contract can still be broken by cosmetic whitespace that only affects the filter, not the extraction.

**Suggested fix:**

- Move the `trim()` earlier: `.filter(|line| line.trim().replace("**", "").contains("Final (P11)"))` so the entire pipeline is consistently trim-normalized.

---

## 4. Low — Release-time smoke-test script (Gate 2 AC4) has no tracking entry in Open Items or PLAN_HARDENING_HISTORY.md

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:837` and Plan-Level Gate 2 item 4
- Open Items section (no matching entry)

**Problem:**

Gate 2 AC4 now explicitly lists the release archive, signed checksum, and smoke-test script as deferred release-time work. No corresponding Open Item or Amendment entry tracks the script's eventual creation, the commands it must exercise, or the platforms it must cover. All other deferred items have at least a one-line placeholder.

**Impact:**

- The release-time requirement is now a formal Gate 2 criterion but has zero visibility in the project's tracking artifacts.
- Future release engineering will have to rediscover the requirement from the AC table alone.

**Suggested fix:**

- Add a one-line Open Item entry: "Release-time smoke-test script (Gate 2 AC4) — commands limited to v1 binary surface; written at `anvil release` time."

---

**End of findings.**