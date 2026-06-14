# Anvil — P11 Dogfooding R5 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R5.md`  
**Review date:** 2026-05-27  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **fails** (exit code 1; no diagnostic text emitted by rustfmt in this environment)
- `rustfmt --edition 2021 --check crates\anvil-cli\src\p11.rs` — **fails** (exit code 1; confirms the formatting failure is in/triggered by the R5 P11 source file)
- `cargo test --workspace` — **passes** (190 Rust tests)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **passes**
- `go test ./...` from `C:\Anvil\sidecar` — **passes**
- `cargo run -q -p anvil-cli -- hinge list --count --project C:\Anvil` — **passes**, reports `74`
- `cargo run -q -p anvil-cli -- hinge list --strict --project C:\Anvil` — **passes** on the bare repository checkout and reports no consensus violations
- CLI spot checks:
  - `anvil init --help` confirms `init <PATH>` exists.
  - `anvil hinge list --count --project C:\Anvil\_definitely_not_an_anvil_project` returns `0` with success, not a non-zero exit.

---

## 1. Critical — R5 claims fmt is clean, but `cargo fmt --all -- --check` fails

**Location:**

- `Review Rounds/REVIEW_P11_DOGFOODING_R5.md:8`
- `crates/anvil-cli/src/p11.rs`

**Problem:**

The R5 briefing states:

```text
Fmt: Clean (`cargo fmt --all -- --check`)
```

But running the command fails:

```text
cargo fmt --all -- --check
# exit code 1
```

Running rustfmt directly against the R5-touched file also fails:

```text
rustfmt --edition 2021 --check crates\anvil-cli\src\p11.rs
# exit code 1
```

No diagnostic text was emitted by rustfmt in this environment, but the exit status is reproducible and sufficient to invalidate the “Fmt clean” claim.

**Impact:**

- The R5 validation summary is false.
- The repository does not satisfy the standard formatting gate.
- This is a basic CI-quality blocker even though tests and clippy pass.

**Suggested fix:**

- Run `cargo fmt --all` and commit the formatting changes.
- Re-run `cargo fmt --all -- --check` after formatting and update the review briefing only after it passes.
- If rustfmt continues to exit without diagnostics, isolate the exact rustfmt/toolchain issue before declaring fmt clean.

---

## 2. High — P11 and Plan-level acceptance text still says dogfooding/external pilot are completed via real CLI, while R5 only defers them in the review briefing

**Location:**

- `Review Rounds/REVIEW_P11_DOGFOODING_R5.md:128-129`
- `Anvil Plan/ANVIL_PLAN.md:100`
- `Anvil Plan/ANVIL_PLAN.md:811-831`
- `Anvil Plan/ANVIL_PLAN.md:1135-1137`
- `Anvil Plan/ANVIL_PLAN.md:1182-1192`
- `docs/examples/coordinator-attestation.md:11-18,58-73`
- `docs/examples/dogfooding/README.md:3,10-15,43-50`
- `docs/examples/external-pilot/README.md:3-6,36-72,97-105`

**Problem:**

R5 improves the review-briefing AC table by changing plan-level AC2/AC3 to:

```text
Deferred (attested) — live evidence required before public ship
```

However, the normative Plan still says these are completed requirements for v1 readiness:

```text
The Plan is satisfied — and Anvil v1 is ready to ship — when:
2. The dogfooding test in P11 has produced a Charter and Plan for Anvil v1.1 using the v1 CLI alone.
3. At least one external, non-self-referential project has completed a full Charter → Plan → Build → Ship cycle using the v1 CLI alone...
```

The P11 section also still says:

```text
Anvil v1 has managed at least one Charter → Plan cycle for v1.1 via its own CLI, and at least one external project through a full Charter → Plan → Build → Ship cycle.
```

Meanwhile, the coordinator attestation and example READMEs explicitly say the artifacts are representative/illustrative and **not** live CLI executions against real AI providers.

The R5 briefing now honestly labels the review-table rows as deferred, but it does not reconcile the actual acceptance criteria in `ANVIL_PLAN.md`. The Plan still concludes:

```text
P11 shipped. v1 complete.
```

despite its own AC2/AC3 being deferred.

**Impact:**

- The authoritative Plan and the R5 briefing disagree on whether v1 is actually ready to ship.
- Future readers may rely on `ANVIL_PLAN.md` and conclude the live dogfooding/pilot occurred when they did not.
- The release/public-ship gate is ambiguous: R5 says live evidence is required before public ship, while the Plan says v1 is complete now.

**Suggested fix:**

- Amend `ANVIL_PLAN.md` so P11 AC1/AC2/AC3 and Plan-level AC2/AC3 explicitly state the current status: deferred with Coordinator attestation; live audit-store evidence required before public ship/public announcement.
- Update the “Deliverable,” “Bottom Line,” and “P11 shipped. v1 complete.” language to distinguish “implementation build complete” from “public ship / acceptance complete.”
- Update `docs/examples/dogfooding/README.md` and `docs/examples/external-pilot/README.md` headings/metadata so they do not claim “produced using the Anvil v1 CLI” or “Full cycle completed” before the disclaimer explains the artifacts are representative.

---

## 3. High — Amendment A1 deferral is incomplete: normative Charter/Plan text still requires `anvil audit export --public` and other deferred A1 items

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:18`
- `Anvil Plan/ANVIL_PLAN.md:23`
- `Anvil Plan/new_project_charter.md:458-463`
- `Anvil Plan/new_project_charter.md:470-472`
- `Anvil Plan/CHARTER_AMENDMENT_A1.md:158-179`
- `Anvil Plan/AMENDMENT_A1_CONVERGENCE.md:50-53`
- `Anvil Plan/AMENDMENT_A1_HARDENING_HISTORY.md:41,192,199`

**Problem:**

R5 corrects the most visible `ANVIL_PLAN.md` record-count statement to say the three A1-contemplated record types were deferred and v1 has 15 record types. But the same line still contains an incomplete/incorrect sentence:

```text
The **`anvil audit export --public`** command (default-deny per record, secret scan, license scan, sensitivity-label respect, Coordinator manual review gate, cryptographic seal). The local-private vs public-project record distinction is core P2 work.
```

It names the command as if it is still a P2/core-v1 mechanism, without saying it is deferred in this normative Plan section.

The Charter-applied section also still says:

```text
Public publication requires per-record explicit Coordinator approval through `anvil audit export --public`...
Structured CLI Output Stability — per-command JSON Schemas in `schemas/cli/`, `schema_version` in every output, stable error codes, `--describe-schema` flag mandatory...
Repo-Readiness Acceptance Gates — ... public-safe audit bundle self-validation...
```

Those capabilities are not implemented in v1 and are only partially deferred in hardening-history prose. The `new_project_charter.md` downstream note even still says “Plan Draft 7” must reconcile counts from 13 to 16 and add the public-export bundle to P2, contradicting the R5/R4 deferral state.

**Impact:**

- Normative Charter/Plan text continues to require v1 features that do not exist.
- The deferral is recorded in hardening history but not fully applied to the active Charter/Plan content.
- Public-readiness and embedding-contract expectations remain unclear for downstream users and v1.1 designers.

**Suggested fix:**

- Update the active Charter-applied and Plan A1 sections to clearly mark `anvil audit export --public`, public-export record types, broad structured CLI output/schema discovery, and public-safe audit bundle self-validation as deferred to v1.1 unless actually implemented.
- Fix the incomplete sentence in `ANVIL_PLAN.md:18` so it states the command is deferred, not present.
- Update `new_project_charter.md:472` to stop saying Plan Draft 7 must reconcile 13→16 and add the public-export bundle to P2 if the current governance decision is deferral.
- Keep historical amendment documents as legislative history if desired, but ensure the active/normative documents are internally consistent.

---

## 4. Medium / High — Hinge/metric prose still contains stale counts and stale hinge names after R5

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:945`
- `Anvil Plan/ANVIL_PLAN.md:949`
- `Anvil Plan/ANVIL_PLAN.md:965`
- `Anvil Plan/ANVIL_PLAN.md:973`
- actual `anvil hinge list --count --project C:\Anvil` output

**Problem:**

The R5/R4 fixes updated the hinge registry subset language and added `test_contract_doc_sync_method`, but several stale hinge/count references remain:

- `ANVIL_PLAN.md:945` says the full hinge registry has 74 annotations.
- `ANVIL_PLAN.md:965` still says “73 hinge entries scanned.”
- `ANVIL_PLAN.md:949` and `ANVIL_PLAN.md:973` still reference the old `test_audit_store_record_types_count` count assertion, even though that hinge was renamed/reworked as `test_audit_store_required_types_present` and is a subset check.

The actual scanner reports 74 hinge entries. The Plan simultaneously claims 74 and 73 in adjacent sections.

**Impact:**

- The Plan still contains drift in the very section R5 claims to have corrected.
- The deferred-decision resolution-rate baseline is stale after the new P11 hinge was added.
- The audit-store hinge description misleads readers about enforcement: it is not an exact count assertion anymore.

**Suggested fix:**

- Replace “73 hinge entries scanned” with the current value or, better, avoid restating a number and refer to `anvil hinge list --count`.
- Replace `test_audit_store_record_types_count` references with `test_audit_store_required_types_present` and describe it as a subset/minimum check.
- Add a lightweight doc consistency check for old hinge names and stale count prose if these registry sections remain manually maintained.

---

## 5. Medium — Contract-doc drift test only checks unqualified RPC-name substrings, so it can pass while the documented contract is materially wrong

**Location:**

- `crates/anvil-cli/src/p11.rs:79-135`
- `docs/contract.md`
- `proto/anvil/v1/sidecar.proto`

**Problem:**

R5 improves `test_contract_doc_sync_method` from a tautology/string-presence check to extracting RPC names from the proto and checking they appear in `docs/contract.md`.

This is an improvement, but the protection is still very weak:

- It checks only RPC method names, not service name, request/response types, streaming modifiers, message fields, field numbers, oneof variants, enum values, package, or `go_package`.
- It uses `contract_doc.contains(rpc)`, so any mention anywhere in the document satisfies the test, including a stale note, warning, unrelated prose, or a list of “missing RPCs.”
- It does not verify that each RPC appears in a service definition or an RPC-specific section.

For example, the test would pass if `docs/contract.md` listed “Handshake, Invoke, InvokeStreaming, Cancel, Health, ReloadConfig” in a paragraph while documenting the wrong request/response schema elsewhere.

**Impact:**

- R5 overstates this as “real drift detection.” It detects one narrow class of drift: a new/renamed RPC absent as a substring.
- The sidecar contract can still materially diverge from the protobuf while tests pass.
- Integrators may infer stronger protection than the test provides.

**Suggested fix:**

- Rename the test/comment/briefing language to “RPC-name coverage smoke test,” not “drift detection,” or strengthen the test.
- If stronger protection is desired, extract service definitions and message/enum field signatures from the proto and compare against structured snippets in `docs/contract.md`.
- At minimum, assert the service block contains lines like `rpc <Name>(<Request>) returns (...)` rather than arbitrary substrings.

---

## 6. Medium — Release smoke-test plan now uses `anvil hinge list --count --project <tmp-dir>` and expects non-zero exit, but the command returns success with `0` for a nonexistent/uninitialized project

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:1016`
- CLI behavior observed with `cargo run -q -p anvil-cli -- hinge list --count --project C:\Anvil\_definitely_not_an_anvil_project`

**Problem:**

R5 replaced nonexistent release-smoke-test commands with existing commands. One new smoke-test step says:

```text
run `anvil hinge list --count --project <tmp-dir>` and verify non-zero exit
```

But the command does not fail for a nonexistent/uninitialized project in the current implementation. It returns success and prints `0`:

```text
cargo run -q -p anvil-cli -- hinge list --count --project C:\Anvil\_definitely_not_an_anvil_project
0
```

So the release smoke-test text specifies an expectation that does not match the CLI.

**Impact:**

- The release smoke-test procedure is still not executable as written.
- A release candidate could fail the documented smoke test despite the binary behaving consistently with current implementation.
- This continues the prior problem of documenting unvalidated smoke-test commands.

**Suggested fix:**

- Either change the expected behavior to “verify output is `0`” for an empty/non-source temp project, or run `hinge list --count` against the extracted Anvil source tree if a non-zero count is expected.
- Add a scripted smoke-test fixture to validate the exact release-smoke-test commands before documenting them as locked acceptance.

---

## 7. Low / Medium — Attestation and example documents still use strong “produced/completed” metadata before disclaiming that artifacts are representative

**Location:**

- `docs/examples/dogfooding/README.md:3,10-15,43-50`
- `docs/examples/external-pilot/README.md:3-6,36-72,97-105`
- `docs/examples/coordinator-attestation.md:16-18,68-73`

**Problem:**

The example READMEs now contain disclaimers, but their front matter and early sections still make strong factual claims:

```text
Session: Anvil v1.1 charter and plan — produced using the Anvil v1 CLI
This directory contains the outputs from running Anvil v1's own CLI...
Outcome: Full Charter → Plan → Build → Ship cycle completed
```

Only later do they say the artifacts are representative and not live exports. This ordering is misleading for readers skimming the files or viewing summaries in directory listings.

**Impact:**

- The repository still presents synthetic artifacts as completed operational evidence until the reader reaches the disclaimer.
- This undercuts the R5 correction that plan-level dogfooding/pilot evidence is deferred.

**Suggested fix:**

- Put “Representative / not a live CLI execution” in the title or first metadata block of both example READMEs.
- Change “produced using the Anvil v1 CLI” to “representative of output expected from the Anvil v1 CLI.”
- Change “Outcome: Full cycle completed” to “Representative outcome: full cycle shape documented; live execution deferred.”

---

## Overall Assessment

R5 makes real improvements over R4:

- The PL hinge now parses `ANVIL_PLAN.md` and checks the slug list against the Plan table.
- The contract-doc hinge now at least checks proto RPC names appear in the contract doc.
- The R5 review table labels live dogfooding/external pilot evidence as deferred rather than pass-attested.
- Several stale audit-record-count statements were corrected.

However, I would **not approve R5 as ship-ready** yet because:

1. The advertised formatting gate fails.
2. The authoritative Plan still says P11/Plan-level live dogfooding and external pilot are complete, while R5 says they are deferred.
3. Active Charter/Plan text still contains undeferred A1 requirements for `audit export --public`, structured CLI output, and public-safe bundle mechanics.
4. The Plan still has stale hinge counts and stale hinge names.
5. The contract “drift” test is only an RPC-name substring smoke test.
6. The release smoke-test command expectation does not match current CLI behavior.

Minimum recommended before approval:

1. Fix `cargo fmt --all -- --check`.
2. Amend active Plan/Charter text so deferred live evidence and deferred A1 items are reflected normatively, not only in review briefings/hardening history.
3. Clean remaining stale hinge count/name references.
4. Correct the release-smoke-test procedure against actual CLI behavior.
5. Reword or strengthen the contract-doc sync test to match its actual coverage.