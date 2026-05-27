# Anvil — P11 Dogfooding R10 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R10.md`  
**Review date:** 2026-05-27  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- P11 phase ACs now split into explicit Gate 1 (lines 828-832) / Gate 2 (833-837) in ANVIL_PLAN.md.
- p11.rs now uses `line.trim() ==` and `line.trim().starts_with("## ")` for section bounds (trim-normalized header matching).
- New substring comment added to service-name assertion (p11.rs:155-156).
- Plan-Level Gate 2 now contains 3 criteria (release-archive item moved in); P11 Gate 2 lists 4.
- One instance of "dogfooding session" remains at ANVIL_PLAN.md:877.
- Representative artifact metadata updates (status, outcome, version fields) confirmed in the six files listed in F3.
- coordinator-attestation.md contains the Gate 1 "documentation only" statement.

---

## 1. High — Missed cleanup instance: "dogfooding session" still appears in Cross-Cutting Concerns after F5 claimed all such references were removed

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:877`

**Problem:**

F5 response stated five locations were updated to replace "P11 dogfooding" / "dogfooding" phrasing with "build observations" or "P11 build." The Required Choices table, v1.1 Design Seeds, Provider adapter roadmap, and PLAN_HARDENING_HISTORY.md were changed. However, the Cross-Cutting Concerns bullet for `cli-command-structure` still reads:

> confirmed Final at P11 after evaluation against `docs/ux-audit.md` and dogfooting session.

This is the exact language the finding targeted. The R10 briefing does not list this location among the updates.

**Impact:**

- The document remains internally inconsistent on whether live dogfooding occurred.
- A reader following the "all dogfooding references cleaned" claim in R9/R10 will encounter a counter-example on the same page as the P11 AC block.
- The change was mechanical string replacement in some places but not exhaustive.

**Suggested fix:**

- Replace the remaining "dogfooting session" with "build observations" (or "P11 build / UX audit evaluation") to match the other four locations.

---

## 2. High — Plan-Level Gate 2 (3 criteria) and P11 Gate 2 (4 criteria) are now out of sync after release-archive item was moved

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:1154-1162` (Plan-Level Gate 2)
- `Anvil Plan/ANVIL_PLAN.md:833-837` (P11 Gate 2)

**Problem:**

R9 moved the release-archive / SHA256SUMS / smoke-test requirement from Gate 1 criterion 9 into Plan-Level Gate 2 as item 3. The P11 section's Gate 2 was simultaneously expanded to four items, adding:

> 4. The v1.1 Plan from live dogfooding validated as the input for the v1.1 App design.

The Plan-Level Gate 2 list stops at three items and does not contain the v1.1 Plan validation criterion. The two normative lists that are supposed to define the same Gate 2 state are now divergent.

**Impact:**

- Which list is authoritative? A future reviewer or release engineer consulting only the Plan-Level section will see a different Gate 2 definition than the per-phase P11 section.
- The v1.1 validation requirement (logically a Gate 2 item) is invisible in the top-level acceptance criteria.
- This is a direct regression introduced by the R9 Gate-reconciliation changes.

**Suggested fix:**

- Make the Plan-Level Gate 2 list identical to the P11 Gate 2 list (four items), or explicitly cross-reference so the two sections cannot drift.

---

## 3. Medium — Trim normalization applied only to section header detection; slug extraction and table-row parsing remain untrimmed

**Location:**

- `crates/anvil-cli/src/p11.rs:50,57` (header)
- `crates/anvil-cli/src/p11.rs:70-76` (slug extraction inside table rows)

**Problem:**

The R9/R10 change added `.trim()` to the two header-position calls, satisfying the "trim-normalized" wording in AC5. However, the subsequent table-row processing still does:

```rust
let cols: Vec<&str> = line.split('|').collect();
cols.get(1).and_then(|col| { ... parts.next() ... })
```

No `.trim()` is applied to the extracted slug or to the "Final (P11)" cell before the `contains` check. A markdown table row with incidental whitespace around the backticks or the status cell can cause either the forward or reverse assertion to fail even though the logical content matches.

**Impact:**

- The "section-scoped + trim-normalized + bidirectional" claim in AC5 overstates the robustness of the current implementation.
- The test can produce spurious failures on purely cosmetic markdown edits that do not change any slug or status value.

**Suggested fix:**

- Apply `.trim()` to both the line before splitting and to the extracted slug and status cell. Add a small comment that the trim is required for the bidirectional contract to be whitespace-tolerant.

---

## 4. Low — AC3 language change ("hinge smoke-checks service name + all RPC names") still presents the test as stronger than the implementation warrants

**Location:**

- R10 briefing AC table row for AC3
- `crates/anvil-cli/src/p11.rs:113-116,155-156`

**Problem:**

The updated AC3 text now reads "hinge smoke-checks service name + all RPC names" with the parenthetical "(smoke test; full schema sync is v1.1)". The two new comment lines correctly label the substring match as intentional. However, the test still performs no negative check (no extra services/RPCs in the doc), performs no field-level or type-level comparison, and the service-name assertion was the only one that received the clarifying comment in this round. The overall framing continues to present the test as a meaningful contract guard when it is deliberately minimal.

**Impact:**

- The AC table gives the impression of a non-trivial check; the reality remains "every name appears somewhere as text."
- Future maintainers may assume more coverage exists than the comments actually guarantee.

**Suggested fix:**

- Change AC3 wording to "docs/contract.md contains every service and RPC name from sidecar.proto (substring smoke test only)" so the limitation is visible at the AC level rather than only inside the test comment.

---

**End of findings.**