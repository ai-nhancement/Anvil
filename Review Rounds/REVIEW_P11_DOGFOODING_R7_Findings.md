# Anvil — P11 Dogfooding R7 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R7.md`  
**Review date:** 2026-05-27  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **passes**
- `cargo test --workspace` — **passes** (190 Rust tests)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **passes**
- `go test ./...` from `C:\Anvil\sidecar` — **passes**
- `cargo run -q -p anvil-cli -- hinge list --count --project C:\Anvil` — **passes**, reports `74`
- `cargo run -q -p anvil-cli -- hinge list --strict --project C:\Anvil` — **passes** on the bare repository checkout and reports no consensus violations
- Release-smoke-test spot check:
  - `anvil init <fresh-temp-dir>` succeeds.
  - `anvil hinge list --count --project <fresh-temp-dir>` exits 0 and prints `0`.
  - `<fresh-temp-dir>\anvil.toml` exists.
- CLI help spot checks performed for `sidecar`, `plan`, `phase`, and `ship`.

---

## 1. High — Plan-level acceptance still says “Plan is satisfied” / “v1 ready to ship” while two required acceptance criteria are explicitly deferred

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:1131-1137`
- `Anvil Plan/ANVIL_PLAN.md:1190-1192`
- `Review Rounds/REVIEW_P11_DOGFOODING_R7.md:130-131`
- `docs/examples/coordinator-attestation.md:58-73`

**Problem:**

R7 correctly keeps the review-briefing AC table honest:

```text
AC2 (plan-level) Dogfooding cycle via v1 CLI — Deferred (attested)
AC3 (plan-level) External pilot via v1 CLI with multi-reviewer rotation — Deferred (attested)
```

But the normative Plan-level AC section still opens with:

```text
The Plan is satisfied — and Anvil v1 is ready to ship — when:
```

and then lists the deferred criteria inline:

```text
2. The dogfooding test ... using the v1 CLI alone. *(Deferred with attestation ...)*
3. At least one external ... completed ... using the v1 CLI alone ... *(Deferred with attestation ...)*
```

This creates a logical contradiction. A condition cannot simultaneously be required for “Plan is satisfied / ready to ship” and be deferred until before public ship/public announcement. The bottom line compounds this with:

```text
P11 build complete. v1 implementation shipped. Live dogfooding and external pilot evidence deferred...
```

The documents now distinguish “implementation shipped” from “live evidence deferred,” but the Plan-level acceptance header still states that the Plan is satisfied when the listed conditions are true, and two listed conditions are not true yet.

**Impact:**

- The release gate remains ambiguous: is v1 “ready to ship” now, or only after deferred AC2/AC3 live evidence exists?
- Reviewers and future maintainers can reasonably interpret the Plan-level ACs as unsatisfied because AC2/AC3 are explicitly deferred.
- The distinction between “implementation build complete,” “P11 accepted,” “public ship,” and “public announcement” is not cleanly defined.

**Suggested fix:**

- Split the section into two explicit gates:
  1. **Implementation-build/P11 documentation gate** — satisfied by representative artifacts + attestation.
  2. **Public-ship acceptance gate** — requires live dogfooding and external pilot audit-store evidence.
- Change the header from “The Plan is satisfied — and Anvil v1 is ready to ship — when” to language that reflects the two-stage state.
- Use one term consistently: “public ship,” “public announcement,” and “repo public flip” currently appear as overlapping but not identical gates.

---

## 2. High — Evaluation metrics and risk mitigations still claim observational dogfooding/pilot data exists, contradicting the deferred-evidence status

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:954-965`
- `Anvil Plan/ANVIL_PLAN.md:975`
- `Anvil Plan/ANVIL_PLAN.md:987-989`
- `Anvil Plan/ANVIL_PLAN.md:1009`
- `docs/examples/coordinator-attestation.md:16-18,58-73`

**Problem:**

The Plan now says live dogfooding and external-pilot evidence is deferred, but several later sections still treat the dogfooding/pilot as completed observational data:

```text
P11 dogfooding and the external pilot (Leaflog) produced the first observational baselines.
Based on that data, all thresholds are confirmed as stated — no revision warranted.
```

Risk mitigations still say:

```text
The external pilot fills the gap: it exercises Build → Ship on a real project.
```

and:

```text
The external pilot in P11 validates that the CLI is usable on a real project by a real user.
```

Open Items still says:

```text
Baseline performance data will come from P11 dogfooding and pilot. Status: open, to be characterized after P11.
```

These claims are not compatible with the attestation, which says the example artifacts are representative/illustrative and not live audit-store exports from actual CLI execution against real providers.

**Impact:**

- Layer-2 metric thresholds are marked “confirmed” from observational data that the project simultaneously says was not produced.
- Risk mitigations are overstated: the external pilot has not yet validated real Build → Ship behavior or real-user CLI usability.
- v1.1 design may incorrectly rely on representative artifacts as if they were empirical usage data.

**Suggested fix:**

- Reclassify the metric thresholds as “provisionally confirmed from build/test data; live P11 dogfooding/pilot baselines deferred before public ship.”
- Update the risk mitigations to state that the external pilot is still a required pre-public validation, not a completed mitigation.
- Update the performance-characterization open item so its status reflects the deferred live run, not “after P11” as though P11 already produced the data.

---

## 3. Medium / High — Representative example READMEs still narrate live CLI activity and live provider results after the disclaimer

**Location:**

- `docs/examples/dogfooding/README.md:20-39`
- `docs/examples/external-pilot/README.md:38-75`
- `docs/examples/external-pilot/README.md:87-95`

**Problem:**

R6/R7 moved representative-artifact disclaimers to the top, which is a real improvement. However, the body text still presents synthetic/representative material as if it happened in a live run.

Dogfooding README examples:

```text
These were found during the Charter → Plan cycle and were fixed before P11 shipped.
The CLI handled the v1.1 design cycle without workflow-blocking failures.
After running the setup wizard in `anvil setup` three times during dogfooding...
```

External-pilot README examples:

```text
The Coordinator ran `anvil discuss` to help structure it.
`anvil plan invoke` produced a 4-phase plan.
All four phases went through `anvil phase build → review → ship`.
`anvil ship --project .` passed all gates.
Provider diversity stress: **passed**.
```

Those statements are not framed as hypothetical or representative in the sections where they appear. They directly contradict the top-level notice saying the artifacts are not live CLI executions.

**Impact:**

- A reader can still reasonably conclude that live dogfooding/pilot runs happened, despite the top disclaimer.
- The documents continue to blur the boundary between actual build observations and representative examples.
- Provider-diversity stress is particularly misleading because the attestation says real provider API calls were not part of the build/test harness.

**Suggested fix:**

- Rewrite body sections in conditional/representative language, e.g. “A real run is expected to surface…” / “The representative Leaflog flow assumes…” / “Provider-diversity stress to be validated in live run.”
- Move any truly real build-observed friction into `docs/ux-audit.md` or the coordinator attestation and label it separately from representative pilot claims.
- Avoid “passed,” “produced,” “ran,” and “went through” for non-live example flows.

---

## 4. Medium — PL parser is improved but still scans the entire Plan and remains table-format dependent; R7 overstates “without false positives”

**Location:**

- `crates/anvil-cli/src/p11.rs:44-60`
- `Review Rounds/REVIEW_P11_DOGFOODING_R7.md:21-28`

**Problem:**

R7 improves the P11 PL parser by stripping `**` before checking for `Final (P11)`:

```rust
.filter(|line| line.replace("**", "").contains("Final (P11)"))
```

This addresses one formatting variant, but the parser still:

- scans the entire `ANVIL_PLAN.md`, not just the Locked Required Project-Level Choices table;
- depends on pipe-delimited Markdown table rows;
- assumes the slug is the first backtick-delimited token in column 2;
- fails if the status column is reworded semantically, split across lines, or moved;
- can pick up any future table row elsewhere in the Plan that happens to contain `Final (P11)` and a backtick token in the second column.

The R7 briefing says the change is tolerant of minor formatting variations “without false positives.” That is overstated: it is more tolerant of bold-marker placement, but it is not scoped tightly enough to rule out unrelated table-row matches.

**Impact:**

- The hinge test is useful but still a heuristic tied to one Markdown shape.
- Future legitimate Plan edits can break or pollute the PL extraction.
- AC5’s “full bidirectional sync” is only as strong as this unscoped extraction.

**Suggested fix:**

- Scope extraction to the `## Locked Required Project-Level Choices` section and stop at the next top-level section.
- Parse the table header to find the Choice and Lock Type columns rather than assuming column positions.
- Fail with a clear error if no table is found or if a matching row lacks a slug, rather than silently filtering it out.

---

## 5. Medium — Contract documentation check remains a substring smoke test; R7 correctly rebuts R6’s factual error but does not close the underlying drift risk

**Location:**

- `crates/anvil-cli/src/p11.rs:92-147`
- `docs/contract.md:53-63`
- `proto/anvil/v1/sidecar.proto:20-42`
- `Review Rounds/REVIEW_P11_DOGFOODING_R7.md:31-56`

**Problem:**

R7 is correct that R6 Finding 2 mischaracterized the current code: the test does extract RPC names from the proto and checks each appears in `docs/contract.md`.

However, the underlying limitation from R5 remains:

```rust
contract_doc.contains(rpc)
```

checks only unqualified RPC-name substrings anywhere in the document. It does not validate service name, request/response types, streaming modifiers, message fields, field numbers, enum values, package, or whether the names appear in the actual service block.

R7’s “No code change” is reasonable as a rebuttal to the factual error, but this should not be treated as a substantive contract-sync solution.

**Impact:**

- Contract drift can still pass tests as long as RPC names appear somewhere.
- The maintenance note says automated drift detection is v1.1, which is accurate; therefore the P11 hinge should be described as a minimal smoke test, not a sync guarantee.

**Suggested fix:**

- Keep the R7 factual correction, but avoid implying the contract doc is materially protected from drift.
- If staying within v1 scope, rename the hinge description to “RPC-name presence smoke test.”
- For v1.1, implement a structured proto-vs-doc check or generate `docs/contract.md` from the `.proto`.

---

## 6. Low / Medium — R7 “all findings applied” wording is imprecise because one R6 finding was refuted, not applied

**Location:**

- `Review Rounds/REVIEW_P11_DOGFOODING_R7.md:4`
- `Review Rounds/REVIEW_P11_DOGFOODING_R7.md:11`
- `Review Rounds/REVIEW_P11_DOGFOODING_R7.md:31-56`

**Problem:**

R7 says:

```text
Scope: Full P11 R6 finding responses — all 5 findings addressed
...
All findings applied across all rounds.
```

But R7 F2 says the R6 finding was factually incorrect and no code change was made:

```text
Resolution: No code change — finding is factually incorrect.
```

That is a valid disposition, but it is not “applied.” It is “refuted” or “addressed by correction.”

**Impact:**

- Minor review-history imprecision.
- Future readers may assume every finding resulted in a patch when one was deliberately rejected.

**Suggested fix:**

- Change “all findings applied” to “all findings addressed; one R6 finding refuted as factually incorrect.”

---

## Overall Assessment

R7 is materially healthier than prior rounds:

- Formatting, tests, clippy, Go tests, and hinge scanner checks all pass.
- The PL hinge now has a bidirectional comparison and is more tolerant of bold-marker variations.
- The R6 factual error about the contract test was correctly called out.
- The release-smoke-test count expectation now matches actual CLI behavior for a fresh initialized project.

I would still **not call P11 fully clean** because the remaining issues are about acceptance semantics rather than compilation:

1. The Plan still says it is “satisfied / ready to ship” while live dogfooding and external-pilot ACs are deferred.
2. Metric and risk sections still claim observational dogfooding/pilot evidence exists.
3. Representative example bodies still narrate live CLI/provider activity.
4. The PL parser remains heuristic despite the bidirectional set check.
5. The contract-doc check is only a substring smoke test.

Recommended minimum before final approval:

1. Split implementation-complete versus public-ship acceptance gates in `ANVIL_PLAN.md`.
2. Reword metric/risk sections so deferred live evidence is not treated as completed observational baseline data.
3. Rewrite representative example body text to avoid live-run claims.
4. Scope the PL parser to the Required Choices table or clearly document its heuristic nature.