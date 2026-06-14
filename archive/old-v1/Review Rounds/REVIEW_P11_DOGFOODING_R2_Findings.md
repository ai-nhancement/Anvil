# Anvil — P11 Dogfooding R2 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R2.md`  
**Review date:** 2026-05-27  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first. This review deliberately seeks issues the R1/R2 fixes introduced or left unaddressed.

## Validation Performed

- `cargo build --workspace` — **passes**
- `cargo test --workspace` — **passes** (189 tests)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **passes**
- `cargo fmt --all -- --check` — **passes**
- `go test ./...` from `C:\Anvil\sidecar` — **passes**

---

## 1. Critical — Hinge test hard-codes 8 PL names that do not appear as keys in `ANVIL_PLAN.md` Required Choices table

**Location:**

- `crates/anvil-cli/src/p11.rs:15`–`24` (the `confirmed_final` array after R2 F1 fix)
- `Anvil Plan/ANVIL_PLAN.md` (Required Choices table and §P11 deliverables)

**Problem:**

After the R2 fix for F1, the test asserts exactly these eight string literals:
```rust
"plan-consolidation-triggers",
"per-metric-numeric-thresholds",
"file-system-layout",
"deferred-decision-tracking",
"ship-transport-actions",
"runtime-alert-response-policies",
"cli-setup-wizard-step-ordering",
"cli-command-structure"
```
A search of `ANVIL_PLAN.md` finds **zero occurrences** of `cli-setup-wizard-step-ordering` or `cli-command-structure`. The Plan refers to the two v1.1-prep items in prose as "Setup Wizard ordering" and "CLI command structure" (or "the two 'v1.1 prep' locks") but never uses the hyphenated slugs that the test now treats as canonical.

The R2 "fix" simply moved the two items into the Final list and removed the `v11_deferred` array without ensuring the names matched the actual Plan table. The hinge test and the Plan are now two independent, unsynchronized sources of truth.

**Impact:**

- The P11 ship gate (the only automated enforcement of AC4/AC5) is pinning a fiction. If the real Required Choices table in the Plan uses different identifiers, the test passes while the governance record is inconsistent.
- Adding or renaming a PL in the future will require coordinated edits in at least three places (Plan table, p11.rs, hardening history) with no compiler or test guard.

**Suggested fix / improvement:**

- The test should either (a) be removed in favor of a manual Coordinator sign-off recorded in the hardening history, or (b) be augmented with a small parser that extracts the PL names from the Plan Markdown and asserts the count and "no outstanding" state. The current hard-coded list is worse than the original v1.1-deferred design because it creates a false sense of automated verification.

---

## 2. High — R2 "all 8 Final" claim contradicts remaining v1.1-prep language still present in `ANVIL_PLAN.md`

**Location:**

- `Anvil Plan/ANVIL_PLAN.md` lines 822, 986, 1137 (references to the two v1.1-prep locks and their revision triggers)
- `p11.rs:9`–`12` (test comment still says "v1.1 prep; v1 wizard confirmed Final")

**Problem:**

The R2 fix updated the Required Choices count note and the hinge test, but left multiple paragraphs in the Plan that still describe the two locks as carrying `revision trigger = v1.1 App design begins`. The test comment itself retains the phrase "v1.1 prep".

The coder performed a mechanical array edit rather than a semantic reconciliation. The Plan now contains internally contradictory statements about whether those two decisions are "confirmed Final at P11 ship" or still gated on v1.1 App design.

**Impact:**

- A future reader (or the v1.1 team) will encounter conflicting guidance on whether the v1 CLI wizard ordering and command structure are locked or still open to re-evaluation.
- The formal "confirmed Final" status asserted by the hinge test is not reflected in the primary governance document.

**Suggested fix / improvement:**

- The Plan paragraphs that still reference the v1.1-prep triggers must be rewritten or removed. The current state is a partial, inconsistent fix that creates new ambiguity rather than resolving the original F1 concern.

---

## 3. High — Representative `audit-store-summary.json` is fabricated data presented with insufficient warning; could be mistaken for real pilot output

**Location:**

- `docs/examples/external-pilot/audit-store-summary.json` (new in R2)
- `docs/examples/external-pilot/README.md` and `docs/examples/dogfooding/README.md` (the "representative and illustrative" notice)

**Problem:**

R2 F3 added a JSON file containing invented record-type counts, phase outcomes, reviewer pool, and hinge test results for the Leaflog pilot. The READMEs now contain a one-sentence disclaimer. However, the file is named `audit-store-summary.json` (the exact name a real `anvil audit export` would produce) and lives under `examples/external-pilot/`. Nothing in the JSON itself or in its directory structure marks it as synthetic.

**Impact:**

- A downstream consumer, auditor, or v1.1 developer could treat the file as authoritative pilot telemetry.
- The "illustrative" notice is easy to miss; it does not appear inside the JSON or as a top-level README warning.

**Suggested fix / improvement:**

- Rename the file to `audit-store-summary.EXAMPLE.json` or move it under a clearly synthetic `mock-data/` subdirectory. The current placement and naming create a material risk of misrepresentation.

---

## 4. Medium — `docs/contract.md` rewrite may have introduced new transcription errors vs the live generated protobuf code

**Location:**

- `docs/contract.md` (completely rewritten in R2 F5)
- `sidecar/internal/contract/sidecar.pb.go` and `proto/anvil/v1/sidecar.proto`

**Problem:**

The R2 fix states the document was "fully rewritten from the .proto." A spot-check of a few message fields (e.g., `InvokeRequest` routing, `InvokeStreamEvent` variants, `ErrorClass` enum values) shows alignment at first glance. However, the review document itself lists 10+ discrepancies that were corrected. This volume of prior errors, combined with a full manual rewrite rather than an automated extraction step, makes it likely that new subtle mismatches were introduced during the rewrite.

No test or build step validates that the Markdown contract document remains in sync with the authoritative `.proto` / generated Go code.

**Impact:**

- The contract reference that external sidecar implementors will rely on could silently diverge from the actual wire protocol.
- Future changes to the protobuf will require another manual, error-prone sync.

**Suggested fix / improvement:**

- Add a CI step that extracts the service, RPC, and message definitions from the `.proto` (or the generated Go descriptors) and fails if `docs/contract.md` is out of date. Manual rewriting is not a sustainable long-term solution for a wire contract.

---

## 5. Medium — Hinge test comment claims "Flipping requires … a confirmed-Final PL being reopened with a new audit record" but the test itself provides no such enforcement

**Location:**

- `crates/anvil-cli/src/p11.rs:13`–`14`

**Problem:**

The test comment states the intended governance contract, but the test only checks a hard-coded list of 8 names and a count of 8. Nothing prevents a developer from adding a new PL name to the array (or removing one) without any corresponding audit record or Plan-table update. The "flip requires code change" property is true only because the list is hard-coded; it does not actually tie the test to any live governance artifact.

**Impact:**

- The test gives a false impression of strong enforcement. It is no stronger than the original count-only version the R1 reviewer criticized.

**Suggested fix / improvement:**

- Either accept that the test is a weak social convention (and document it as such), or implement the parser-based verification suggested in finding 1. The current state is cosmetic.

---

## Summary of R2 Code Health

- The R2 fixes for F1, F3, F4, F5, and F8 are mechanical and leave deeper inconsistencies (name mismatch between test and Plan, lingering v1.1-prep language, fabricated pilot data, manual contract document).
- The most severe issue is that the P11 ship gate (the hinge test) now asserts a set of PL names that do not exist in the primary governance document. This is worse than the state the R1 reviewer flagged.
- Several "representative" artifacts and documentation files were added without sufficient safeguards against misinterpretation.
- The overall pattern in the R2 response is "make the symptom pass the review checklist" rather than "make the underlying governance artifacts consistent and self-verifying."

The P11 deliverables remain in a fragile state despite the claim that "all 8 findings addressed."