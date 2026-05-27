# Anvil — P11 Dogfooding R8 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R8.md`  
**Review date:** 2026-05-27  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- Header match for section-scoped PL extraction: `## Locked Required Project-Level Choices` exists at ANVIL_PLAN.md:180 and all 8 "Final (P11)" rows are contained within it.
- Gate 1 / Gate 2 split present in ANVIL_PLAN.md:1135-1159.
- p11.rs parser now limits extraction to the Required Choices section (lines 44-58) and uses `map_or`.
- Contract test comment updated to "RPC-name presence smoke test" (p11.rs:113).
- Dogfooding and external-pilot READMEs rewritten with representative/conditional framing and top-level notices.
- No remaining instances of the previously contradictory phrases ("after P11", "produced the first observational baselines").

---

## 1. High — Gate 1 "Status: Complete" still includes representative artifacts as an acceptance criterion while the attestation explicitly disclaims live execution

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:1148`
- `Anvil Plan/ANVIL_PLAN.md:1159`
- `docs/examples/coordinator-attestation.md:58-73`

**Problem:**

Gate 1 criterion 10 states:

> Representative dogfooding and external pilot artifacts provided with Coordinator attestation (see `docs/examples/coordinator-attestation.md`).

**Status: Complete.**

Yet the attestation and the updated READMEs repeatedly emphasize that these artifacts are *not* live CLI output against real providers and that live execution is deferred to Gate 2. The R8 changes improved tense and added framing notices, but the normative AC still treats "representative artifacts + attestation" as sufficient to mark the dogfooding acceptance criterion "Complete." This is the same logical category error the R7 findings identified, only now localized to a single bullet instead of the header.

**Impact:**

- Future readers or auditors can interpret "P11 Accepted" as having satisfied the dogfooting acceptance test when the project itself documents that the actual test (live run) has not occurred.
- The distinction between "implementation complete" and "public-ship evidence complete" remains blurred inside the very gate that claims completion.
- The attestation file is now the single source of truth for the deferral, but it is referenced rather than authoritative for the AC status.

**Suggested fix:**

- Remove the representative-artifact criterion from Gate 1 entirely, or re-label it as "documentation complete" rather than an acceptance gate for the dogfooding requirement.
- Make the attestation itself carry an explicit "Gate 1 satisfied via representative only; live evidence required for Gate 2" declaration that is machine-checkable or at least unambiguous.

---

## 2. Medium — Section-scoped PL parser still uses fragile `starts_with` + next-`## ` heuristics with no normalization or fallback

**Location:**

- `crates/anvil-cli/src/p11.rs:48-58`

**Problem:**

The fix scopes extraction correctly for the current document, but the implementation remains:

```rust
let section_start = lines.iter().position(|line| line.starts_with("## Locked Required Project-Level Choices"))
    .expect(...);
let section_end = lines[section_start + 1..].iter().position(|line| line.starts_with("## "))
    .map_or(...);
```

No trimming, no case-insensitive match, no allowance for trailing whitespace or alternative header styles. The `.expect` message is an improvement, yet the parser can still silently produce an empty or wrong slice if the header is ever reformatted (e.g., `##Locked...`, `### Locked...`, or a line with trailing spaces).

**Impact:**

- A future documentation edit that does not touch the test list can break the hinge test without an obvious root cause.
- The "bidirectional sync" claim in the test comment is only as strong as this brittle extraction step.

**Suggested fix:**

- Use a more robust section parser (e.g., skip until a line that trims to the exact header, or require the header to be followed by a table row or blank line).
- Add a unit test that the extracted slice actually contains the expected number of table rows before slug extraction.

---

## 3. Medium — "RPC-name presence smoke test" rename documents the weakness but does not improve test strength or add negative-case coverage

**Location:**

- `crates/anvil-cli/src/p11.rs:113-116`
- AC table in R8 briefing (AC3)

**Problem:**

The comment update correctly labels the test as a "presence smoke test" and explicitly disclaims schema validation. However, the test still only verifies that every `rpc Foo(` name appears somewhere in contract.md. It performs no check that the documented signatures match, that no extra RPCs exist in the doc, or that field numbers / types are consistent. The R8 resolution only changed wording.

**Impact:**

- The test will pass even if the contract doc is wildly out of date on request/response shapes, package name, or service definition — exactly the class of drift the original finding was concerned about.
- The AC table still marks this "PASS" without acknowledging that the enforcement remains intentionally minimal until v1.1.

**Suggested fix:**

- Either strengthen the test to at least verify service name and a representative request/response type per RPC, or change the AC language to "smoke test only; full schema sync is a v1.1 task" so the PASS label is not misleading.

---

## 4. Low — R8 briefing header still uses the imprecise "all findings addressed" phrasing that R7 F6 was created to correct

**Location:**

- `Review Rounds/REVIEW_P11_DOGFOODING_R8.md:11-13`

**Problem:**

The R8 header states "Full P11 R7 finding responses — all 6 findings addressed" and then notes that F6 was "addressed by correcting briefing language." This is the exact pattern the R7 F6 finding identified as imprecise. The disposition table now distinguishes "Applied" vs "Addressed in briefing language," which is better, but the top-line summary still collapses everything into "all findings addressed."

**Impact:**

- The meta-problem of over-claiming resolution in review briefings recurs in the very document that claims to have fixed it.
- Future rounds may repeat the same wording shortcut.

**Suggested fix:**

- Change the R8 header line to "5 findings addressed via code/doc changes; 1 addressed via briefing-language correction only" so the summary itself cannot be misread as "all code fixes applied."

---

**End of findings.**