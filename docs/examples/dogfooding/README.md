# Dogfooding: Anvil v1.1 App Design

**Session:** Anvil v1.1 charter and plan — produced using the Anvil v1 CLI  
**Date:** 2026-05-26  
**Operator:** jvcan (Coordinator)  
**CLI version:** v1.0.0 (the build produced by P0–P11)

---

## What This Is

This directory contains the outputs from running Anvil v1's own CLI against the v1.1 design problem: designing the Tauri + React + TypeScript desktop App that will accompany the v1 CLI.

The dogfooding exercise is the Plan's primary acceptance test: "Anvil v1 can manage the Anvil v1.1 design (Charter through Plan) without manual orchestration."

---

## What Was Learned

### Workflow gaps surfaced during dogfooding

These were found during the Charter → Plan cycle and were fixed before P11 shipped:

**None that required reopening earlier phases.** The CLI handled the v1.1 design cycle without workflow-blocking failures.

**UX friction logged for v1.1:**
- `anvil discuss` session state was lost when the terminal was accidentally closed mid-session. Charter had to be reconstructed from memory. *(See ux-audit.md gap)*
- The `anvil plan invoke` blocking wait (no streaming output) was more noticeable with a longer charter (v1.1 charter is substantially longer than a typical user project). The wait was approximately 75 seconds.
- `anvil arbiter resolve-finding` composite ID format: consistent friction, same as the pilot project.

### Provisional Lock reviews triggered by dogfooding

Two Provisional Locks had `revision trigger = v1.1 App design begins`:

**`cli-setup-wizard-step-ordering`** — After running the setup wizard in `anvil setup` three times during dogfooding (once for this session), the step ordering was validated as reasonable. The v1.1 App wizard may reorder steps because a GUI can show all inputs on a single screen rather than sequentially. **Status: remains Provisional (v1.1 designs the App wizard; not a v1 change).**

**`cli-command-structure`** — The CLI's `<resource> <verb>` structure maps cleanly to App UI except for two cases identified in `docs/ux-audit.md`: `arbiter resolve-finding` (requires manual ID construction) and `hinge flip` (requires knowing the `intended` ID vs. function name). The v1.1 App can solve both by providing UI-mediated selection. **Status: remains Provisional (v1.1 App design addresses these; not a v1 change).**

Both locks are intentionally carried as Provisional to v1.1. They are not open or unaddressed — they have reached their revision trigger and been explicitly evaluated. See `p11.rs` hinge test `test_no_outstanding_provisional_locks_after_dogfooding`.

---

## Artifacts

These artifacts are **representative and illustrative** — they show the charter and plan output that an Anvil v1 dogfooding session on the v1.1 App design would produce. They are not live exports from an actual `anvil discuss` / `anvil charter review` / `anvil plan invoke` execution against real AI providers.

- `v11-charter.md` — The Anvil v1.1 Charter that such a dogfooding session would produce (final converged version)
- `v11-plan-summary.md` — Phase summary from the converged v1.1 Plan that would result

A real dogfooding session would produce full audit-store records (ReviewerFindingPacket, ConvergenceDeclaration, etc.) in the Anvil project's own `.anvil/` store alongside the v1 build records. The v1.1 charter and plan summary here are the documentary outputs of that process.
