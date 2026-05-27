# Dogfooding: Anvil v1.1 App Design

> **Representative artifacts — not a live CLI execution.** The artifacts below are illustrative of what an Anvil v1 dogfooding session would produce. They are not live exports from an actual `anvil discuss` / `anvil charter review` / `anvil plan invoke` execution against real AI providers. See `docs/examples/coordinator-attestation.md`.

**Session:** Anvil v1.1 charter and plan — representative of output expected from the Anvil v1 CLI (live execution deferred; see coordinator-attestation.md)  
**Date:** 2026-05-26  
**Operator:** jvcan (Coordinator)  
**CLI version:** v1.0.0 (the build produced by P0–P11)

---

## What This Is

This directory contains representative artifacts showing what an Anvil v1 dogfooding session on the v1.1 App design problem would produce. The actual dogfooding session — running the CLI against real AI providers — is deferred to before public ship (see `docs/examples/coordinator-attestation.md`).

The dogfooding exercise is the Plan's primary acceptance test: "Anvil v1 can manage the Anvil v1.1 design (Charter through Plan) without manual orchestration."

---

## What a Live Session Would Reveal

*The following is based on build observations and UX audit data collected during P0–P11. The Provisional Lock evaluations below are real governance decisions; the specific CLI interaction details are representative of what a live run would produce.*

### Anticipated workflow gaps

A live Charter → Plan cycle on the v1.1 design is expected to surface no workflow-blocking failures and no gaps requiring earlier phases to reopen. UX friction identified from build observations, expected to appear in a live run:

- `anvil discuss` session state is not persisted — if the terminal is closed mid-session, the charter must be reconstructed from memory. *(Logged as UX gap in `docs/ux-audit.md`)*
- The `anvil plan invoke` blocking wait (no streaming output) is more noticeable with a longer charter (v1.1 charter is substantially longer than a typical user project). Wait expected to be approximately 75 seconds.
- `anvil arbiter resolve-finding` composite ID format: consistent friction, same as the pilot project.

### Provisional Lock reviews triggered at the v1.1-prep boundary

Two Provisional Locks had `revision trigger = v1.1 App design begins`. These are real governance decisions made during P11 build, using the UX audit and build observations as evidence:

**`cli-setup-wizard-step-ordering`** — The setup wizard step ordering was evaluated against v1.1 App design requirements. A GUI can show all inputs on a single screen rather than sequentially, so the App wizard has latitude to differ. **Status: confirmed Final at P11 (v1 decision locked; v1.1 App wizard will independently design its own step ordering).**

**`cli-command-structure`** — The `<resource> <verb>` CLI structure was evaluated against App UI mapping. Two friction points identified in `docs/ux-audit.md`: `arbiter resolve-finding` (requires manual ID construction) and `hinge flip` (requires knowing the `intended` ID vs. function name); the App can solve both via UI-mediated selection. **Status: confirmed Final at P11 (v1 decision locked; v1.1 App addresses friction via UI-mediated selection).**

Both v1 decisions are Final. See `ANVIL_PLAN.md` Required Choices table (canonical slugs: `cli-setup-wizard-step-ordering`, `cli-command-structure`) and `p11.rs` hinge test `test_no_outstanding_provisional_locks_at_p11_gate1`.

---

## Artifacts

These artifacts are **representative and illustrative** — they show the charter and plan output that an Anvil v1 dogfooding session on the v1.1 App design would produce. They are not live exports from an actual `anvil discuss` / `anvil charter review` / `anvil plan invoke` execution against real AI providers.

- `v11-charter.md` — The Anvil v1.1 Charter that such a dogfooding session would produce (final converged version)
- `v11-plan-summary.md` — Phase summary from the converged v1.1 Plan that would result

A real dogfooding session would produce full audit-store records (ReviewerFindingPacket, ConvergenceDeclaration, etc.) in the Anvil project's own `.anvil/` store alongside the v1 build records. The v1.1 charter and plan summary here are the documentary outputs of that process.
