# Anvil — P11 Dogfooding R14 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R14.md`  
**Review date:** 2026-05-27  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- Leaflog description corrected to "houseplant watering journal CLI" in Open Items and action list.
- Smoke-test Open Item now matches Distribution scope (includes `anvil-sidecar --version`, init + hinge list commands).
- Five "v1 usage / P11 observational" phrases qualified with "Gate 2 live" (including choices.rs:181).
- Build Stage Outcome cells updated to "Would ship".
- p11.rs filter now uses `line.trim().replace("**", "")` — AC5 "fully trim-normalized (header + filter + slug)" holds.
- History note added for old smoke-test entry.

---

## 1. High — Canonical hinge test name still contains "dogfooding" and was never renamed despite R12 finding on terminology

**Location:**

- `crates/anvil-cli/src/p11.rs:5` (hinge pin comment)
- `Anvil Plan/ANVIL_PLAN.md:840` (P11 hinge-test list)
- `anvil hinge list` runtime output for any project containing the annotation

**Problem:**

R12 F2 explicitly called out the test name `test_no_outstanding_provisional_locks_after_dogfooding` as the last remaining unqualified use of the term the entire review series has been eliminating from documentation and prose. R13/R14 addressed Leaflog wording, smoke-test scope, and additional v1-usage qualifiers, but left the test identifier and its pin comment untouched. The name appears in the hinge registry, PLAN_HARDENING_HISTORY.md references, and cross-phase comments.

**Impact:**

- The terminology the project has spent seven rounds cleaning from every other artifact remains in the single most visible runtime artifact (`anvil hinge list --strict`).
- Any future "dogfooding language" audit or search will continue to surface this as a live instance.
- The R14 briefing's "all 5 applied" claim is accurate for the R13 findings but ignores the standing R12 issue that was never folded into subsequent rounds.

**Suggested fix:**

- Rename the test to `test_no_outstanding_provisional_locks_at_p11_gate1` (or equivalent) and update the `// hinge_test:` pin, the P11 hinge-test list entry, and all history references.

---

## 2. Medium — AC5 now claims "fully trim-normalized (header + filter + slug)" while the test remains a runtime-only bidirectional contract with no static enforcement

**Location:**

- `crates/anvil-cli/src/p11.rs:69-76`
- R14 AC table row for AC5

**Problem:**

The implementation is now consistently trim-normalized on header detection, filter predicate, and extracted slug. However, the AC language and the test comment present this as a robust "bidirectional synchronization" safeguard. In reality the contract is still enforced only at `cargo test` time via `assert_eq!` / `assert!` inside a single integration test. No compile-time constant, no build.rs generation, no CI step that fails the build if the list and table diverge before tests run.

**Impact:**

- The "fully trim-normalized" upgrade improves robustness against cosmetic edits but does not change the fundamental nature of the check (runtime, test-only, include_str! style).
- Reviewers and future maintainers may overestimate the strength of the synchronization guarantee.

**Suggested fix:**

- Update AC5 wording to "runtime bidirectional slug check with full trim normalization on header, filter, and extraction" so the limitation remains visible at the acceptance-criteria level.

---

## 3. Low — Smoke-test Open Item now references the Distribution command list, but the Distribution section itself still contains an older, broader smoke-test description that was never reconciled

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:1018` (Distribution smoke tests)
- `Anvil Plan/ANVIL_PLAN.md:1033` (current Open Item)

**Problem:**

The R13 fix aligned the Open Item to the Distribution list (`anvil --version`, `anvil-sidecar --version`, `anvil init`, `anvil hinge list --count`, `anvil.toml` verification). The Distribution paragraph at 1018 still opens with a longer historical list that includes `anvil phase build` / `anvil phase ship` / `anvil ship` scenarios and Windows-specific daemon tests. The Open Item now points to a "Distribution smoke-test command list" that is not cleanly extractable from the surrounding text.

**Impact:**

- The authoritative scope is now split between two paragraphs; a release engineer must mentally merge them.
- The inline correction note added to PLAN_HARDENING_HISTORY.md (F5) does not address this remaining divergence inside the normative Distribution section.

**Suggested fix:**

- Either factor the canonical command list into a single bullet or table that both the Open Item and the Distribution paragraph reference, or shorten the Distribution paragraph to the exact five-command list now used by the Open Item.

---

**End of findings.**