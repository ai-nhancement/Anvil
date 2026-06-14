# Anvil — P5 Charter Stage Pipeline R3 Findings

**Source review doc:** `Review Rounds/REVIEW_P5_CHARTER_PIPELINE_R3.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Pass**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Pass**
- `cargo test --workspace` — **Pass — 68 tests, 0 failures**

I did not run real model/sidecar end-to-end flows (`anvil discuss`, `anvil charter review`, `anvil charter findings`) because they require configured provider credentials and an installed/running sidecar.

---

## 1. High / Medium — Interactive curation silently defaults on input errors or cancellation, allowing unintended audit writes

**Location:**

- `crates/anvil-cli/src/charter.rs:326-331`
- `crates/anvil-cli/src/charter.rs:340-344`
- `crates/anvil-cli/src/charter.rs:355-365`
- `crates/anvil-cli/src/charter.rs:401-430`

**Problem:**

The interactive curation flow swallows `dialoguer` input errors and substitutes defaults:

```rust
.interact().unwrap_or(0)
.interact_text().unwrap_or_default()
```

This occurs for action selection, annotation/note text, disposition label selection, and narrative inputs.

If the terminal is interrupted, stdin is unavailable, the user presses Ctrl+C at a prompt, or an input backend error occurs, the command can continue with implicit defaults such as:

- action = `Keep`
- disposition label = `Fixed`
- annotation = empty
- narrative/corrections/residual/reproducibility/bottom line = empty

The command then proceeds to write the disposition document, append hardening history, and persist `CuratedFindingsRecord`.

**Impact:**

- Curation may be committed without an explicit coordinator decision.
- A cancelled or failed terminal interaction can become a valid-looking audit record.
- This weakens the audit semantics of P5 curation, especially because the audit store is append-only.

**Suggested fix:**

- Propagate `dialoguer` errors instead of defaulting silently.
- Treat user cancellation/interruption as `AnvilError::SetupCancelled` or a new curation-cancelled error.
- Only use defaults when the user explicitly accepts the default selection.
- Add a behavior test by factoring prompt operations behind a small input trait or by isolating curation decision construction from terminal I/O.

---

## 2. Medium — `anvil discuss` can spin forever on EOF / non-interactive stdin

**Location:**

- `crates/anvil-cli/src/discuss.rs:109-122`

**Problem:**

The conversation loop reads from stdin and treats empty input as “continue”:

```rust
stdin.lock().read_line(&mut user_input)?;
let user_input = user_input.trim().to_owned();

if user_input.is_empty() {
    continue;
}
```

`read_line()` returns `Ok(0)` on EOF. In a non-interactive environment, piped input exhaustion, or closed stdin, this creates an infinite loop that repeatedly prints the prompt and immediately reads EOF again.

**Impact:**

- `anvil discuss` can hang/busy-loop instead of exiting clearly.
- This is especially likely in scripted or accidentally headless runs.
- It can make the command appear stuck after the first model turn if no charter packet was produced.

**Suggested fix:**

- Capture the byte count from `read_line()`.
- If it is `0`, return a clear cancellation/input-ended error.
- Optionally detect non-interactive stdin at startup and provide a headless mode or a clear “interactive terminal required” message.
- Add a test for EOF handling if the input loop is factored away from direct `stdin` access.

---

## 3. Medium / Low — Provenance lookup for P5 records is asserted as ready but not directly regression-tested

**Location:**

- `Review Rounds/REVIEW_P5_CHARTER_PIPELINE_R3.md:123`
- `crates/anvil-cli/src/charter.rs:537-550`
- `crates/anvil-graph/src/graph.rs:26-73`

**Problem:**

R3 claims:

```text
Provenance graph can locate P5 records by cross-reference | Ready
```

The P5-specific test only verifies that generated cross-reference strings parse:

```rust
CrossRefKey::parse(&key).is_some()
```

That is useful, but it does not seed an `AuditStore` with P5 record types and verify that `ProvenanceGraph::build()` can locate `ReviewerFindingPacket`, `VerifierResult`, and `CuratedFindingsRecord` by `charter.md:§root:R<N>`.

The implementation likely works because the graph stores raw cross-reference strings, but the acceptance claim is stronger than the test coverage.

**Impact:**

- A future change could break P5 provenance wiring while `test_p5_cross_ref_keys_parseable` still passes.
- The R3 readiness claim is not directly pinned by a P5 provenance integration test.

**Suggested fix:**

- Add a focused test that initializes an `AuditStore`, appends P5 records with `CrossRefKey::new("charter.md", "§root", "R1")`, builds `ProvenanceGraph`, and asserts all expected record IDs are returned for that key.
- Keep the parseability test as a lower-level format guard.

---

## 4. Low / Medium — Reviewer model identity can be persisted as an empty string when omitted by the model

**Location:**

- `crates/anvil-cli/src/charter.rs:134-145`
- `crates/anvil-cli/src/charter.rs:193-201`

**Problem:**

`PartialFindingsPacket` defaults `reviewer_model_identity` to an empty string:

```rust
#[serde(default)]
reviewer_model_identity: String,
```

`run_charter_review()` then stores that value directly in the `FindingsPacket`, even though it already has the configured `model_id` available from the selected reviewer binding.

If the reviewer model omits `reviewer_model_identity`, the persisted audit packet contains an empty model identity.

**Impact:**

- Audit records lose useful provenance about which configured model produced the findings.
- This weakens traceability even though the CLI already knows the model identity used for invocation.

**Suggested fix:**

- If `partial.reviewer_model_identity.trim().is_empty()`, fall back to the configured `model_id`.
- Consider similarly enforcing or defaulting `reviewer_id` to the configured role (`reviewer-1`) rather than trusting model output completely.
- Add a test for missing reviewer model identity in `<findings_packet>` JSON.

---

## 5. Low — Section-heading grounding does not recognize indented Markdown headings

**Location:**

- `crates/anvil-core/src/pipeline.rs:395-403`
- `crates/anvil-core/src/pipeline.rs:693-774`

**Problem:**

R3 tightened heading detection so `###NoSpace` is no longer accepted. That is good. However, the predicate still operates on the raw line:

```rust
let after_hashes = line.trim_start_matches('#');
after_hashes != line
    && after_hashes.starts_with(|c: char| c.is_whitespace())
    && after_hashes.trim_start() == section.as_str()
```

This does not accept headings with leading spaces, for example:

```markdown
  ### Third
```

CommonMark allows up to three leading spaces before an ATX heading. If reviewer grounding is intended to follow Markdown semantics, the current check is still narrower than expected.

**Impact:**

- Findings anchored to valid indented Markdown headings may be marked `CannotBeVerified`.
- This is lower risk than the prior `###NoSpace` issue but still a verifier false negative.

**Suggested fix:**

- Trim leading spaces before checking hash markers, while preserving the requirement for whitespace after the hashes.
- If strict non-indented headings are intentional, document that the verifier supports only flush-left headings.
- Extend `test_verify_section_heading_all_levels` with an indented heading case.

---

## 6. Low — Token accumulation remains in `stream_one_turn()` despite no longer being used for commit

**Location:**

- `crates/anvil-cli/src/discuss.rs:207-215`

**Problem:**

R2/R3 correctly removed the `_ => token_buf` fallback, so partial streamed output is not committed. However, `stream_one_turn()` still builds `token_buf`:

```rust
let mut token_buf = String::new();
...
token_buf.push_str(tok);
```

The buffer is not read after the final-result error arms were added.

**Impact:**

- Minor unnecessary allocation and memory growth for long responses.
- The presence of a token buffer can confuse future maintainers into thinking it is part of the authoritative result path.

**Suggested fix:**

- Remove `token_buf` entirely and only print tokens in the display callback.
- If debug capture is desired, name it explicitly as display/debug-only and keep it behind a logging path.

---

## Overall Assessment

P5 R3 is materially improved over R2 and all claimed validation commands pass in this workspace. The R2 follow-up items are addressed: `run_charter_findings` no longer needs a `too_many_lines` suppress, the RFP/VR mismatch path has a real `AuditStore` error-path test, missing required charter fields are covered, and `###NoSpace` is rejected.

I would consider R3 close to approvable, but I recommend fixing the interactive input error handling before final approval because it can persist audit records after cancellation or terminal input failure. The other findings are lower-risk improvements or test-coverage gaps.

Minimum recommended before approval:

1. Propagate/cancel on `dialoguer` input errors in `anvil charter findings` instead of silently defaulting.
2. Handle EOF in `anvil discuss` to avoid infinite loops in headless or closed-stdin contexts.
3. Add a P5 provenance graph integration test for RFP/VR/CuratedFindings lookup by `charter.md:§root:R1`.
