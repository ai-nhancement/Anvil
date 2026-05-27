# Anvil v1.1 Charter

**Version:** R1 (representative final/converged form — not a live `anvil discuss` / `anvil charter review` output)  
**Produced using:** Representative of output expected from Anvil v1.0.0 CLI (`anvil discuss` + `anvil charter review`)  
**Feeds into:** Anvil v1.1 implementation (Build stage begins after v1 ships)

---

## Purpose

Anvil v1 ships a CLI. The CLI is the right first form — it reaches expert developers who can stress the workflow and surface real gaps before Anvil needs to be accessible to a broader audience.

Anvil v1.1 adds a desktop App (Tauri + React + TypeScript) that makes the same workflow accessible to people who prefer a visual interface. The App does not replace the CLI; both remain first-class. The CLI is the scripting and CI surface; the App is the interactive design surface.

The App is built on top of the same `anvil-core` Vault library as the CLI. The CLI's seven-year architectural investment in a clean library boundary is the v1.1 enabler.

---

## What the App Is

A desktop application (macOS, Windows, Linux) that presents the Anvil workflow as a visual interface:

- **Project dashboard** — phase completion status, rotation position, alert count, recent activity
- **Charter panel** — write or import the charter; send to reviewer; view and curate findings inline
- **Plan panel** — generate, view, and version the plan; findings tray; consolidation history
- **Phase detail** — per-phase build/review/ship view with streaming Coder output; findings tray; ship gate pre-flight checklist
- **Audit log** — filterable record list; record detail drawer; integrity status indicator
- **Hinge Registry** — table of all hinge entries; validate button; flip dialog
- **Metrics dashboard** — six Layer-1 metrics with sparklines and threshold bands

The App communicates with the same `anvil-sidecar` daemon as the CLI. The App and CLI can coexist in the same workspace without coordination conflicts.

---

## Core Principles

1. **CLI parity.** Every action possible in the CLI is possible in the App. The App does not expose a restricted subset.
2. **Audit store is the authority.** The App reads from and writes to the same `.anvil/audit-store/` as the CLI. There is no App-specific state.
3. **Non-destructive by default.** Destructive actions (reopen, delete) require confirmation dialogs. The `--yes` flag is exposed as a "Skip confirmation in this session" setting, not a persistent default.
4. **Streaming output is first-class.** AI calls show streaming output in the UI. The user sees partial responses as they arrive, not a blank screen for 30–90 seconds.
5. **No feature gating by surface.** Power features (hinge flip, arbiter resolve, graph blast-radius) are in the App, not hidden behind "advanced" menus.

---

## Constraints

- **Tauri + React + TypeScript.** This is the technology stack. Alternatives were considered in Plan; Tauri was chosen for its Rust core, small binary size, and native OS integration.
- **`anvil-core` as the Vault.** The App calls `anvil-core` API directly (Rust → Tauri IPC → TypeScript). The App does not call the CLI binary as a subprocess.
- **`anvil-sidecar` for AI calls.** The App does not call AI providers directly. All AI calls go through the sidecar.
- **File-system locking from v1.** The App respects the same file locks as the CLI. CLI and App can coexist in the same workspace.
- **No cloud sync in v1.1.** The audit store remains local. Cloud sync is a v1.2 design problem.

---

## Non-Goals (v1.1 scope boundary)

- Cloud sync or collaboration features
- Mobile apps
- A self-contained SaaS version
- AI provider management within the App (credentials remain in the OS keychain, configured via CLI setup or a dedicated setup screen)
- Plugin or extension system
- API server mode (the App is not an API server)

---

## Success Criteria

A user who has never seen the Anvil CLI can install the App, run `anvil setup` once (or the App's equivalent setup screen), and complete a Charter → Plan → Build → Ship cycle on a 3-phase project without reading the runbook.

---

## Provisional Locks (v1.1 start)

| Lock | Hypothesis | Revision trigger |
|---|---|---|
| `app-wizard-step-ordering` | The setup wizard in the App can present all inputs on one screen rather than sequentially | First App design sprint |
| `app-nav-structure` | Top-level navigation maps to: Dashboard, Charter, Plan, Phases, Audit, Settings | First App design sprint |
| `ipc-boundary-granularity` | Tauri IPC calls map 1:1 to Vault library commands (same as CLI command boundaries) | First App implementation sprint — if IPC round-trips are too chatty for streaming, this is revisited |

---

## Governance

Same as v1: BDFL model (jvcan). Maintainer admission follows `GOVERNANCE.md`. The v1.1 design is itself managed via Anvil v1 CLI — Plan through Ship.
