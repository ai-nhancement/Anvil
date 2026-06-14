# Anvil — P11 Dogfooding R15 Findings

**Source review doc:** `Review Rounds/REVIEW_P11_DOGFOODING_R15.md`  
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

## 1. Medium — v1.1 Design Seeds still create untracked “P11 must record” data obligations even though live P11 evidence is Gate 2-deferred

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:1041`
- `Anvil Plan/ANVIL_PLAN.md:1054`
- `Anvil Plan/ANVIL_PLAN.md:1067`
- `Anvil Plan/ANVIL_PLAN.md:1091`
- Related Gate 2 criteria: `Anvil Plan/ANVIL_PLAN.md:1160-1165`
- Related attestation commitments: `docs/examples/coordinator-attestation.md:61-80`

**Problem:**

R14/R15 correctly qualified several “v1 usage” and “P11 observational data” phrases with Gate 2 language. However, the v1.1 Design Seeds appendix still contains several operational obligations phrased as immediate P11 duties:

```text
P11 must record observed mid-stream error rates on Charter/Plan rendering invocations...
```

```text
P11 must record the actual multi-workspace usage patterns of the Coordinator's own development...
```

```text
P11 and post-v1 user feedback on how many users actually hit the no-keychain case...
```

The appendix introduction says the relevant v1 data points are “typically: live Gate 2 evidence from P11 dogfooding and pilot runs,” but the seed-specific text still reads as if P11 itself must produce these measurements. P11 Gate 1 is already complete, and the formal Gate 2 list/attestation only commits to:

1. live dogfooding Charter + Plan;
2. live external pilot full cycle with multi-reviewer rotation; and
3. v1.1 Plan validation.

It does not explicitly track the seed-specific measurements above, such as mid-stream error rates, token-position distributions, multi-workspace daemon resource costs, stale-daemon sweep counts, or no-keychain user-feedback counts.

**Impact:**

- The Plan now has untracked “must record” obligations outside the Gate 2 acceptance list and coordinator attestation.
- A future release reviewer can satisfy Gate 2 AC1–AC3 and still miss the v1.1 seed data that the appendix says P11 “must record.”
- The Gate 1 / Gate 2 evidence boundary remains slightly leaky in the seed appendix: it is unclear which seed data is required before public ship, which is merely opportunistic post-v1 telemetry, and which is design-input guidance for v1.1.

**Suggested fix:**

- Rephrase seed-specific obligations to make the boundary explicit, for example:
  - “Gate 2 live dogfooding/pilot runs should record...” for data required before public ship; or
  - “Post-v1 usage should record...” for telemetry that is not a Gate 2 blocker.
- If these seed data points are release-blocking, add them to Gate 2 or to a dedicated “Gate 2 evidence capture checklist.”
- If they are not release-blocking, replace “P11 must record” with “v1.1 design should consider any Gate 2/post-v1 observations available.”

---

## 2. Low — Release-time smoke-test Open Item omits Linux unsigned-binary warning text while the Distribution section includes it

**Location:**

- `Anvil Plan/ANVIL_PLAN.md:1018`
- `Anvil Plan/ANVIL_PLAN.md:1033`
- `Review Rounds/REVIEW_P11_DOGFOODING_R15.md:56-62`

**Problem:**

R15 improves the release-time smoke-test Open Item by making the core archive-smoke scope explicit and excluding Windows daemon robustness scenarios. However, the Open Item’s unsigned-warning examples are narrower than the Distribution section.

Distribution says the smoke test verifies unsigned-binary warning text for:

```text
Windows SmartScreen, macOS Gatekeeper, Linux distribution-specific warnings
```

The Open Item says:

```text
the script must also verify unsigned-binary warning text per OS (Windows SmartScreen, macOS Gatekeeper)
```

Linux is omitted from the Open Item even though Linux x64 musl-static is listed as a stretch release platform and the Distribution section explicitly names Linux distribution-specific warnings.

**Impact:**

- Low risk because Windows x64 remains the primary required platform and the Distribution section is declared authoritative.
- Still, the Open Item is intended to let a release engineer determine smoke-test script scope from the Open Item alone. As written, it can lead them to skip Linux warning-text verification when stretch Linux artifacts are produced.

**Suggested fix:**

- Add “Linux distribution-specific warnings, when Linux stretch artifacts are produced” to the Open Item’s unsigned-warning parenthetical.
- Alternatively, avoid enumerating OS examples in the Open Item and refer directly to the Distribution section’s full warning-text list.

---

## Overall Assessment

R15 is close to clean and the executable validation is clean:

- R14 Finding 1 was a false positive: the P11 hinge test identifier is already renamed to `test_no_outstanding_provisional_locks_at_p11_gate1` in source, Plan, history, attestation, and runtime hinge output.
- R14 Finding 2 is addressed: the source comment and R15 AC table now describe the PL check as runtime bidirectional synchronization rather than an over-broad static guarantee.
- R14 Finding 3 is mostly addressed: the smoke-test Open Item now separates release-archive smoke testing from Windows daemon robustness scenarios.

Remaining issues are documentation-boundary issues, not implementation failures. Recommended minimum before final approval:

1. Clarify whether the v1.1 Design Seed “P11 must record” data points are Gate 2 blockers, Gate 2 evidence-capture guidance, or post-v1 telemetry.
2. Include Linux warning-text verification in the release-smoke Open Item when Linux stretch artifacts are produced, or defer entirely to the Distribution section’s full list.