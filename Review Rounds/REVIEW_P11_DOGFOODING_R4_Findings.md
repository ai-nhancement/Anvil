# Anvil — P11 Dogfooding R4 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R4.md`  
**Review date:** 2026-05-27  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first. This review examines whether the R3/R4 fixes actually strengthened the deliverables or merely papered over symptoms.

## Validation Performed

- `cargo build --workspace` — **passes**
- `cargo test --workspace` — **passes** (190 tests)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **passes**
- `cargo fmt --all -- --check` — **passes**
- `go test ./...` from `C:\Anvil\sidecar` — **passes**

---

## 1. Critical — `test_contract_doc_sync_method` is a weak string-presence check that provides almost no actual drift protection

**Location:**

- `crates/anvil-cli/src/p11.rs:41`–`46` (the `include_str!` + `contains("Automated drift detection is a v1.1 task")` assert)

**Problem:**

The R3 fix replaced a tautological `assert_eq!("manual-sync", "manual-sync")` with an `include_str!` that only verifies the continued presence of one specific sentence in `docs/contract.md`. This test will pass even if:

- The entire contract content is replaced with incorrect information, as long as the sentence remains.
- The protobuf changes and the Markdown drifts in every other respect.
- The maintenance note is moved, reworded, or placed inside a code block.

The test does not compare any actual RPC, message, or field definitions. It is a social reminder, not an automated guard.

**Impact:**

- The hinge test gives a false impression that the contract document is protected against drift. In reality, the only thing preventing silent divergence is human discipline.
- The comment in the test itself acknowledges that real CI enforcement is a v1.1 task — meaning the current test adds negligible value over the previous tautology.

**Suggested fix / improvement:**

- Either remove the test (and accept that contract maintenance is a human process) or implement a meaningful extraction-based comparison. The current form is cosmetic and should not be presented as a meaningful R3 improvement.

---

## 2. High — Coordinator attestation document acknowledges the core constraint but does not provide the "evidence" the Plan ACs actually require

**Location:**

- `docs/examples/coordinator-attestation.md` (new in R3)
- R4 AC table entries for plan-level AC2 and AC3 marked "PASS (attested)"

**Problem:**

The attestation document explicitly states that live AI-provider dogfooding and a real external pilot with multi-reviewer rotation were not performed because the system was being built at the same time. It then lists what *was* validated (190 tests, CLI surfaces, record types, hinge framework) and commits to live evidence "before public announcement."

The R4 briefing now marks the plan-level AC2/AC3 as "PASS (attested)" with a reference to this document. This is a redefinition of "PASS." The original AC language required actual execution of Charter → Plan → Build → Ship cycles via the v1 CLI against real providers. The attestation converts that into "we tested the scaffolding and will do the real work later."

**Impact:**

- The ship gate for P11 now rests on an attestation that the work described in the ACs was not completed.
- Future readers of the AC table will see "PASS" where the substantive requirement was waived.

**Suggested fix / improvement:**

- The AC table should reflect the honest status: "Deferred with attestation; live evidence required before public ship." Labeling it "PASS (attested)" misrepresents the state of the deliverables.

---

## 3. High — Hinge test still hard-codes PL slugs with a comment claiming they are extracted from the Plan table; no automated verification exists

**Location:**

- `crates/anvil-cli/src/p11.rs:9`–`14` (comment: "The strings below are the canonical choice_key slugs from the Required Choices table in ANVIL_PLAN.md")

**Problem:**

The test continues to maintain an explicit list of eight strings and asserts their count. The comment claims these are the slugs that appear in the Plan table. While some slugs do appear in parentheses, the test provides no runtime check that the list remains in sync with the table. Adding, removing, or renaming a PL in the Plan requires a manual, error-prone edit to the test.

The R3/R4 fixes did not add any parser or cross-check; they only updated the list and the test comment.

**Impact:**

- The "AC5: Hinge test asserts PL count and slugs match Required Choices table" claim in the R4 AC table is aspirational. The test asserts a count and a list; it does not assert a match against the live Plan.

**Suggested fix / improvement:**

- Either accept the test as a manual convention (and remove the misleading comment) or implement a small Markdown table parser that extracts the PL slugs from `ANVIL_PLAN.md` at test time. The current state is no stronger than it was before the critical findings.

---

## 4. Medium — `include_str!` path is brittle and the test will silently fail to compile if the module is ever moved

**Location:**

- `crates/anvil-cli/src/p11.rs:41` (`include_str!("../../../docs/contract.md")`)

**Problem:**

The relative path is hardcoded three directories up from `src/p11.rs`. If the `p11` module is ever reorganized, moved into a subdirectory, or if the test is relocated, the path breaks at compile time. More importantly, the test only runs under `cargo test`; a normal `cargo build` or `cargo clippy` will not execute it, so path drift can go undetected until the next test run.

**Impact:**

- The guard is fragile and its failure mode (compile error on a moved test) is not the intended "drift detected" signal.

**Suggested fix / improvement:**

- Use `env!("CARGO_MANIFEST_DIR")` + `Path::new(...)` construction so the path is relative to the crate root rather than the source file. The current form is an easy source of future breakage.

---

## 5. Medium — Record-type count reconciliation (15 vs 16) updated several documents but left the constitutional hinge test comment and related Plan prose potentially stale

**Location:**

- `new_project_charter.md`, `GOVERNANCE.md`, `CHARTER_HARDENING_HISTORY.md`, `PLAN_HARDENING_HISTORY.md` (R3 changes)
- `test_audit_store_required_types_present` (the constitutional hinge)

**Problem:**

R3 F2 corrected the total count from 16 to 15 and documented the three deferred A1 types. However, the constitutional hinge test comment and several Plan paragraphs that previously referenced "16 record types" or the A1 amendment may still contain outdated wording in less-visible sections. The review only spot-checked the most prominent locations.

**Impact:**

- Subtle inconsistencies can remain in secondary sections, creating future confusion about the exact v1 audit-store contract.

**Suggested fix / improvement:**

- A global search for "16" combined with "record" or "audit" across the entire `Anvil Plan/` and `docs/` trees should have been performed and any remaining references updated. The R3 fix appears to have been targeted rather than exhaustive.

---

## Summary of R4 Code Health

- The R3/R4 fixes addressed the most visible symptoms raised in prior rounds, but several underlying weaknesses remain: the contract-sync test is cosmetic, the PL hinge test still relies on manual synchronization with the Plan table, the attestation redefines "PASS" for AC2/AC3, and the `include_str!` guard is path-fragile.
- The pattern across rounds continues to be "add a document or a weak assert to satisfy the checklist item" rather than implementing robust, automated verification.
- The P11 deliverables are in a better-documented state than at R1, but the critical governance and drift-detection mechanisms the reviewers requested are still missing or implemented at a token level.

No new correctness or compilation issues were found, but the substantive improvements claimed in the R4 briefing are thinner than presented.