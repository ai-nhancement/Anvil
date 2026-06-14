# Anvil — P11 Dogfooding R11 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R11.md`  
**Review date:** 2026-05-27  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **passes**
- `cargo test --workspace` — **passes** (190 Rust tests)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **passes**
- `go test ./...` from `C:\Anvil\sidecar` — **passes**
- `cargo build --workspace` — **passes**
- `go build ./cmd/anvil-sidecar` from `C:\Anvil\sidecar` — **passes**
- `cargo run -q -p anvil-cli -- hinge list --count --project C:\Anvil` — **passes**, reports `74`
- `cargo run -q -p anvil-cli -- hinge list --strict --project C:\Anvil` — **passes** on the bare repository checkout and reports no consensus violations
- Release-smoke spot check:
  - `anvil init <fresh-temp-dir>` succeeds.
  - `anvil hinge list --count --project <fresh-temp-dir>` exits 0 and prints `0`.
  - `<fresh-temp-dir>\anvil.toml` exists.
- CLI help spot check:
  - `anvil phase reopen --help` shows both `-y` and `--yes`.

---

## 1. High — R11 claims the two Gate 2 lists are identical, but Plan-Level Gate 2 and P11 Gate 2 still differ materially

**Location:**

- `Review Rounds/REVIEW_P11_DOGFOODING_R11.md:33-44`
- `Anvil Plan/ANVIL_PLAN.md:833-837`
- `Anvil Plan/ANVIL_PLAN.md:1154-1161`

**Problem:**

R11 says:

```text
The two Gate 2 lists are now identical.
```

They are not identical.

P11 Gate 2 currently lists:

```text
1. Live dogfooding Charter → Plan
2. Live external pilot full Charter → Plan → Build → Ship
3. External pilot includes at least one Build phase with multi-reviewer rotation
4. v1.1 Plan from live dogfooding validated as v1.1 App design input
```

Plan-Level Gate 2 currently lists:

```text
1. Live dogfooding Charter and Plan
2. Live external pilot full cycle, including at least one Build phase with multi-reviewer rotation
3. v1.1 Plan from live dogfooding validated as v1.1 App design input
4. Release archive + SHA256SUMS.txt + signed SHA256SUMS.txt.asc + smoke-test script passes
```

The differences are material:

- P11 Gate 2 has **no release archive / signed checksum / smoke-test criterion**.
- P11 Gate 2 treats multi-reviewer rotation as a standalone item; Plan-Level Gate 2 folds it into the external-pilot item.
- R11’s acceptance table includes Gate 2 AC4 release-time work, but the normative P11 section does not.

**Impact:**

- R10 Finding 2 is not fully resolved.
- Future release reviewers still have two different Gate 2 definitions depending on whether they read the P11 phase section or the Plan-Level section.
- The release-time criterion may be missed if someone follows only the P11 phase acceptance list.

**Suggested fix:**

- Make the P11 Gate 2 list and Plan-Level Gate 2 list textually identical, or make one list explicitly authoritative and replace the other with a cross-reference.
- If keeping both lists, add a hinge/documentation check or reviewer checklist item that prevents future drift.

---

## 2. High — Plan-Level Gate 2 status says “Deferred (attested)” for all four criteria, but the Coordinator attestation only covers dogfooding/external pilot and omits release-time Gate 2 work

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:1154-1163`
- `docs/examples/coordinator-attestation.md:3-18`
- `docs/examples/coordinator-attestation.md:58-73`
- `Review Rounds/REVIEW_P11_DOGFOODING_R11.md:92-95`

**Problem:**

Plan-Level Gate 2 now has four criteria, including:

```text
4. Release archive produced for the primary platform ... smoke-test script ... passes against the release candidate.
```

But the status line below the full Gate 2 list says:

```text
Status: Deferred (attested) — see docs/examples/coordinator-attestation.md.
```

The attestation is still scoped to the old AC2/AC3 model:

```text
Scope: Plan-Level Acceptance Criteria 2 and 3 (dogfooding + external pilot)
```

and its “What Remains” / sign-off language commits only to live dogfooding evidence:

```text
Before v1 is declared publicly shipped, the Coordinator commits to running at least one real dogfooding cycle...
Live dogfooding evidence will be produced and recorded before v1 is publicly announced.
```

It does not attest to:

- Gate 2 AC3: v1.1 Plan from live dogfooding validated as App input; or
- Gate 2 AC4: release archive, checksums/signature, and smoke-test script passing.

R11’s AC table correctly labels Gate 2 AC4 as **Deferred (release-time)**, not **Deferred (attested)**, but the normative Plan-Level Gate 2 status does not make that distinction.

**Impact:**

- The Plan points release-time obligations at an attestation that does not cover them.
- A reader may incorrectly believe all Gate 2 deferrals have formal Coordinator attestation.
- The attestation under-documents the external pilot commitment after the Gate 2 split; it names the external-pilot requirement in the intro but the explicit future commitment only names dogfooding.

**Suggested fix:**

- Split the Gate 2 status into per-item status labels:
  - AC1–AC3: Deferred pending live validation / attested where applicable.
  - AC4: Deferred release-time, not covered by dogfooding attestation.
- Update `docs/examples/coordinator-attestation.md` to use Gate 2 AC1–AC3 terminology rather than “Plan-Level Acceptance Criteria 2 and 3.”
- Add explicit commitments for live external pilot evidence and v1.1 Plan validation, or state that those are separate Gate 2 obligations outside this attestation.

---

## 3. Medium / High — R11 says the “sole remaining” dogfooding-session phrasing was fixed, but several live-dogfooding semantics remain

**Location:**

- `Review Rounds/REVIEW_P11_DOGFOODING_R11.md:17-29`
- `Anvil Plan/ANVIL_PLAN.md:989`
- `Anvil Plan/PLAN_HARDENING_HISTORY.md:533,606`
- `docs/examples/dogfooding/v11-charter.md:3-5`
- `docs/examples/dogfooding/v11-plan-summary.md:4-7`

**Problem:**

R11 says the line 877 edit was:

```text
the sole remaining instance of the "dogfooding session" phrasing
```

A targeted scan still finds stale or over-live wording, including:

```text
ANVIL_PLAN.md:989:
both confirmed Final at P11 after dogfooding and UX audit
```

```text
PLAN_HARDENING_HISTORY.md:533:
v1.1 charter, plan phase summary, and dogfooding session notes
```

```text
docs/examples/dogfooding/v11-charter.md:
Version: R1 (converged via dogfooding session, 2026-05-26)
Produced using: Anvil v1.0.0 CLI (`anvil discuss` + `anvil charter review`)
```

```text
docs/examples/dogfooding/v11-plan-summary.md:
shows converged shape expected from a live dogfooding session
```

Some README references are appropriately representative, but the child charter artifact still directly claims it was produced by live CLI commands.

**Impact:**

- The “live dogfooding was deferred” boundary remains leaky.
- A reader opening `v11-charter.md` directly can conclude the live CLI dogfooding session occurred.
- R11’s statement that the line 877 instance was the sole remaining instance is factually incorrect.

**Suggested fix:**

- Update `docs/examples/dogfooding/v11-charter.md` metadata to mirror the `v11-plan-summary.md` representative disclaimer, e.g. “representative final/converged form — not a live `anvil discuss` / `anvil charter review` output.”
- Replace remaining “after dogfooding” / “dogfooding session notes” phrasing with “P11 build/UX audit/representative dogfooding analysis.”
- Do an exhaustive scan over Plan, hardening history, and `docs/examples/**` before claiming all live-session phrasing is removed.

---

## 4. Medium — P11 action list and open-item prose still read as if the external pilot was chosen/run during P11, despite Gate 2 deferral

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:811-821`
- `Anvil Plan/ANVIL_PLAN.md:1031`
- `Anvil Plan/PLAN_CONVERGENCE.md:51`

**Problem:**

The P11 AC block is now split into Gate 1 / Gate 2, but surrounding prose still says:

```text
Use Anvil v1 CLI ... to complete one external pilot project.
Run a Charter → Plan cycle ... using the v1 CLI.
Identify and run a small, non-self-referential project through a full Charter → Plan → Build → Ship cycle using the v1 CLI.
The pilot's artifacts ... are preserved as a worked example...
```

The Open Items section also says:

```text
External pilot project selection ... chosen during P11 ... Status: open, resolved in P11.
```

Given the current Gate 2 model, the live external pilot has not run. At most, a representative Leaflog scenario has been selected/documented for Gate 1. The prose does not clearly distinguish “representative pilot selected/documented” from “live pilot selected/run.”

**Impact:**

- The surrounding P11 narrative still conflicts with the corrected AC split.
- The Open Item “resolved in P11” may be read as the real external pilot being selected and executed, not merely a representative scenario being documented.

**Suggested fix:**

- Update P11 action-list prose to distinguish Gate 1 representative documentation from Gate 2 live execution.
- Change “External pilot project selection ... resolved in P11” to specify whether Leaflog is the representative scenario or the intended live Gate 2 pilot.
- If Leaflog is intended to be the live Gate 2 pilot later, state “selected in P11; execution deferred to Gate 2.”

---

## 5. Medium — v1.1 evidence language still implies v1 usage data exists before Gate 2 live usage

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:57-59`
- `Anvil Plan/ANVIL_PLAN.md:1187-1192`
- `Anvil Plan/ANVIL_PLAN.md:1204`
- `Anvil Plan/ANVIL_PLAN.md:1099-1101`

**Problem:**

Several remaining passages frame v1.1 as being designed from “v1 usage” evidence:

```text
The App in v1.1 will be designed against this evidence rather than against guesses.
```

```text
the App design can refine them based on real CLI usage data
```

```text
v1.1 work ... scoped after v1 ships and after v1 usage produces design evidence
```

```text
v1 proves the discipline
```

```text
Data from P11 and pilot usage.
```

Some of this can be true after Gate 2, but the current P11 state has only build observations plus representative artifacts. The text should distinguish current Gate 1 evidence from future Gate 2 live usage evidence.

**Impact:**

- The v1.1 planning story still over-relies on live usage evidence that has not yet been produced.
- The distinction between “build observations,” “representative analysis,” and “live Gate 2 evidence” remains inconsistent.

**Suggested fix:**

- Qualify these statements as “after Gate 2 live usage produces evidence” or “build observations plus future Gate 2 evidence.”
- Replace “v1 proves the discipline” with a narrower Gate 1 claim unless/until live dogfooding/pilot evidence exists.

---

## 6. Low / Medium — Representative external-pilot JSON still uses unqualified `shipped: true` phase fields

**Location:**

- `docs/examples/external-pilot/audit-store-summary.EXAMPLE.json:18-23`
- `docs/examples/external-pilot/README.md:61-76`

**Problem:**

R10/R11 improved top-level representative fields:

```json
"outcome": "representative_shipped_shape",
"integrity_check": "representative_pass_shape",
"provider_diversity_stress": "pending_live_validation"
```

However, the nested phase outcomes still say:

```json
{ "phase_id": "P0", "rounds": 1, "shipped": true }
```

and the README workflow table still uses “Ships” plus “All 4 phases shipped” / “Audit integrity: pass.” The README is under a representative-flow heading, so this is less severe than earlier rounds, but the JSON fields themselves are still unqualified booleans that look like live exported facts.

**Impact:**

- Consumers of the JSON file may read `shipped: true` as real state unless they also parse the top-level note.
- The representative/live boundary remains partly encoded in prose rather than in each data field.

**Suggested fix:**

- Change nested phase fields to representative names/values, e.g. `"representative_shipped_shape": true` or `"status": "representative_shipped_shape"`.
- Change README ship bullets to “would ship” / “representative pass shape” consistently.

---

## 7. Low — Contract smoke test remains intentionally substring-only; acceptable for v1, but still a known weak guard

**Location:**

- `crates/anvil-cli/src/p11.rs:111-194`
- `Review Rounds/REVIEW_P11_DOGFOODING_R11.md:87`

**Problem:**

R11 improves the briefing language by explicitly saying:

```text
substring smoke test only; full schema sync is v1.1
```

This is accurate and sufficient for v1 if intentionally scoped. The implementation still only checks that service and RPC names appear somewhere in `docs/contract.md`, not that the service block, request/response types, streaming modifiers, package, field numbers, or enum values match.

**Impact:**

- Low risk now that the limitation is documented at AC level.
- Still worth tracking so this does not get mistaken for real schema drift detection.

**Suggested fix:**

- Keep the current wording for v1.
- Track structured proto-vs-doc validation or generated contract documentation as a concrete v1.1 task.

---

## Overall Assessment

R11 is improved and the executable validation is clean:

- Rust formatting, tests, clippy, Go tests, workspace builds, and hinge scans pass.
- The PL parser now trims section headers, row text, and extracted slugs.
- AC3 now correctly labels contract checking as a substring smoke test.
- The line 877 “dogfooding session” instance was fixed.

However, I would **not call R11 clean** because the main R10 Gate 2 drift issue is still present in a different form:

1. P11 Gate 2 and Plan-Level Gate 2 are not identical despite R11 claiming they are.
2. Gate 2’s status points all four items to an attestation that only covers part of the list.
3. Several dogfooding-session/live-output claims remain in child artifacts and Plan prose.
4. External-pilot selection/execution wording remains ambiguous under the Gate 1/Gate 2 split.

Recommended minimum before final approval:

1. Make the two Gate 2 lists textually identical or replace one with a cross-reference.
2. Split Gate 2 statuses by item so release-time work is not labeled “attested.”
3. Update coordinator attestation scope to Gate 2 AC1–AC3 and explicitly cover live external pilot / v1.1 Plan validation commitments.
4. Clean remaining live-dogfooding wording in `docs/examples/dogfooding/v11-charter.md`, `ANVIL_PLAN.md`, and `PLAN_HARDENING_HISTORY.md`.