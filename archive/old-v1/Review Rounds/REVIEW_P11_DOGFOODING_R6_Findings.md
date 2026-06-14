# Anvil — P11 Dogfooding R6 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R6.md`  
**Review date:** 2026-05-27  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first. This review examines whether the cumulative fixes have produced robust, maintainable deliverables or merely layered more documentation and weak assertions.

## Validation Performed

- `cargo build --workspace` — **passes**
- `cargo test --workspace` — **passes** (190 tests)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **passes**
- `cargo fmt --all -- --check` — **passes** (verified after the R5 fmt run)
- `go test ./...` from `C:\Anvil\sidecar` — **passes**

---

## 1. High — PL parser in `test_no_outstanding_provisional_locks_after_dogfooding` is brittle and tied to a very specific Markdown table format

**Location:**

- `crates/anvil-cli/src/p11.rs:47`–`76` (the `plan_slugs` extraction logic added in R5/R6)

**Problem:**

The parser filters lines containing the literal string `"**Final (P11)**"`, splits on `|`, and extracts the first backtick-delimited token from the second column. This works for the current table shape but will silently produce wrong results (empty list, wrong count, or missed slugs) if:

- The Plan ever changes the exact bold marker (e.g., `**Final (P11)**` → `**Final** (P11)` or `Final (P11)`).
- The table column order changes.
- A PL row uses a different status phrasing or is split across multiple lines.
- The backtick convention for slugs is altered.

The test now performs a real cross-check, which is an improvement, but the extraction logic is a heuristic that has no tolerance for the normal evolution of Markdown tables.

**Impact:**

- Future Plan edits that are semantically correct can cause the P11 hinge test to fail or (worse) pass with an incomplete slug set.
- The "runtime match" claim in AC5 is only as strong as this fragile parser.

**Suggested fix / improvement:**

- The parser should be made more robust (e.g., using a small Markdown table library or at least normalizing whitespace and bold markers) or the test should fall back to a documented manual convention if the parser cannot confidently extract the list. The current implementation trades one form of fragility (hard-coded list) for another (format-dependent extraction).

---

## 2. High — `test_contract_doc_sync_method` implementation still only performs a single-string presence check; the improved comment does not change the weakness

**Location:**

- `crates/anvil-cli/src/p11.rs:80`–end (the `include_str!` + `contains(...)` assert)
- R6 F5 updated only the comment, not the code.

**Problem:**

The R6 fix improved the comment to accurately describe the test as an "RPC-name coverage check" that explicitly disclaims coverage of service names, message types, fields, enums, etc. However, the actual test code remains unchanged: it only verifies that one sentence exists in the document. No RPC names are actually extracted from the proto and compared.

The comment now correctly labels a weak test; it does not make the test stronger.

**Impact:**

- The hinge test gives the appearance of having addressed the drift-detection concern while the implementation remains a token presence check.
- Anyone reading only the test code (or running the test) will still receive almost no protection against actual contract drift.

**Suggested fix / improvement:**

- Either implement a real extraction-based comparison (as the comment itself says is a v1.1 task) or accept that the test is purely documentary and remove the `include_str!` machinery. Labeling a weak test accurately is better than before, but does not constitute a substantive fix.

---

## 3. Medium — Large number of inline deferral notes added to `ANVIL_PLAN.md` and `new_project_charter.md` create visual noise and risk of future inconsistency

**Location:**

- Multiple locations updated in F2 and F3 (seven locations in ANVIL_PLAN.md, four in new_project_charter.md)

**Problem:**

R6 inserted parenthetical deferral notes, italicized AC notes, and inline explanations throughout the primary governance documents. While each individual note is accurate, the cumulative effect is a document that now contains many "*(deferred with attestation…)*" asides. These notes are easy to miss during future edits and can become stale independently.

**Impact:**

- The Plan and Charter are harder to read.
- Future maintainers may update one note but miss another, recreating the inconsistency problems that earlier rounds attempted to fix.
- The normative documents now embed implementation-history commentary that belongs in the hardening history.

**Suggested fix / improvement:**

- Move the detailed deferral explanations to a single "P11 Status and Deferrals" section or appendix, and keep only short cross-references in the main text. The current approach of scattering many similar notes increases maintenance surface area.

---

## 4. Medium — AC5 claim now reads "Hinge test asserts PL count and slugs match Required Choices table (runtime)" but the match is only one-directional and parser-dependent

**Location:**

- R6 AC table (AC5)
- `p11.rs:69`–`75` (the `any(|s| s == slug)` check)

**Problem:**

The test asserts that every slug in the hard-coded list appears in the Plan-extracted list and that the counts match. It does **not** assert the converse (that every slug extracted from the Plan appears in the hard-coded list). A new PL added to the Plan with the "**Final (P11)**" marker would increase the extracted count, causing the length assertion to fail — but only if the parser successfully finds it. The "match" is therefore partial and still relies on the brittle extraction logic.

**Impact:**

- The AC5 wording overstates the strength of the automated check. The test provides a useful guard but is not a full bidirectional synchronization.

**Suggested fix / improvement:**

- The test could collect the symmetric difference and report any Plan-only slugs as well. The current one-directional check is an improvement over pure hard-coding but does not fully deliver the "match" language in the AC table.

---

## 5. Low — Smoke-test correction in F6 may have introduced a new incorrect expectation for a freshly initialized project

**Location:**

- `ANVIL_PLAN.md` smoke-test step (updated in R6 F6)

**Problem:**

The fix changed the expectation from "non-zero exit" to "exit 0 (count of `0` is expected for a freshly initialized project with no source annotations)." This is correct for a brand-new project. However, the smoke test is intended to be run on a real release artifact. If the release process ever includes running the test inside a directory that already contains source with hinge annotations (e.g., the Anvil repo itself), the count will be non-zero and the test will still pass — but the documented expectation will be wrong.

**Impact:**

- The smoke-test documentation now assumes a clean checkout. This may not match how the test is actually executed during a real release.

**Suggested fix / improvement:**

- The smoke-test description should note that the count is expected to be zero only in a fresh checkout with no source annotations, and that a non-zero count on a real project is also acceptable provided it matches the number of annotations present.

---

## Summary of R6 Code Health

- R5/R6 made several substantive improvements (real Plan parser in the PL hinge test, more accurate comments, corrected smoke-test expectation, leading representative-artifact notices in the example READMEs).
- However, the parser is format-brittle, the contract test remains a weak presence check despite better labeling, the Plan documents are now cluttered with many similar deferral notes, and AC5 still overstates the bidirectional nature of the match.
- The cumulative effect of the rounds is a more heavily documented and annotated codebase, but the core governance mechanisms (automated contract drift detection, robust PL/Plan synchronization, live evidence for the dogfooding ACs) remain deferred or implemented at a heuristic level.

No new compilation, clippy, or test failures were introduced, but several of the "fixes" are thinner or more fragile than the briefing claims.