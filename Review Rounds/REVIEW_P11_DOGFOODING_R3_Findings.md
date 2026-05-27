# Anvil — P11 Dogfooding R3 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R3.md`  
**Review date:** 2026-05-27  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **passes**
- `cargo test --workspace` — **passes** (190 Rust tests)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **passes**
- `go test ./...` from `C:\Anvil\sidecar` — **passes**
- `cargo run -q -p anvil-cli -- hinge list --count --project C:\Anvil` — **passes**, reports `74`
- `cargo run -q -p anvil-cli -- hinge list --strict --project C:\Anvil` — **passes** on the bare repository checkout and reports no consensus violations
- CLI help spot checks performed for `setup`, `charter`, `phase reopen`, and top-level commands.
- Contract spot check performed against `proto/anvil/v1/sidecar.proto`.

---

## 1. Critical — P11 AC1/AC2 are marked PASS while the preserved “dogfooding” and “external pilot” artifacts explicitly say they are not live Anvil CLI executions

**Location:**

- `Review Rounds/REVIEW_P11_DOGFOODING_R3.md:115-125`
- `Anvil Plan/ANVIL_PLAN.md:811-827`
- `Anvil Plan/ANVIL_PLAN.md:1134-1136`
- `docs/examples/external-pilot/README.md:97-105`
- `docs/examples/dogfooding/README.md:43-50`
- `docs/examples/external-pilot/audit-store-summary.EXAMPLE.json:1-35`

**Problem:**

R3 marks these acceptance criteria PASS:

- AC1: v1.1 Charter → Plan cycle completes via `anvil` CLI alone.
- AC2: external non-self-referential project completes full Charter → Plan → Build → Ship via `anvil` CLI alone.
- AC3: external pilot includes at least one Build phase through multi-reviewer rotation.

However, the artifact READMEs now explicitly disclaim that the artifacts are representative/illustrative rather than live evidence:

```text
These artifacts are representative and illustrative ... not live audit-store exports from an actual `anvil` CLI execution against real AI providers.
```

and:

```text
These artifacts are representative and illustrative ... not live exports from an actual `anvil discuss` / `anvil charter review` / `anvil plan invoke` execution against real AI providers.
```

The example JSON also says actual record IDs are not preserved and the pilot project's own `.anvil/` directory is authoritative, but that authoritative store is not present in the reviewed repository.

This is a direct contradiction: the Plan requires actual CLI cycles as the primary P11 acceptance evidence, while the shipped evidence states it is synthetic/representative.

**Impact:**

- P11’s main purpose is not demonstrated by the provided artifacts.
- Reviewers cannot verify that `anvil` successfully performed the dogfooding Charter → Plan cycle or the external pilot Build → Ship workflow.
- Multi-reviewer rotation, provider-diversity stress, audit integrity, convergence declarations, and phase ship records are self-attested rather than evidenced.
- Marking AC1/AC2/AC3 as PASS over representative artifacts risks shipping v1 without the core dogfooding/pilot validation having actually occurred or being auditable.

**Suggested fix:**

- Preserve real, redacted audit-store evidence for both dogfooding and Leaflog, or clearly mark AC1/AC2/AC3 as not satisfied.
- At minimum, include a verifiable evidence manifest with record IDs, record types, phase IDs, reviewer/provider identities, convergence records, `PhaseDisposition` records, `GateApproval` records, and integrity-check output.
- If the actual audit stores cannot be published, create a signed Coordinator attestation that names where they are retained and records hashes/counts sufficient to verify they existed at P11 review time.
- Do not call representative examples “completed via `anvil` CLI alone” unless the corresponding live audit evidence is available for review.

---

## 2. High — Charter/Plan still require v1 audit-store record types that are absent from the implementation

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:18`
- `Anvil Plan/new_project_charter.md:466-468`
- `Anvil Plan/CHARTER_HARDENING_HISTORY.md:240-242`
- `Anvil Plan/CHARTER_AMENDMENT_A1.md:176-179`
- `crates/anvil-audit/src/records.rs:18-38`
- `crates/anvil-audit/src/records.rs:132-149`
- `Anvil Plan/PLAN_HARDENING_HISTORY.md:537-547`

**Problem:**

The Charter and Plan still state that v1 includes sixteen audit-store record types, including:

- `PublicVisibilityPolicy`
- `PublicExportApproval`
- `EmergencyFreezeDeclaration`

The implementation’s `RecordType` enum has 15 entries, and the missing three A1/governance records are not among them. The 15 implemented entries instead include later Plan extensions such as `CuratedFindings` and `PlanConsolidation`.

R3/R2 added `PLAN_HARDENING_HISTORY.md` Amendment 9 to defer `anvil audit export --public`, but that amendment does not cleanly reconcile the Charter/Plan statements that the record types themselves are v1 audit-store types. In particular, `new_project_charter.md` says:

```text
With Amendment A1 applied, v1's audit store now includes sixteen record types in total ... PublicVisibilityPolicy, PublicExportApproval, EmergencyFreezeDeclaration.
```

That statement is false against the current code.

**Impact:**

- The audit-store schema implemented in code does not match the project’s constitutional/governance documents.
- Governance events such as emergency freezes have no corresponding audit record type despite `GOVERNANCE.md` saying they are recorded as `EmergencyFreezeDeclaration` records.
- Public-export deferral is incomplete: the export command may be deferred, but the Charter still claims supporting record types are part of v1.
- Downstream users or v1.1 code may rely on record schemas that do not exist.

**Suggested fix:**

- Either implement the three missing record types and update `ALL_RECORD_TYPES`/layout/tests accordingly, or formally amend the Charter/Plan to remove them from v1 and place them in v1.1.
- If `PublicVisibilityPolicy` and `PublicExportApproval` are deferred with `audit export --public`, state that explicitly in the Charter-applied section, not only in Plan hardening history.
- Decide whether `EmergencyFreezeDeclaration` is deferred or required in v1; make `GOVERNANCE.md`, Charter, Plan, and `RecordType` agree.

---

## 3. High — Plan-level release acceptance still references nonexistent commands and unevidenced P11 smoke-test deliverables

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:1010-1022`
- `Anvil Plan/ANVIL_PLAN.md:1132-1145`
- `Anvil Plan/PLAN_HARDENING_HISTORY.md:238-250`
- CLI help output for `setup` and `charter`

**Problem:**

R3 fixed `docs/runbook.md` and `docs/onboarding.md`, but the normative Plan still says every release candidate smoke test runs:

```text
anvil setup --headless
anvil charter render
```

Neither command exists:

- `anvil setup` accepts only optional `[PATH]`; no `--headless` flag.
- `anvil charter` has only `review` and `findings`; no `render` subcommand.

The same Plan section also says the smoke-test script is a v1 deliverable in P11, and Plan-level acceptance criterion #11 says the smoke-test script must pass against the primary-platform release candidate before v1 is declared shipped. The R3 briefing does not list a smoke-test script, release archive, checksum file, signature, `INSTALL.md`, or release-candidate validation evidence.

**Impact:**

- Plan-level acceptance criterion #11 is not satisfied as written.
- The Plan’s distribution/open-source release process cannot be executed because it names nonexistent CLI commands.
- R3’s P11 PASS table is incomplete if v1 readiness still depends on Plan-level acceptance criteria beyond the local P11 section.

**Suggested fix:**

- Update the Plan distribution smoke-test commands to match the actual CLI, or implement the missing commands if they are required.
- Provide the P11 smoke-test script and evidence that it passes against a Windows x64 release candidate, or formally defer release packaging/smoke testing out of v1 ship criteria.
- Reconcile `PLAN_HARDENING_HISTORY.md` with the corrected distribution acceptance text so the legislative record does not continue to require nonexistent commands.

---

## 4. Medium / High — Deferred-Decision Registry / hinge documentation is stale and describes a proc-macro/style mechanism that does not exist

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:920-944`
- `Anvil Plan/ANVIL_PLAN.md:946-949`
- `Anvil Plan/ANVIL_PLAN.md:589`
- `Anvil Plan/ANVIL_PLAN.md:872`
- `Anvil Plan/ANVIL_PLAN.md:1002`
- `crates/anvil-cli/src/p11.rs:33-41`
- Actual hinge CLI output: `anvil hinge list --count --project C:\Anvil` reports `74`

**Problem:**

The Plan says the Deferred-Decision Registry table is canonical and that the count is derived from `anvil hinge list --count`. But the table is visibly stale:

- It still lists `test_workspace_lock_enforced`, which earlier P4 review material says was renamed to `test_workspace_runtime_dir_in_layout`.
- It does not include the new P11 hinge `test_contract_doc_sync_method` added in R3.
- It contains only a small subset of the 74 hinge annotations reported by the actual scanner.

The same section also says:

```text
P10b's hinge-framework implementation enforces this convention: the hinge proc-macro accepts a `style: "exact" | "minimum"` attribute ...
```

The current hinge framework uses comment annotations (`// hinge_test: pins=..., intended=..., phase=...`) and scanner logic. There is no proc-macro and no `style` attribute in the codebase.

**Impact:**

- The Plan’s claimed canonical hinge registry is not canonical.
- Readers get incorrect guidance about how hinge tests are represented and enforced.
- The new R3 manual-sync hinge is not reflected in the Plan registry even though the R3 briefing claims it is an intentional P11 pin.
- Hinge-count and deferred-decision evidence is less trustworthy because the normative registry and actual scanner output disagree.

**Suggested fix:**

- Rewrite the Deferred-Decision Registry section to reflect the actual scanner-based hinge system.
- Remove references to a nonexistent proc-macro/style attribute, or implement that mechanism if it is intended.
- Either generate the registry table from `anvil hinge list` output or stop calling the Markdown table canonical.
- Add `test_contract_doc_sync_method` and rename/remove stale `test_workspace_lock_enforced` references.

---

## 5. Medium — `test_contract_doc_sync_method` is a tautological hinge that can never detect drift

**Location:**

- `crates/anvil-cli/src/p11.rs:33-41`
- `Review Rounds/REVIEW_P11_DOGFOODING_R3.md:67-99`
- `docs/contract.md:6-8`

**Problem:**

R3 adds this hinge test:

```rust
assert_eq!("manual-sync", "manual-sync");
```

This does not inspect `docs/contract.md`, `proto/anvil/v1/sidecar.proto`, generated bindings, a timestamp, or even the presence of the maintenance note. It only adds a registry entry saying the current sync method is manual.

The R3 briefing presents this as a useful pin. It is honest that CI drift detection is absent, but the test itself gives the appearance of validation while providing no executable guardrail.

**Impact:**

- The hinge registry gains another always-green entry that cannot fail under any repository drift scenario.
- Future reviewers may overestimate the protection around the sidecar contract documentation.
- A normal comment in `docs/contract.md` would communicate the same fact without polluting the hinge registry with a non-test.

**Suggested fix:**

- Remove the tautological hinge or replace it with a minimal meaningful assertion, such as checking that `docs/contract.md` contains the maintenance note and the `Last synced` line.
- Prefer a real drift check for service/RPC/message names if the intent is to protect integration documentation.
- If the intent is only to document manual sync, keep it as documentation rather than a hinge test.

---

## 6. Medium — Runbook still describes `gate check-plan` as a disposition-rendering gate and audit-record creator, but implementation only checks Required Choices and writes no audit record

**Location:**

- `docs/runbook.md:101-113`
- `crates/anvil-cli/src/main.rs:659-679`

**Problem:**

The runbook’s Gate 4 section says:

```text
After curation, render the disposition document:
...
anvil gate check-plan --project .
...
Audit record: GateApproval (disposition-rendered gate)
```

The implementation of `cmd_gate_check_plan` only loads config and checks whether Required Choices are locked. It prints success/failure and does not render a disposition document or append any `GateApproval` record.

**Impact:**

- The runbook does not accurately cover the six gate operations.
- Operators following the runbook will believe a gate audit record was created when none was written.
- This undermines AC1 for documentation coverage and may produce missing provenance during real project use.

**Suggested fix:**

- Correct the runbook to distinguish Required-Choice plan-stage checks from disposition rendering.
- Document the actual command/path that creates disposition-rendered `GateApproval` records, or state that this gate is currently manual/implicit if no command exists.
- Add a doc smoke test or audit-store fixture test to ensure documented gate commands actually create the records the runbook says they create.

---

## 7. Low / Medium — R3 metadata says prior R1 was a clean pass, despite the checked-in R1 findings containing eight blocking concerns

**Location:**

- `Review Rounds/REVIEW_P11_DOGFOODING_R3.md:11`
- `Review Rounds/REVIEW_P11_DOGFOODING_R1_Findings.md:18-409`
- `Review Rounds/REVIEW_P11_DOGFOODING_R2_Findings.md:17-149`

**Problem:**

The R3 briefing says:

```text
Prior rounds: R1 (2026-05-27, clean pass), R1 second pass (2026-05-27, 8 findings, all applied), R2 ...
```

But the checked-in `REVIEW_P11_DOGFOODING_R1_Findings.md` is not a clean pass; it contains eight findings and concludes “I would not approve P11 R1 as final v1 ship-ready.”

This may be explainable if there was an earlier untracked/overwritten first reviewer pass, but the current repository evidence is confusing.

**Impact:**

- The review history is hard to audit.
- Future readers may misunderstand which findings were actually raised in which round.
- This is minor compared with the acceptance/evidence issues above, but it weakens the review trail.

**Suggested fix:**

- Clarify the round naming: e.g., “initial reviewer pass clean; second reviewer R1 pass produced 8 findings.”
- If both passes are important, preserve both findings files under distinct names.

---

## Overall Assessment

R3 improved several R2 issues: the two v1.1-prep PL slugs now appear in the Required Choices table, stale PL language in `ANVIL_PLAN.md` is mostly corrected, the example audit summary is clearly marked `.EXAMPLE`, and `docs/contract.md` now matches the broad shape of the protobuf.

The implementation health is also clean: Rust/Go tests, fmt, clippy, and hinge strict checks pass.

However, I would **not approve P11 R3 as final v1 ship-ready** because the main remaining problems are acceptance-evidence and governance consistency issues:

1. The dogfooding and external-pilot artifacts explicitly say they are representative, not live Anvil CLI executions, while P11 AC1/AC2 require actual CLI cycles.
2. The Charter/Plan still claim v1 audit-store record types that the implementation does not contain.
3. Plan-level release/smoke-test acceptance still names nonexistent commands and lacks evidence.
4. The hinge registry section is stale and describes a non-existent proc-macro/style mechanism.
5. The new contract-doc sync hinge is tautological and provides no real guardrail.
6. The runbook still misstates at least one gate command’s behavior/audit effects.

Minimum recommended before approval:

1. Provide real redacted audit evidence for dogfooding and the external pilot, or downgrade AC1/AC2/AC3 status.
2. Reconcile A1 audit-record types across Charter, Plan, and `RecordType` implementation.
3. Fix Plan-level release smoke-test commands and provide/re-scope the P11 release smoke-test deliverable.
4. Update the hinge registry documentation to match the scanner-based implementation and actual hinge list.
5. Remove or strengthen tautological hinge tests.
6. Correct the runbook’s Gate 4/audit-record claims.