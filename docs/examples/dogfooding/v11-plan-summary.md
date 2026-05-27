# Anvil v1.1 Plan — Phase Summary

**Date:** 2026-05-26  
**Status:** Representative (shows converged shape expected from a live dogfooding session; not a live `anvil plan invoke` output)  
**Full plan:** Lives in the Anvil v1.1 project's own `ANVIL_PLAN.md` (not yet started)

This summary shows the representative phase-level output expected from an `anvil plan invoke` dogfooding run against real AI providers.

---

## Phase Breakdown

| Phase | Name | Description | Deps |
|---|---|---|---|
| P0 | Tauri Shell | App scaffold, IPC foundation, Vault integration, CI | — |
| P1 | Setup Screen | Provider connections, model bindings, credentials (App-native wizard) | P0 |
| P2 | Charter Panel | Charter editor, review invoke, findings tray, arbiter inline | P0, P1 |
| P3 | Plan Panel | Plan generate, view, findings tray, consolidation history | P2 |
| P4 | Phase Detail | Build/review/ship views, streaming output, findings tray, ship gate | P3 |
| P5 | Dashboard | Project status, metrics sparklines, rotation position, alerts | P4 |
| P6 | Audit + Hinge | Audit log screen, hinge registry, flip dialog | P4 |
| P7 | Polish + Ship | UX review, accessibility, packaging, release signing | P5, P6 |

**Critical path:** P0 → P1 → P2 → P3 → P4 → (P5 ∥ P6) → P7.

---

## Key Plan Decisions

**IPC boundary:** Tauri IPC calls map 1:1 to Vault library commands. The App invokes the same `anvil-core` functions as the CLI, routed through Tauri's `invoke` mechanism. This is **Provisionally Locked** with revision trigger: first App implementation sprint.

**Streaming:** Phase build and AI review calls stream chunks via a Tauri event channel. The sidecar's `ChatStream` RPC is consumed by the Vault, which re-emits chunks as Tauri frontend events. The frontend renders them in a code panel.

**Setup screen vs. CLI wizard:** The App's setup screen shows all fields on one form (provider connections, model bindings, credentials). The CLI wizard's sequential step ordering is not replicated. **Provisionally Locked** (`app-wizard-step-ordering`).

**Navigation structure:** Five top-level sections — Dashboard, Charter, Plan, Phases, Audit+Hinge, Settings. **Provisionally Locked** (`app-nav-structure`).

---

## Hinge Tests (v1.1)

```
// hinge_test: pins=tauri-v2, intended=app-framework-version, phase=P0
// hinge_test: pins=react-19, intended=frontend-framework-version, phase=P0
// hinge_test: pins=1:1, intended=ipc-boundary-granularity, phase=P0
// hinge_test: pins=event-channel, intended=streaming-ipc-mechanism, phase=P4
```

---

## Plan-Level Acceptance Criteria (v1.1)

1. All 8 phases shipped
2. A user unfamiliar with Anvil CLI can complete Charter → Plan → Build → Ship on a 3-phase project using only the App
3. CLI and App coexist in the same workspace without conflict
4. All Provisional Locks resolved before v1.1 ship
