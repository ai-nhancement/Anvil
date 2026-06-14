# Anvil — P11 Dogfooding R13 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R13.md`  
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

## 1. High / Medium — The Plan now describes Leaflog as a changelog/release-notes CLI, but all external-pilot artifacts define it as a houseplant watering journal

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:1031`
- `docs/examples/external-pilot/README.md:3,21,28-34`
- `docs/examples/external-pilot/charter.md:12-14`
- `docs/examples/external-pilot/LEAFLOG_PLAN.md:13-15,40-49,59-87`

**Problem:**

The R13/R12-updated Open Items entry says:

```text
Leaflog (a structured changelog and release-notes CLI) selected as the Gate 1 representative pilot scenario...
```

But the actual external-pilot artifact set consistently defines Leaflog as a houseplant-care tool:

```text
Project: Leaflog — a houseplant watering journal CLI
Domain unrelated to Anvil | Houseplant care
Leaflog is a small CLI tool for tracking houseplant watering events, soil moisture readings, and care notes.
```

The charter and plan likewise center on plants, watering intervals, reminders, and export of plant/event data.

**Impact:**

- This is a factual contradiction in the normative Plan’s tracking entry for the representative external pilot.
- “Structured changelog and release-notes CLI” is closer to developer/productivity tooling than “houseplant care,” which weakens the documented rationale that the pilot domain is unrelated to workflow tools.
- A reader trying to identify the Gate 1 representative pilot from the Plan will get a different project than the artifacts actually preserve.

**Suggested fix:**

- Change the Open Items entry to match the artifacts, e.g. “Leaflog (a houseplant watering journal CLI) selected as the Gate 1 representative pilot scenario...”
- If “structured changelog and release-notes CLI” was intended to replace the pilot domain, update the external-pilot artifact set consistently and re-check the domain-unrelated rubric.

---

## 2. Medium — The new release-time smoke-test Open Item conflicts with the Distribution smoke-test scope and omits `anvil-sidecar --version`

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:1013-1018`
- `Anvil Plan/ANVIL_PLAN.md:1033`
- `Anvil Plan/ANVIL_PLAN.md:1160-1165`

**Problem:**

R13 added the requested Open Item, but its scope does not match the Distribution section’s locked smoke-test scope.

Distribution currently says every release candidate smoke test runs:

```text
extract, run `anvil --version`, run `anvil-sidecar --version`, run `anvil init <tmp-dir>`, run `anvil hinge list --count --project <tmp-dir>`, verify `anvil.toml` was created by init
```

The new Open Item says:

```text
Script scope: v1 binary surface only (install, `anvil init`, `anvil hinge list`, `anvil phase build`, `anvil phase ship`, `anvil ship`)
```

This introduces three problems:

1. It omits `anvil-sidecar --version`, even though the release archive contains both binaries and the Distribution smoke test explicitly includes the sidecar version command.
2. It adds `anvil phase build`, `anvil phase ship`, and `anvil ship`, which are not part of the current Distribution smoke-test list and are much heavier than archive extraction/version/init checks. These commands require meaningful project state and, in normal use, model/provider setup or completed gate records.
3. It says “primary `anvil` binary commands,” but Gate 2 AC4 and the release archive are about both `anvil` and `anvil-sidecar`.

**Impact:**

- The release engineer now has two different smoke-test definitions in the same Plan.
- A release candidate could satisfy the Open Item while skipping the sidecar binary check required by the Distribution section.
- Conversely, the Open Item could be interpreted as requiring full phase build/ship/project ship workflows during release smoke testing, which is not the same bounded smoke test that prior rounds converged on.

**Suggested fix:**

- Make the Open Item explicitly reference the Distribution smoke-test command list instead of restating a divergent scope.
- Include `anvil-sidecar --version` in the Open Item if commands are listed inline.
- Avoid adding `anvil phase build`, `anvil phase ship`, and `anvil ship` unless the Plan deliberately expands Gate 2 AC4 from release-archive smoke testing into a full workflow acceptance run.

---

## 3. Medium — R13 fixed the two R12 “v1 usage” misses, but other unqualified v1/P11 usage-evidence phrases remain

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:55`
- `Anvil Plan/ANVIL_PLAN.md:889`
- `Anvil Plan/ANVIL_PLAN.md:995`
- `Anvil Plan/ANVIL_PLAN.md:1110`
- `crates/anvil-core/src/choices.rs:181`

**Problem:**

R13 correctly fixed the two R12-specific misses at `ANVIL_PLAN.md:823` and `ANVIL_PLAN.md:841`. However, a broader search still finds unqualified usage-evidence language that can be read as current P11 evidence rather than future Gate 2 evidence.

Examples:

```text
The CLI is the v1 deliverable; a desktop App ... is scoped as v1.1 and will be informed by usage feedback from v1.
```

```text
Cost limits are advisory in v1 ... v1.x may evolve the policy based on P11 + pilot usage.
```

```text
Post-v1: if P11 observational data shows this is a frequent pain point...
```

```text
depending on how v1 usage shapes the product story
```

The code-side required-choice seed still says:

```rust
"v1.1 App design begins; validate against v1 usage feedback"
```

Some of these may be intended as future Gate 2 evidence, but the text no longer consistently says so.

**Impact:**

- The Gate 1 / Gate 2 evidence boundary remains slightly inconsistent outside the exact locations fixed in R13.
- Future readers can still infer that P11 live usage data exists when the Plan elsewhere says live provider-backed usage is deferred.
- The default required-choice seed in code can reintroduce “v1 usage feedback” phrasing into generated or inspected artifacts.

**Suggested fix:**

- Qualify these remaining references as “Gate 2 live usage,” “future Gate 2 pilot usage,” or “build observations” depending on intent.
- Update the code-side required-choice revision trigger text if it is meant to reflect the current finalized evidence boundary.

---

## 4. Low / Medium — Representative external-pilot README still uses unqualified “Ships” in the phase outcome table

**Location:**

- `docs/examples/external-pilot/README.md:61-66`

**Problem:**

R12/R13 changed the Ship section bullets to representative language:

```text
All 4 phases would ship (representative_shipped_shape)
Audit integrity: representative_pass_shape
```

But the Build Stage table still uses unqualified outcome values:

```text
| P0 | ... | R1 clean | Ships |
| P1 | ... | R1: 4 findings, R2 clean | Ships |
| P2 | ... | R1: 2 findings, R2 clean | Ships |
| P3 | ... | R1 clean | Ships |
```

The section is framed as a representative flow, so this is lower severity than earlier JSON/live-output issues. Still, the table was one of the areas previously called out for live-run semantics, and it remains less explicit than the corrected JSON and Ship bullets.

**Impact:**

- Minor remaining documentation ambiguity for readers skimming the table.
- The artifact’s representative/live boundary is still conveyed by surrounding prose rather than directly in the outcome cells.

**Suggested fix:**

- Change the Outcome cells to “Would ship” or `representative_shipped_shape` to match the corrected JSON and Ship section.

---

## 5. Low — `PLAN_HARDENING_HISTORY.md` still contains an old release-smoke note with obsolete commands and “v1 deliverable” wording

**Location:**

- `Anvil Plan/PLAN_HARDENING_HISTORY.md:244-250`
- Later correction at `Anvil Plan/PLAN_HARDENING_HISTORY.md:583-585`

**Problem:**

An older hardening-history entry still says:

```text
Smoke tests: scripted release-candidate smoke test (extract, version checks, init, setup --headless, charter render, expected hash). Smoke-test script is a v1 deliverable.
```

Later history correctly records that `setup --headless` and `charter render` were removed and that the smoke-test script is release-time, not a P11 code deliverable. Because this is historical record, it may be acceptable to leave the older text as the state at that time. But the older entry is still easy to find and directly contradicts the current Plan if read in isolation.

**Impact:**

- Low risk because later correction exists.
- Still a possible source of confusion for release engineers searching for “smoke test” in the history file.

**Suggested fix:**

- If hardening-history entries are allowed to receive clarification notes, add an inline parenthetical pointing to the later correction.
- Otherwise, leave as historical record but ensure the current normative `ANVIL_PLAN.md` Open Item is unambiguous and matches the Distribution section.

---

## Overall Assessment

R13 is much closer to clean:

- All executable validation passes.
- The R12-specific `v1 usage` misses at lines 823 and 841 are fixed.
- The P11 PL hinge test was renamed and all current references appear updated.
- The PL table extraction filter is now trim-normalized.
- Gate 2 AC4 now has an Open Items tracking entry.

Remaining issues are documentation consistency issues, not implementation failures. The main blocker to a fully clean documentation pass is the new Leaflog identity mismatch and the smoke-test Open Item’s divergence from the Distribution smoke-test definition.

Recommended minimum before final approval:

1. Correct the Leaflog Open Items description to match the houseplant watering journal artifacts.
2. Align the Gate 2 AC4 Open Item with the Distribution smoke-test command list, including `anvil-sidecar --version`.
3. Qualify the remaining “v1 usage” / “P11 observational data” references as Gate 2 future evidence or build observations.