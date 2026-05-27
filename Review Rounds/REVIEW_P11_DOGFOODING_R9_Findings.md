# Anvil — P11 Dogfooding R9 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R9.md`  
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
- Release-smoke-test spot check:
  - `anvil init <fresh-temp-dir>` succeeds.
  - `anvil hinge list --count --project <fresh-temp-dir>` exits 0 and prints `0`.
  - `<fresh-temp-dir>\anvil.toml` exists.
- CLI help spot check:
  - `anvil phase reopen --help` shows both `-y` and `--yes`, so the old README claim that only `-y` appears in help is stale.

---

## 1. High — Gate 1 says all phases shipped per per-phase acceptance criteria, but P11’s own AC1–AC3 remain deferred

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:827-831`
- `Anvil Plan/ANVIL_PLAN.md:1135-1150`
- `Anvil Plan/ANVIL_PLAN.md:1206`

**Problem:**

R8/R9 improved the Plan-Level Acceptance Criteria by splitting Gate 1 from Gate 2. However, Gate 1 still says:

```text
The following criteria are satisfied at P11 ship:

1. All 15 phases ... have shipped per the per-phase acceptance criteria.
```

But P11’s own per-phase acceptance criteria still include three deferred criteria:

```text
1. At least one Charter → Plan cycle completes via `anvil` CLI alone ... *(Deferred with attestation ...)*
2. At least one external ... completes a full Charter → Plan → Build → Ship cycle ... *(Deferred with attestation ...)*
3. The external pilot includes at least one Build phase ... *(Deferred with attestation ...)*
```

So Gate 1 claims all 15 phases shipped per per-phase ACs while P11’s per-phase AC list explicitly says part of P11’s acceptance is deferred. The Gate 1/Gate 2 split fixed the Plan-level AC contradiction but did not reconcile the lower-level P11 AC list.

**Impact:**

- “P11 Accepted” remains ambiguous: either P11 is accepted despite deferred per-phase ACs, or Gate 1 criterion 1 is false.
- Future readers can still interpret the implementation build as satisfying live dogfooding/pilot ACs because Gate 1 says all phase ACs were met.
- The Plan now has two acceptance models for the same P11 facts: the P11 section says AC1–AC3 deferred; Gate 1 says all per-phase ACs shipped.

**Suggested fix:**

- Rewrite the P11 section’s acceptance criteria to separate:
  - P11 documentation/build ACs satisfied at Gate 1; and
  - live dogfooding/pilot ACs deferred to Gate 2.
- Alternatively, change Gate 1 criterion 1 to “all 15 phases have shipped per Gate-1-applicable per-phase acceptance criteria; P11 live dogfooding/pilot criteria are explicitly Gate 2.”
- Avoid saying “all per-phase acceptance criteria” unless every listed per-phase criterion is actually true.

---

## 2. High / Medium — Gate 1 is marked complete while it includes release-candidate artifacts and smoke-test results that are explicitly not P11 deliverables

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:1137-1150`
- `Anvil Plan/ANVIL_PLAN.md:1011-1016`

**Problem:**

Gate 1 says the following criteria are satisfied at P11 ship, then includes:

```text
9. v1 binaries (`anvil`, `anvil-sidecar`) build correctly for the primary platform ...
   A signed `SHA256SUMS.txt.asc` is published with every release archive.
   The smoke-test script described in *Open Items / Distribution* passes against the primary-platform release candidate before v1 is declared shipped.
   The script is written at release time (not in P11) ...
```

This criterion mixes at least three different states:

1. Development build succeeds — currently validated.
2. Release archive/checksum/signature exists — not shown in R9 and not present as a P11 deliverable.
3. Release-candidate smoke-test script passes — explicitly “written at release time (not in P11).”

Yet Gate 1 concludes:

```text
Status: Complete.
```

The R9 validation shows normal workspace builds pass, but it does not show a release archive, signed checksum, release-candidate smoke-test script, or `anvil-sidecar --version` from a packaged archive. A `Get-ChildItem` spot check found no obvious release/smoke script artifact in the repository.

**Impact:**

- Gate 1 “Complete” overclaims release readiness.
- The Plan treats release-time deliverables as already satisfied at P11 while simultaneously saying they are not P11 code deliverables.
- This blurs “implementation build complete” with “release candidate produced and verified.”

**Suggested fix:**

- Move release archive/signature/smoke-test-script requirements out of Gate 1 into a separate **Release Candidate Gate** or Gate 2/public-ship checklist.
- Keep Gate 1 limited to what R9 actually validates: workspace builds/tests pass and documentation is complete.
- If Gate 1 is intended to include release readiness, add the actual smoke-test script and release artifact validation evidence before marking it complete.

---

## 3. Medium / High — Representative sub-artifacts still assert live/converged/shipped outcomes despite R9 README framing

**Location:**

- `docs/examples/dogfooding/v11-plan-summary.md:3-7`
- `docs/examples/external-pilot/audit-store-summary.EXAMPLE.json:1-35`
- `docs/examples/external-pilot/README.md:16-22,80-87,101-107`
- `docs/examples/external-pilot/charter.md:3-6`
- `docs/examples/external-pilot/LEAFLOG_PLAN.md:3-5`

**Problem:**

R9/R8 fixed much of the README body language, but several representative child artifacts still read like completed live outputs.

Examples:

```text
docs/examples/dogfooding/v11-plan-summary.md:
Status: Converged (R1 clean pass via dogfooding session)
This summary is the phase-level output of the `anvil plan invoke` dogfooding run.
```

```json
docs/examples/external-pilot/audit-store-summary.EXAMPLE.json:
"note": "... counts produced by a complete Leaflog Charter → Plan → Build → Ship cycle ..."
"outcome": "shipped"
"provider_diversity_stress": "pass"
```

README examples still say:

```text
Timebox (≤14 days) | Completed in 6 days
No pilot-blocking failures occurred.
charter.md — final converged charter (R2 clean pass)
LEAFLOG_PLAN.md — final converged plan (4-phase)
```

These artifacts are marked representative in some places, but their internal metadata and summary fields still present the synthetic scenario as a live completed/converged/shipped run.

**Impact:**

- A reader browsing files directly can miss the README framing and conclude the live dogfooding run, live external pilot, provider diversity stress, and ship gate all occurred.
- The `.EXAMPLE` suffix helps, but the JSON content itself still encodes `outcome: shipped` and `provider_diversity_stress: pass` rather than representative/pending states.
- This undermines Gate 2’s “Deferred (attested)” status.

**Suggested fix:**

- Update representative child artifacts to carry explicit representative metadata at the top or in fields, e.g. `status: representative_not_live`, `outcome: representative_shipped_shape`, `provider_diversity_stress: pending_live_validation`.
- Change `v11-plan-summary.md` wording to “representative phase-level output expected from a future `anvil plan invoke` dogfooding run.”
- Change README artifact labels from “final converged” to “representative final/converged form.”
- Avoid “completed,” “shipped,” and “pass” in synthetic data unless immediately qualified as representative.

---

## 4. Medium — Coordinator attestation still overclaims what representative artifacts prove and what was validated

**Location:**

- `docs/examples/coordinator-attestation.md:16-18`
- `docs/examples/coordinator-attestation.md:30`
- `docs/examples/coordinator-attestation.md:36-40`
- `docs/examples/coordinator-attestation.md:68-73`

**Problem:**

The attestation now sits at the center of the Gate 1/Gate 2 split, but it still uses language that overstates representative artifacts as evidence:

```text
representative artifacts are the appropriate evidence for v1's first-generation build
```

and:

```text
The representative artifacts validate that the Anvil workflow ... would produce the right artifacts.
```

It also says:

```text
Every command exercised in the example artifacts ... exists in the built binary with the exact argument shapes shown.
```

and:

```text
The UX friction points, provider diversity behavior, and workflow gaps documented in the example artifacts are accurately drawn from knowledge of the CLI's implementation and from operating it during the build process.
```

Given that live provider calls and live pilot execution are explicitly deferred, the attestation should be careful to say representative artifacts are documentation/scaffolding evidence, not validation evidence for the live acceptance criteria. “Provider diversity behavior” is especially problematic because R9’s external-pilot README now says provider diversity stress is “to be validated in live run.”

**Impact:**

- Gate 1 criterion 10 was clarified, but the referenced attestation still partly reintroduces the same overclaim.
- Future readers may treat representative artifacts as stronger evidence than intended.
- The attestation’s provider-diversity statement conflicts with the deferred live validation status.

**Suggested fix:**

- Reword the attestation to state that representative artifacts are **documentation deliverables** and **workflow-shape examples**, not substitutes for Gate 2 evidence.
- Replace “validate” with “illustrate” where referring to representative artifacts.
- Remove or narrow “provider diversity behavior” until the live pilot has executed against real providers.
- Add an explicit statement: “Gate 1 is satisfied only for documentation; Gate 2 remains unsatisfied until live audit-store evidence exists.”

---

## 5. Medium — P11 and v1.1 transition prose still references dogfooding as if it happened, despite Gate 2 deferral

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:199-200`
- `Anvil Plan/ANVIL_PLAN.md:811-821`
- `Anvil Plan/ANVIL_PLAN.md:987-989`
- `Anvil Plan/ANVIL_PLAN.md:1027-1037`
- `Anvil Plan/ANVIL_PLAN.md:1183-1188`
- `Anvil Plan/PLAN_HARDENING_HISTORY.md:506-520`

**Problem:**

R8/R9 fixed several metric/risk references, but other Plan sections still describe dogfooding/pilot feedback as if it already happened or will inform v1.1 after v1 ship:

```text
Reviewed against v1 usage in P11 dogfooding and docs/ux-audit.md.
Reviewed against docs/ux-audit.md during P11 dogfooding.
```

```text
v1.1 work ... scoped after v1 ships and after v1 usage produces design evidence...
```

```text
Which adapters ship in v1.1 vs. later is informed by P11 dogfooding and pilot feedback on users' actual provider preferences.
```

The dogfooding/pilot feedback that involves live CLI execution is Gate 2 and remains deferred. Some evidence does exist from build observations and UX audit, but the text does not consistently distinguish those real build observations from deferred live dogfooding/pilot evidence.

**Impact:**

- The v1.1 planning story still relies on P11 dogfooding/pilot evidence that the Plan elsewhere says is not yet available.
- The Required Choices table and transition section can mislead readers into thinking the v1.1-prep locks were evaluated using a live dogfooding run.

**Suggested fix:**

- Replace “during P11 dogfooding” with “during P11 build/UX audit and representative dogfooding analysis” where that is what actually happened.
- Reserve “live P11 dogfooding/pilot feedback” for Gate 2 evidence once it exists.
- Update v1.1 transition and provider-adapter roadmap sections to say live evidence remains pending before public ship.

---

## 6. Low / Medium — Contract smoke test improved to service-name + RPC-name presence, but remains substring-only and has no negative coverage

**Location:**

- `crates/anvil-cli/src/p11.rs:111-191`
- `Review Rounds/REVIEW_P11_DOGFOODING_R9.md:42-69`
- `Review Rounds/REVIEW_P11_DOGFOODING_R9.md:87`

**Problem:**

R9 improves the contract-doc smoke test by checking service names in addition to RPC names. The AC table also correctly labels it as a smoke test, with full schema sync deferred to v1.1.

However, the test still uses substring checks:

```rust
contract_doc.contains(service)
contract_doc.contains(rpc)
```

This can pass if names appear in unrelated text and does not check the actual `service Sidecar { rpc ... }` block or signatures.

This is no longer a blocker if it is intentionally scoped as a smoke test, but it remains an area for improvement.

**Impact:**

- The test can miss material contract-doc drift.
- There is still no negative-case coverage proving that a missing service/RPC in the actual service block fails for the intended reason.

**Suggested fix:**

- If staying within v1 scope, keep the “smoke test only” language and add a short comment that substring matching is intentional.
- For v1.1, parse or generate the service block so request/response signatures and streaming modifiers are checked structurally.

---

## 7. Low — External-pilot README still contains a stale `phase reopen --yes` help-output complaint

**Location:**

- `docs/examples/external-pilot/README.md:84-87`
- CLI spot check: `cargo run -q -p anvil-cli -- phase reopen --help`

**Problem:**

The README says:

```text
The `--yes` flag on `anvil phase reopen` is not documented in the CLI's `--help` output — it appears only as `-y` in the Clap short form.
```

But the current help output shows both forms:

```text
-y, --yes                Skip the confirmation prompt (for CI / non-interactive use)
```

This may have been true in an earlier round, but it is stale now.

**Impact:**

- Minor documentation drift.
- The example reports a UX gap that no longer exists.

**Suggested fix:**

- Remove this bullet or replace it with a current UX gap verified against current `--help` output.

---

## Overall Assessment

R9 is substantially improved:

- Formatting, Rust tests, clippy, Go tests, workspace build, and hinge scans all pass.
- Gate 1 criterion 10 is now correctly labeled as a documentation deliverable rather than a live dogfooding substitute.
- The PL parser now scopes to the Required Choices section and trims header boundaries.
- The contract smoke test now checks service names and RPC names.

Remaining issues are not build failures; they are acceptance-boundary and evidence-boundary issues:

1. Gate 1 still says all phases shipped per per-phase ACs while P11 AC1–AC3 are deferred.
2. Gate 1 includes release-candidate/signature/smoke-test requirements that are explicitly not P11 deliverables.
3. Representative child artifacts still encode live/converged/shipped/pass outcomes.
4. The attestation still overclaims representative artifacts as validation evidence.
5. Some Plan/v1.1 prose still refers to dogfooding/pilot feedback as if live evidence exists.

Recommended minimum before final approval:

1. Reconcile the P11 per-phase AC list with the Gate 1/Gate 2 split.
2. Move release-candidate artifacts and signed smoke-test requirements out of Gate 1, or produce the release evidence.
3. Update representative child artifacts and attestation wording to avoid live-run semantics.
4. Clean remaining dogfooding/pilot references in the Plan so build observations, representative analysis, and deferred live evidence are clearly separated.