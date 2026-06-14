# Anvil v1 CLI UX Audit

**Version:** 1.0.0  
**Date:** 2026-05-27  
**Scope:** All `anvil <resource> <verb>` commands; CLI surface only.

This document audits the Anvil v1 CLI command surface. It maps each command to its conceptual App UI equivalent, notes friction points, and records any gaps that should be addressed in v1.1.

---

## Command Inventory

The full v1 CLI surface groups into ten resource families:

| Resource | Verbs | Gate |
|---|---|---|
| `init` | _(positional)_ | Setup |
| `setup` | _(positional)_ | Setup |
| `discuss` | _(positional)_ | Setup → Charter |
| `config` | `show`, `set` | All |
| `gate` | `check-plan` | Charter → Plan |
| `charter` | `review`, `findings` | Charter |
| `arbiter` | `declare-convergence`, `resolve-finding` | Charter, Plan, Build |
| `plan` | `invoke`, `review`, `findings`, `consolidate` | Plan |
| `phase` | `build`, `review`, `findings`, `ship`, `reopen` | Build |
| `graph` | `show`, `blast-radius` | Build |
| `ship` | _(positional)_ | Build → Ship |
| `sidecar` | `status`, `start`, `stop` | All |
| `audit` | `list`, `show`, `integrity`, `provenance` | All |
| `status` | _(positional)_ | All |
| `metrics` | `show`, `history` | Post-ship |
| `hinge` | `list`, `flip` | All |

---

## Setup Stage Commands

### `anvil init <path>`

Initializes `.anvil/` at `<path>`. Idempotent.

**Friction:** `<path>` is a positional argument but most users run this from within their project directory. Passing `.` is the intended idiom, but the argument is required — there is no default. Minor discovery cost for new users.

**UI equivalent:** "New Project" wizard first step — creates the workspace on disk and advances to the setup flow.

---

### `anvil setup [path]`

Interactive wizard: provider connections, model bindings, governance choices, credentials. Writes `anvil.toml` and stores keys in the OS keychain.

**Friction:** The wizard runs entirely in the terminal with no progress indicator visible across steps. Headless mode (`--headless`) requires all five `ANVIL_API_KEY_*` variables set; partial headless invocations fail with a confusing error.

**UI equivalent:** Multi-step onboarding wizard in the App. Each wizard step maps 1:1 to a screen: name/description → provider connections → model bindings → governance choices → credentials.

**Gap:** No `anvil setup --check` command to verify credentials without re-running the full wizard. Operators must inspect `anvil config show` and probe the sidecar manually.

---

### `anvil discuss --project .`

Interactive Interlocutor session: structured dialogue with the AI to produce `charter.md`.

**Friction:** Single terminal session; no resume support. If the session is interrupted, the user starts over.

**UI equivalent:** Chat panel in the App. Session state would persist across browser sessions.

---

## Configuration Commands

### `anvil config show --project .`

Prints Required Choices, Provisional Lock status, sidecar settings, provider connections, model bindings, and reviewer pool.

**Friction:** Output is unstructured text. Machine consumers must parse it; `--format json` is absent from v1.

**UI equivalent:** Settings screen in the App, broken into tabs: Choices, Providers, Reviewer Pool, Sidecar.

---

### `anvil config set <key> <value> --project .`

Sets one of four supported keys: `sidecar.idle_timeout_secs`, `sidecar.binary_path`, `reviewer_pool`, `single_clean_pass_override`.

**Friction:** Only four keys are settable via CLI. Everything else requires direct `anvil.toml` editing. Operators touching provider connections or model bindings must use `anvil setup`.

**UI equivalent:** Inline edit for any field on the Settings screen.

**Gap:** `reviewer_pool` is set as a comma-separated string, which is error-prone. A v1.1 verb (`anvil config reviewer add/remove`) would reduce operator error.

---

## Charter Stage Commands

### `anvil gate check-plan --project .`

Verifies all Required Choices are locked (not Unlocked). Exits non-zero if any choice is unlocked.

**Friction:** The command name (`gate check-plan`) implies it only relates to the Plan stage, but it blocks the Charter → Plan transition. The naming is correct but non-obvious at first encounter.

**UI equivalent:** Status badge on the Charter stage completion screen. Blocked badge → tooltip lists unlocked choices. Resolved badge → "Advance to Plan" button becomes active.

---

### `anvil charter review --project .`

Sends `charter.md` to the next reviewer in the rotation. Spawns sidecar if not running. Writes a `ReviewerFindingPacket` audit record.

**Friction:** Progress is invisible — the command blocks for 30–90 seconds with no output until the sidecar returns. Users with slow connections may assume a hang.

**UI equivalent:** "Send to Reviewer" button in the Charter panel. Progress shown with a spinner and estimated wait time. Result displayed inline when ready.

---

### `anvil charter findings --project .`

Lists verified findings from the current `ReviewerFindingPacket` for the Coordinator to review.

**Friction:** Interactive curation is a separate command (`anvil arbiter resolve-finding`). Operators must cross-reference packet IDs manually.

**UI equivalent:** Findings tray in the Charter panel. Each finding shows severity, description, and inline buttons (Keep / Drop / Annotate).

---

## Arbiter Commands

### `anvil arbiter declare-convergence <artifact> --reason ... --project .`

Declares convergence for an artifact (e.g., `charter.md`). Writes a `ConvergenceDeclaration` record.

**Friction:** Requires `--reason` but provides no template. Operators must know the expected format (full-pool clean / human authority) from external documentation.

**UI equivalent:** "Declare Convergence" dialog with a prefilled template, round count pre-populated from audit store, and inline validation that prevents empty reasons.

---

### `anvil arbiter resolve-finding <packet_id:finding_id> --reason ... --project .`

Dispositions one finding. Writes an `ArbiterFindingResolution` record.

**Friction:** The composite `<packet_id>:<finding_id>` format (e.g., `uuid-here:F1`) is not surfaced by `anvil charter findings`; operators must copy it from `anvil audit list`. The finding ID sub-component format (`F1`, `F2`, …) is implicit.

**UI equivalent:** Inline "Resolve" action on each finding card. No composite ID entry required — the packet context is inherited from the current view.

---

## Plan Stage Commands

### `anvil plan invoke --project .`

Invokes the Planner model against the approved charter; validates and writes `ANVIL_PLAN.md`.

**Friction:** Blocks for the full AI round-trip. No `--dry-run` or `--prompt-only` flag to preview what the Planner will receive without committing.

**UI equivalent:** "Generate Plan" button in the Plan stage screen. Streaming output displayed as the plan is generated.

---

### `anvil plan review --project .`

Sends `ANVIL_PLAN.md` to the current reviewer. Writes a `ReviewerFindingPacket`.

**UI equivalent:** Same as `anvil charter review` but scoped to the Plan panel.

---

### `anvil plan findings --project .`

Lists plan review findings for curation.

**UI equivalent:** Findings tray in the Plan panel; same UX as charter findings.

---

### `anvil plan consolidate --trigger <description> --project .`

Absorbs hardening notes into the plan, bumps version, writes `PlanConsolidationRecord`.

**Friction:** `--trigger` defaults to `"end-of-phase"` but there is no enumeration of valid trigger values. Operators pass arbitrary strings; downstream tooling can't group by trigger type.

**UI equivalent:** "Consolidate Plan" action triggered automatically at phase ship, or manually from the Plan version history screen.

---

## Build Stage Phase Commands

### `anvil phase build <id> --project .`

Invokes the Coder for a phase; produces the Phase Review Briefing. Writes a `PhaseBriefingRecord`.

**Flags:** `--format text|json` · `--describe-schema` (prints the `PhaseBriefingContract` JSON Schema and exits)

**Friction:** `--describe-schema` only exists for `phase build`, not for other commands that produce structured outputs. Inconsistent discoverability.

**UI equivalent:** "Build Phase" button in the phase detail view. Streaming Coder output shown in a code panel; briefing appears when complete.

---

### `anvil phase review <id> --project .`

Sends the phase briefing to the next reviewer. Writes a `ReviewerFindingPacket`.

**UI equivalent:** "Send to Reviewer" button in the phase detail view.

---

### `anvil phase findings <id> --project .`

Lists phase review findings for curation.

**UI equivalent:** Findings tray in the phase detail view.

---

### `anvil phase ship <id> --project .`

Ships a phase. Requires full-pool clean termination (or `single_clean_pass_override`). Writes `PhaseDisposition` and `GateApproval` records.

**Friction:** No `--dry-run` flag; the ship gate is all-or-nothing. Operators can inspect gate state via `anvil audit list` but must mentally assemble the full picture.

**UI equivalent:** "Ship Phase" button, enabled only when the gate passes. Blocked state shows the unmet conditions inline.

---

### `anvil phase reopen <id> --reason ... [--yes] --project .`

Reopens a shipped phase and invalidates all transitive dependents. Writes `RollbackEvent` records.

**Friction:** `--yes` skips confirmation but is not documented prominently. Operators running CI jobs must explicitly pass `--yes` or the command blocks waiting for stdin.

**UI equivalent:** "Reopen Phase" destructive action behind a confirmation dialog listing all transitive dependents that will be invalidated.

---

## Graph Commands

### `anvil graph show --project .`

Displays all phases and their direct dependencies.

**UI equivalent:** Phase dependency graph view — a DAG rendered visually in the App.

---

### `anvil graph blast-radius <phase_id> --project .`

Shows transitive dependents of a phase.

**UI equivalent:** Hover or click on a phase node to highlight transitive dependents.

---

## Project Ship Command

### `anvil ship --project .`

Ships the project. Checks all phases shipped, no unresolved rollbacks, hinge consensus clean. Runs configured transport actions.

**Friction:** Transport action output (e.g., `git push` stdout) is mixed into CLI output with no prefix. Hard to distinguish from Anvil's own status messages.

**UI equivalent:** "Ship Project" screen with a pre-flight checklist (each gate shown as pass/fail), then a confirmation dialog, then a real-time log of transport actions.

---

## Status and Observability Commands

### `anvil status --project . [--artifact <artifact>]`

Shows phase ship status, unresolved rollbacks, rotation position, round count, advisory finding count, and pool clean status.

**Friction:** `--artifact` defaults to `"charter.md"`. Operators working in the Build stage rarely need charter-scoped status; the default choice is less useful in practice than a Build-stage default.

**UI equivalent:** Project dashboard — persistent header or sidebar showing rotation position, phase completion, and alert count.

---

### `anvil sidecar status/start/stop --project .`

Lifecycle management for the sidecar daemon.

**Friction:** `anvil sidecar start` is rarely needed explicitly because every AI command starts it automatically. The command is mostly useful for debugging. Its existence implies the user needs to manage daemon state manually, which is not the intended model.

**UI equivalent:** System tray icon (desktop App) showing sidecar health. Start/stop available from right-click menu.

---

### `anvil audit list <type> --project .`

Lists audit records of a given type. Accepts kebab-case (`gate-approval`) or PascalCase (`GateApproval`).

**Friction:** No `--format json` flag in v1. Machine consumers must parse the human-readable list output.

**UI equivalent:** Audit log screen with type filter, time range filter, and record detail drawer.

---

### `anvil audit show <id> --project .`

Prints the full JSON of one record.

**UI equivalent:** Record detail drawer — structured view of the JSON with field labels.

---

### `anvil audit integrity --project .`

Checks that all indexed records are physically present on disk. Exits non-zero on `BlockShip` status.

**UI equivalent:** Health indicator on the Audit Log screen. Integrity violations shown as banners.

---

### `anvil audit provenance <cross_ref_key> --project .`

Shows which audit records back a cross-reference key.

**UI equivalent:** "What backs this?" link on any Plan section or decision, opening the Audit Log filtered to backing records.

---

## Metrics Commands

### `anvil metrics show --project .`

Displays the six Layer-1 metric values with target status and active alerts.

**UI equivalent:** Metrics dashboard — sparklines for each metric with threshold bands and alert indicators.

---

### `anvil metrics history --project .`

Displays per-metric values across all shipped phases.

**UI equivalent:** Metrics history chart on the dashboard — phase-over-phase trend per metric.

---

## Hinge-Test Commands

### `anvil hinge list [--strict] [--count] --project .`

Lists all hinge-test entries with pinned values, intended IDs, flip history, and phase.

`--strict` runs the cross-language consensus check and exits non-zero if violations exist.

`--count` prints only the total entry count.

**UI equivalent:** Hinge Registry screen — table of all hinge entries with language, pin value, flip history, and phase badge. `--strict` equivalent exposed as a "Validate" button that highlights violations.

---

### `anvil hinge flip <id> --new-value <value> --reason <reason> --project .`

Records a `HingeFlip` audit entry for the given hinge intended ID.

**Friction:** Requires knowing the hinge's `intended` ID (not the test function name). The two differ; `anvil hinge list` shows both, but operators must read carefully.

**UI equivalent:** "Flip" action on each hinge row in the Hinge Registry screen. Dialog pre-populates current pin value; operator enters new value and mandatory reason.

---

## Cross-Cutting Friction

| Issue | Commands affected | Severity |
|---|---|---|
| No `--format json` output | `config show`, `audit list`, `status`, `metrics show` | Medium — blocks scripted integrations |
| No progress indicators on AI calls | `charter review`, `plan invoke`, `plan review`, `phase build`, `phase review` | Medium — operators see blank terminal for 30–90s |
| Composite ID hand-assembly | `arbiter resolve-finding` | Medium — error-prone cross-reference lookup |
| No `--dry-run` on destructive gates | `phase ship`, `ship` | Low — gate state is inspectable via audit commands |
| Non-default stdin block in CI | `phase reopen` | Low — documented; pass `--yes` |
| Missing `--describe-schema` on non-build commands | all AI commands | Low — only `phase build` supports it |
| `--format json` on `phase build` doesn't write briefing file | `phase build` | Low — documented in help text |

---

## v1.1 Recommendations

1. **`anvil config reviewer add/remove <name>`** — replace comma-string `reviewer_pool` editing.
2. **`--format json` on all read commands** — enables scripted integrations and CI dashboards.
3. **Progress streaming on all AI commands** — even a simple elapsed-time ticker reduces perceived hang.
4. **`anvil setup --check`** — credential validation without re-running the wizard.
5. **`--describe-schema` on all AI commands that produce structured JSON** — not just `phase build`.
6. **Finding ID surfaced by `anvil charter/plan/phase findings`** — pre-format the composite ID so operators can paste it directly into `arbiter resolve-finding`.
7. **`anvil audit list --format json`** — unblocks monitoring integrations.
