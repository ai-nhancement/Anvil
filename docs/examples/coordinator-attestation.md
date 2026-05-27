# Coordinator Attestation — P11 Dogfooding and External Pilot

**Attestation date:** 2026-05-27  
**Coordinator:** jvcan (john@ai-nhancement.com)  
**Scope:** Gate 2 AC1–AC3 (dogfooding, external pilot, and v1.1 Plan validation)

---

## What This Document Is

Gate 2 AC1–AC3 require that P11 be evidenced by actual Anvil v1 CLI executions against real AI providers before public ship:

- **Gate 2 AC1:** The dogfooding test in P11 has produced a Charter and Plan for Anvil v1.1 using the v1 CLI alone (live execution; actual audit-store records preserved).
- **Gate 2 AC2:** At least one external, non-self-referential project has completed a full Charter → Plan → Build → Ship cycle using the v1 CLI alone, including at least one Build phase with multi-reviewer rotation (live execution; actual audit-store records preserved).
- **Gate 2 AC3:** The v1.1 Plan from live dogfooding validated as the input for the v1.1 App design.

The artifacts in `docs/examples/dogfooding/` and `docs/examples/external-pilot/` are **representative and illustrative** — they document what such cycles would produce, with content authored to match the charter and plan requirements precisely. They are not live audit-store exports from an actual CLI execution against real AI providers.

This document is the Coordinator's formal attestation explaining why representative artifacts are the appropriate **documentation deliverables** for v1's first-generation build (Gate 1), and what was validated by the build process itself. Gate 1 is satisfied for documentation only; Gate 2 (public ship) requires live audit-store evidence and remains unsatisfied.

---

## Why Live Audit-Store Evidence Is Not Available

Anvil v1 is being built for the first time. The dogfooding test requires running the finished CLI against real AI provider APIs. This creates an unavoidable build-context constraint: during the construction of Anvil, the CLI cannot be fully exercised against real providers for the following reasons:

1. **The tool is being built, not operated.** The P11 dogfooding and external pilot are acceptance tests for the *completed* CLI. During the build phase, the CLI is not yet in the state it would be when operated by a real Coordinator against real AI providers.

2. **Real provider API calls are not part of the build/test harness.** `cargo test --workspace` and `go test ./...` exercise the full implementation using deterministic fixtures, mocks, and unit tests — not live API calls. The sidecar adapters are tested against recorded responses. Running live API calls in CI is outside the build contract.

3. **The dogfooding exercise illustrates workflow structure, not AI output.** The representative artifacts illustrate that the Anvil workflow (Charter → Plan → Build → Ship), the six-gate pipeline, the audit-store record types, the arbiter resolution mechanism, and the hinge test framework are structurally correct and would produce the right artifacts. The AI-generated content in the representative artifacts (charter text, plan phases, reviewer findings) is illustrative of what real providers would produce. These artifacts are documentation deliverables, not substitutes for Gate 2 live evidence.

---

## What Was Actually Validated

The build process validated the following items that would constitute a real dogfooding run:

**CLI structure and commands:**
- Every command exercised in the example artifacts (`anvil init`, `anvil setup`, `anvil charter review`, `anvil phase build`, `anvil phase ship`, `anvil ship`, etc.) exists in the built binary with the exact argument shapes shown.
- The runbook and onboarding documents were corrected against the actual Clap definitions (P11 R2 finding response).

**Audit-store record types:**
- All 15 record types exist as typed Rust enums in `RecordType`.
- The six-gate pipeline (`ReviewerFindingPacket` → `CuratedFindings` → `ConvergenceDeclaration` → `PhaseDisposition` → `GateApproval`) is implemented end-to-end.

**Hinge test framework:**
- `anvil hinge list --count --project C:\Anvil` returns 74 (the actual count for this project's source annotations).
- `anvil hinge list --strict --project C:\Anvil` passes with zero consensus violations.

**All 8 Provisional Locks confirmed Final:**
- The hinge test `test_no_outstanding_provisional_locks_after_dogfooding` asserts the 8 canonical choice_key slugs match the Required Choices table. All 8 are confirmed Final in `ANVIL_PLAN.md`.

**UX friction points documented:**
- `docs/ux-audit.md` documents real friction observed through the build process (composite finding IDs, blocking wait on `plan invoke`, missing progress output). These are authentic — they come from operating the CLI during the build and review rounds, not from the representative pilot.

---

## What Remains for a Full Live Validation

Before v1 is declared publicly shipped, the Coordinator commits to:

1. **Gate 2 AC1 (dogfooding):** Running at least one real dogfooding cycle using the published binary against real AI providers to produce a Charter and Plan for Anvil v1.1. Results will be preserved in the project's `.anvil/` audit store.
2. **Gate 2 AC2 (external pilot):** Running at least one external, non-self-referential project through a full Charter → Plan → Build → Ship cycle using the v1 CLI, including at least one Build phase with multi-reviewer rotation, against real AI providers. Audit-store records will be preserved.
3. **Gate 2 AC3 (v1.1 Plan validation):** Validating the v1.1 Plan produced from live dogfooding as the formal design input for the v1.1 App, and recording that validation.

The representative artifacts in `docs/examples/` will remain as illustrative references alongside the live data.

This attestation is the bridge between the first-generation build (where live evidence is not available) and the first public operation (where it will be).

---

## Coordinator Sign-Off

I, jvcan, attest that:

1. The representative artifacts in `docs/examples/dogfooding/` and `docs/examples/external-pilot/` accurately illustrate what the Anvil v1 CLI would produce for the described projects; they are documentation deliverables, not Gate 2 evidence.
2. The Anvil v1 CLI has been built, tested (190 Rust tests + full Go test suite, all passing), and validated against the requirements documented in `ANVIL_PLAN.md`.
3. The UX friction points and workflow gaps documented in the example artifacts are accurately drawn from knowledge of the CLI's implementation and from operating it during the build process. Provider diversity behavior shown in the representative artifacts is expected based on adapter conformance testing; live provider call validation is a Gate 2 requirement.
4. Live evidence for Gate 2 AC1 (dogfooding), AC2 (external pilot), and AC3 (v1.1 Plan validation) will be produced and recorded before v1 is publicly announced.

**Coordinator:** jvcan  
**Date:** 2026-05-27
