# Anvil — P11 Dogfooding R1 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R1.md`  
**Review date:** 2026-05-27  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **passes**
- `cargo test --workspace` — **passes** (189 Rust tests)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **passes**
- `go test ./...` from `C:\Anvil\sidecar` — **passes**
- `cargo run -q -p anvil-cli -- hinge list --count --project C:\Anvil` — **passes**, reports `73`
- `cargo run -q -p anvil-cli -- hinge list --strict --project C:\Anvil` — **passes** on the bare repository checkout and reports no consensus violations

---

## 1. High — P11 marks two Provisional Locks as still Provisional, contradicting “No outstanding Provisional Locks” acceptance criteria

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:199-202`
- `Anvil Plan/ANVIL_PLAN.md:827-832`
- `Anvil Plan/ANVIL_PLAN.md:1137-1139`
- `Anvil Plan/ANVIL_PLAN.md:1191-1193`
- `Anvil Plan/new_project_charter.md:215-220`
- `Anvil Plan/CHARTER_HARDENING_HISTORY.md:202-204`
- `crates/anvil-cli/src/p11.rs:22-40`
- `Review Rounds/REVIEW_P11_DOGFOODING_R1.md:22,99-100,135`

**Problem:**

P11’s Plan acceptance criterion says:

```text
Every Provisional Lock confirmed (→ Final) or revised (with new audit record). No outstanding Provisional Locks at P11 ship.
```

The Plan-level acceptance criteria similarly say:

```text
All Provisional Locks are resolved (confirmed Final or explicitly revised with audit record).
```

The Charter’s Provisional Lock mechanism is stricter still:

```text
The Charter cannot ship while any Provisional Lock is still outstanding.
```

But P11 leaves two choices in a Provisional state:

```markdown
| CLI Setup Wizard step ordering and prompts | **Provisional (v1.1 prep — revision trigger reached; v1.1 deferred)** | ... |
| CLI command structure | **Provisional (v1.1 prep — revision trigger reached; v1.1 deferred)** | ... |
```

The P11 hinge test encodes this as accepted:

```rust
let v11_deferred: &[&str] = &[
    "cli-setup-wizard-step-ordering",
    "cli-command-structure",
];
...
assert_eq!(confirmed_final.len() + v11_deferred.len(), 8)
```

This is not the same as “confirmed Final or revised with new audit record.” It creates a third terminal category (“still Provisional but explicitly deferred”) that the P11 acceptance criteria do not allow.

**Impact:**

- P11 AC4 and Plan-level acceptance criterion #4 are not satisfied as written.
- The final v1 statement “Anvil v1 is complete” is premature while two PL rows remain Provisional.
- The P11 hinge test can pass even though outstanding Provisional Locks still exist.
- The Provisional Lock governance model is weakened at the final ship gate by redefining “not unaddressed” as “resolved,” without an explicit Charter/Plan amendment changing the acceptance rule.

**Suggested fix:**

- Decide the allowed terminal state for the two v1.1-prep locks:
  - confirm them Final for v1 and create new v1.1 design seeds/open items; or
  - revise them with explicit audit records into new v1.1-scoped locks outside v1 ship gating; or
  - amend P11/Plan-level acceptance criteria to formally allow “trigger reached, explicitly v1.1-deferred” as a resolved state.
- Update `test_no_outstanding_provisional_locks_after_dogfooding` so it enforces the final accepted policy rather than treating remaining Provisional rows as resolved by count.
- Add evidence of the required audit record(s) for any “revised” choice state, since the current P11 briefing and Plan text do not identify record IDs.

---

## 2. High — P11 accepts Charter Amendment A1 obligations as “known gaps,” but they are v1/P11 ship-gate requirements

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:16-29`
- `Anvil Plan/ANVIL_PLAN.md:18` (`anvil audit export --public`)
- `Anvil Plan/ANVIL_PLAN.md:25` (`--describe-schema` / `schemas/cli/*.json`)
- `Anvil Plan/ANVIL_PLAN.md:29` (publication-safe history gate + scripted smoke tests)
- `Anvil Plan/CHARTER_AMENDMENT_A1.md:58-65`
- `Anvil Plan/CHARTER_AMENDMENT_A1.md:158-166`
- `Anvil Plan/CHARTER_AMENDMENT_A1.md:258-264`
- `Review Rounds/REVIEW_P11_DOGFOODING_R1.md:109-119`
- `docs/ux-audit.md:368-386`

**Problem:**

The P11 review briefing lists three gaps as “not blocking P11 ship”:

1. `anvil audit export --public` not implemented.
2. `--describe-schema` only implemented for `phase build`; schema embedding infrastructure not built generally.
3. `--format json` absent from read commands.

These are not merely UX refinements. They are tied to Charter Amendment A1 and Plan amendments that explicitly place work in v1 phases:

- P2: `anvil audit export --public` implementing default-deny public audit bundle with secret/license/sensitivity checks and Coordinator review.
- P8: `--describe-schema` for every command emitting structured output, with schemas in `schemas/cli/*.json` embedded into the binary.
- Cross-cutting / Amendment A1: structured CLI output is the v1 embedded surface; `--format json`, schemas, `schema_version`, and stable error codes are public contracts.
- P11: publication-safe history gate with full-history scans and scripted smoke tests before public flip.

The review doc acknowledges these gaps but then says:

```text
None of these prevent the P11 hinge test from passing or any AC from being satisfied.
```

That conclusion is too narrow: P11 is the final v1 phase, and Plan/Charter-level acceptance includes these obligations.

**Impact:**

- v1 may ship while missing constitutional open-source/publication safety mechanisms.
- The project cannot safely flip public without the public audit export mechanism or a completed publication-safe gate.
- The v1 embeddability/structured-output commitments remain unimplemented while docs call Anvil v1 complete.
- Treating phase-scope misses as v1.1 backlog weakens the phase review process because prior missed acceptance criteria can be waved through at P11.

**Suggested fix:**

- Reclassify these gaps as blocking unless a formal Charter/Plan amendment explicitly defers them out of v1.
- Implement or formally defer `anvil audit export --public`, public visibility records/policies if still required, and the associated scans/approval workflow.
- Either complete the structured-output/schema-discovery surface promised by A1, or amend A1/Plan text so only `phase build` is v1 scope.
- Add explicit P11 validation evidence for the publication-safe gate, including full-history secret scan, full-history license scan, Coordinator commit-message review, and any exception/audit records.

---

## 3. High / Medium — P11 evidence artifacts do not prove a full external pilot or dogfooding run through Anvil audit records

**Location:**

- `docs/examples/external-pilot/README.md:55-72,97-103`
- `docs/examples/dogfooding/README.md:43-48`
- `Review Rounds/REVIEW_P11_DOGFOODING_R1.md:19-25,80-93`
- `Anvil Plan/ANVIL_PLAN.md:821`

**Problem:**

The external pilot and dogfooding summaries are useful narratives, but the artifacts preserved in this repository do not include the audit-store evidence necessary to independently verify the claimed workflow cycles.

For the external pilot, the Plan says the pilot artifacts include:

```text
Charter, Plan, dispositions, hardening history, audit-store records
```

But `docs/examples/external-pilot/README.md` lists only:

```text
- charter.md
- LEAFLOG_PLAN.md
- audit-store-summary.json — record type counts ...

Full audit store records from the pilot are not archived here; the pilot project's own `.anvil/` directory is the authoritative record.
```

There is also no `audit-store-summary.json` present under `docs/examples/external-pilot/` in the current file listing, despite being listed as preserved.

For dogfooding, `docs/examples/dogfooding/README.md` says:

```text
The full `.anvil/` audit store from this session is the Anvil project's own audit store ...
```

but the repository snapshot provided for review does not include such an audit store as a reviewable artifact.

**Impact:**

- AC1/AC2/AC3 claims are largely self-attested by prose rather than verifiable from preserved records.
- Reviewers cannot confirm phase reviews, multi-reviewer rotation, convergence declarations, ship dispositions, audit integrity, provider-diversity behavior, or transport execution from the provided artifacts.
- The external pilot deliverable is incomplete relative to the Plan’s own preservation requirement.

**Suggested fix:**

- Preserve a redacted public-safe audit bundle or at least a structured evidence manifest containing record IDs, record types, phase IDs, round counts, reviewer identities/families, ship dispositions, and integrity-check outputs.
- Add the missing `docs/examples/external-pilot/audit-store-summary.json`, or remove the claim that it exists.
- Include disposition documents and hardening histories for the Leaflog pilot, or explicitly amend the Plan’s artifact-preservation requirement.
- For dogfooding, include record IDs or exported/redacted records proving the v1.1 Charter → Plan cycle was executed through Anvil rather than authored manually.

---

## 4. Medium / High — Documentation contains commands and flags that do not exist or do not match the current CLI

**Location:**

- `docs/runbook.md:21-29,75-82,111-116,127-132,143-162,169-180,204-215,276-289`
- `docs/onboarding.md:65-71,121-128,139-144,165-173,202-204`
- `crates/anvil-cli/src/main.rs:257-386`
- CLI help output for `init`, `setup`, `arbiter`, `phase`, and `audit`

**Problem:**

The P11 review briefing says documentation is consistent with the implemented CLI surface, but multiple examples do not match the actual Clap definitions.

Examples:

- `anvil init` is documented without a path in `docs/runbook.md` and `docs/onboarding.md`, but the CLI requires `anvil init <PATH>`.
- `anvil setup --headless` is documented, but `setup` has no `--headless` flag.
- `anvil arbiter resolve-finding --packet-id <id> --finding-id <fid> --disposition keep/drop ...` is documented, but the CLI takes one positional composite `<packet_id>:<finding_id>` and has no `--packet-id`, `--finding-id`, or `--disposition` flags.
- `anvil arbiter declare-convergence --phase-id ... --round-count ...` is documented, but the CLI takes positional `<ARTIFACT>` and only `--reason` / `--project`; it computes round count internally.
- `anvil phase build --phase-id P<N>`, `phase review --phase-id`, `phase ship --phase-id`, and `phase reopen --phase-id` are documented, but the CLI takes positional `<ID>`.
- `anvil phase ship --yes --reason ...` is documented for CI, but `phase ship` has no `--yes` or `--reason` flags.
- `anvil audit list --type GateApproval` and `anvil audit list --format json` are documented, but the CLI takes positional `<RECORD_TYPE>` and has no `--type` or `--format` flags.

**Impact:**

- AC3 and AC6 are weakened: the runbook/onboarding guide cannot be followed reliably by a new user.
- P11’s claim that documentation is “consistent with the implemented CLI surface” is false.
- Operators following the docs will hit Clap errors during critical gates.

**Suggested fix:**

- Regenerate all command examples from `anvil --help` / subcommand help and update docs to match actual positional arguments and flags.
- Add a documentation smoke test that executes every documented command with `--help`-safe or dry-run fixtures where possible.
- Separate “planned/future v1.1” command shapes from actual v1 commands, especially around headless mode and structured JSON.

---

## 5. Medium / High — `docs/contract.md` documents a sidecar RPC contract that does not match the actual protobuf

**Location:**

- `docs/contract.md:30-112`
- `docs/contract.md:117-128`
- `docs/contract.md:137-148`
- `proto/anvil/v1/sidecar.proto:20-42`
- `proto/anvil/v1/sidecar.proto:46-67`
- `proto/anvil/v1/sidecar.proto:71-96`
- `proto/anvil/v1/sidecar.proto:145-152`
- `proto/anvil/v1/sidecar.proto:232-242`

**Problem:**

`docs/contract.md` is supposed to be a sidecar gRPC contract reference, but it describes a different service shape from the actual protobuf.

Documented but not actual:

- Service name `SidecarService`; actual service is `Sidecar`.
- RPCs `Chat` and `ChatStream`; actual RPCs are `Invoke` and `InvokeStreaming` with `InvokeRequest` and oneof payloads.
- `HealthRequest { client_version }` and `HealthResponse { server_version, ready }`; actual `HealthRequest` is empty and `HealthResponse` has `healthy` and `version`.
- `ChatRequest` top-level fields `client_version`, `provider_connection_id`, `model`, etc.; actual routing fields live in `InvokeRequest`, and `ChatRequest` contains only prompt/messages/options.
- Error classes `AuthError`, `RateLimitError`, `ProviderError`, `SchemaError`, `NetworkError`, `InternalError`; actual enum values are `TRANSPORT`, `PROVIDER_REFUSAL`, `SCHEMA_VIOLATION`, `ADAPTER_BUG`, `TIMEOUT`, `CANCELLED` plus sentinel.

The document also omits first-class actual RPCs: `Handshake`, `Cancel`, and `ReloadConfig`.

**Impact:**

- Contributors or v1.1 App designers using `docs/contract.md` will implement against the wrong API.
- AC3’s “contract documentation exists” is not satisfied in substance if the reference is materially inaccurate.
- The v1.1 handoff is especially risky because this document is likely to guide App-side sidecar integration.

**Suggested fix:**

- Regenerate or manually rewrite `docs/contract.md` from `proto/anvil/v1/sidecar.proto` and the generated Rust/Go bindings.
- Include all six actual RPCs and their request/response messages.
- Align error-class names and semantics with the actual `ErrorClass` enum.
- Add a doc test or CI check that compares documented RPC/message names against the protobuf descriptors or source.

---

## 6. Medium — Publication-Safe History Gate is only documented, not executed or evidenced

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:29`
- `Anvil Plan/CHARTER_AMENDMENT_A1.md:58-65`
- `docs/runbook.md:295-310`
- `Review Rounds/REVIEW_P11_DOGFOODING_R1.md:24,57`

**Problem:**

P11 AC6 in the review doc is marked PASS because `docs/runbook.md` contains a “Publication-Safe History Gate” section. But the Plan amendment says the gate is a P11 acceptance criterion before public flip, and adds scripted smoke tests for the gate.

The runbook provides a partial procedure, but the review artifacts do not show:

- full-history secret scan output;
- full-history license scan output;
- Coordinator commit-message review evidence;
- audit-record dispositions for any hits;
- scripted smoke tests for the gate.

The runbook command also appears internally inconsistent: it labels “full-history secret scan (`gitleaks --no-git` against full history)” but the example uses `gitleaks detect --source . --log-opts HEAD~100..HEAD`, which is bounded to the last 100 commits and does not demonstrate the entire history.

**Impact:**

- P11’s publication safety gate is not actually validated.
- A public release could proceed with unreviewed historical commits, secrets, incompatible licenses, or sensitive commit messages.
- The AC status overstates what was done: documentation exists, but execution evidence does not.

**Suggested fix:**

- Add a P11 evidence file containing exact commands, outputs, dates, tool versions, and Coordinator sign-off for the full publication-safe gate.
- Run a true full-history scan, not a bounded `HEAD~100..HEAD` example, or explain why bounded history is sufficient.
- Add the scripted smoke tests promised by the Plan amendment.
- If P11 only requires documentation and public flip happens later, amend the Plan/AC wording to separate “document gate” from “execute gate.”

---

## 7. Medium — P11 hinge test is not connected to live Plan/audit state and can pass after PL drift

**Location:**

- `crates/anvil-cli/src/p11.rs:14-40`
- `Anvil Plan/ANVIL_PLAN.md:193-202`
- `Anvil Plan/PLAN_HARDENING_HISTORY.md:504-522`

**Problem:**

The P11 hinge test hard-codes two arrays of string literals and asserts only their lengths. It does not parse `ANVIL_PLAN.md`, inspect Required Choices, open the audit store, or verify that the listed PLs have corresponding resolution/revision audit records.

Because the arrays are self-contained, the test can pass even if:

- the Plan table changes but the test is not updated;
- a PL row remains Provisional contrary to acceptance criteria;
- the audit store lacks the required revision records;
- a listed PL key has a typo and no longer corresponds to an actual choice.

**Impact:**

- AC5 is weaker than advertised: it verifies a local count convention, not the actual project state.
- The final v1 gate is easy to satisfy by editing the test rather than resolving governance state.
- The test does not catch the current mismatch where two PLs remain Provisional.

**Suggested fix:**

- Replace or supplement the count-only test with validation against a machine-readable source of Required Choices and their states.
- If audit records are required for revisions, assert those records exist and reference the affected choice keys.
- At minimum, assert the exact expected key set and accepted terminal state for each key, not just array lengths.

---

## 8. Low / Medium — P11/Plan text still contains stale phase-count and provisional-threshold statements after declaring v1 complete

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:953-966`
- `Anvil Plan/ANVIL_PLAN.md:1010`
- `Anvil Plan/ANVIL_PLAN.md:1189-1193`

**Problem:**

After P11, the Plan bottom line says “P11 shipped. v1 complete.” However, other sections remain written as pre-P11/provisional statements, for example:

```text
These are the numeric thresholds ... They are provisional until P11 dogfooding and external pilot produce observational baselines; they will be confirmed or revised at that point.
```

and:

```text
All thresholds are provisional. P11 dogfooding and external pilot produce the first real baselines; confirmed thresholds replace these before the Plan is declared fully satisfied.
```

The bottom line also says:

```text
Fourteen phases, mostly linear...
```

while the current Plan repeatedly states there are 15 phases.

**Impact:**

- The final Plan remains internally inconsistent at v1 completion.
- P10a/P11 metric-threshold confirmation is not clearly completed despite being part of Plan-level acceptance.
- Future readers may not know whether thresholds remain provisional or were confirmed during P11.

**Suggested fix:**

- Update the evaluation metric target section with the P11 baseline decision: confirm, revise, or explicitly defer each threshold.
- Fix the stale “Fourteen phases” bottom-line text to 15 phases.
- Add a short P11 completion note that references where observed baseline data is stored.

---

## Overall Assessment

The code validation is clean: Rust fmt, tests, clippy, Go tests, and hinge strict checks all pass. The new P11 hinge annotation is discoverable and the test runs.

However, I would **not approve P11 R1 as final v1 ship-ready** because the most important risks are governance/documentation/evidence gaps rather than compiler failures:

1. Two Provisional Locks remain Provisional despite the acceptance criterion requiring no outstanding PLs.
2. Charter Amendment A1 obligations are treated as non-blocking v1.1 backlog even though the Plan assigns them to v1/P11.
3. External pilot and dogfooding evidence is mostly narrative and does not preserve the audit records the Plan says should be preserved.
4. The runbook/onboarding docs contain several commands and flags that do not exist in the actual CLI.
5. The sidecar contract reference does not match the actual protobuf service.
6. The publication-safe gate is documented but not executed/evidenced.

Minimum recommended before approval:

1. Resolve or formally amend the two remaining Provisional Locks so P11 AC4 is actually true.
2. Either implement A1/P11 publication and structured-output obligations or formally defer them with Charter/Plan amendments.
3. Preserve redacted audit evidence for Leaflog and dogfooding, including record IDs and ship/convergence proof.
4. Correct docs against actual `anvil --help` and `proto/anvil/v1/sidecar.proto`.
5. Provide publication-safe gate execution evidence or amend P11 AC6 to be documentation-only.