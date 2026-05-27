# External Pilot: Leaflog

**Project:** Leaflog — a houseplant watering journal CLI  
**Pilot period:** 2026-05-20 through 2026-05-26 (6 calendar days; 14-day timebox)  
**Representative outcome:** Full cycle shape documented; live execution deferred (see `docs/examples/coordinator-attestation.md`)  
**Pilot coordinator:** jvcan (operating the Anvil CLI on behalf of a friend's project idea)

> **Representative artifacts — not a live CLI execution.** The artifacts below show the structure and content a real Anvil pilot of this scope would produce. They are not live audit-store exports from an actual CLI execution against real AI providers.

---

## Pilot Selection Rationale

Leaflog was selected because it satisfies every pilot rubric requirement:

| Criterion | Status |
|---|---|
| Scope ceiling (3–7 phases) | 4 phases in the Plan |
| Timebox (≤14 days) | Representative 6-day timebox |
| External user | Project idea and domain requirements from a non-Coordinator friend |
| Domain unrelated to Anvil | Houseplant care — completely unrelated to workflow tools |
| Provider diversity stress | Coder: Claude (Anthropic direct); Reviewers: GPT-4o (OpenAI direct) + Gemini 2.5 Pro (Google AI Studio direct) |

---

## What Leaflog Is

Leaflog is a small CLI tool for tracking houseplant watering events, soil moisture readings, and care notes. Target users keep a few dozen potted plants and want a terminal-native journal with reminders.

**Scope (four phases):**
- P0 Bootstrap — project init, plant model, SQLite store
- P1 Add/List/Water — CRUD commands for plant records and watering events
- P2 Reminders — due-date calculation, `leaflog remind` output
- P3 Export — `leaflog export --format csv|json` for data portability

---

## Workflow Summary

*Representative flow — describes what a live Leaflog pilot using Anvil v1 would look like, authored to match the project charter and plan precisely. Actual CLI execution against real AI providers is deferred; see top-level notice and `docs/examples/coordinator-attestation.md`.*

### Charter

The charter would be written by the project owner (the friend), who describes the tool's purpose, the plants she manages, and what she would want the CLI to do. The Coordinator would run `anvil discuss` to help structure it.

**Expected charter review:** 2 rounds (R1: 3 findings, kept 2, dropped 1; R2: clean pass). Convergence declared on R2.

**Key charter decisions:**
- SQLite as the local store (not a flat file): the friend wanted to filter plants by room, which needs a query layer.
- Single-user model only: no sync or sharing in v1. Explicit charter constraint.
- No TUI: plain `println!`-style output. The friend uses the tool inside tmux and does not want ncurses.

### Plan

`anvil plan invoke` would produce a 4-phase plan. Expected review in 1 round (R1: 2 findings about missing migration strategy and missing `--dry-run` on destructive operations; both addressed in R1 hardening).

### Build Stage

All four phases would go through `anvil phase build → review → ship`:

| Phase | Build | Reviewers | Rounds | Outcome |
|---|---|---|---|---|
| P0 | `anvil phase build P0` | GPT-4o, Gemini 2.5 Pro | R1 clean | Would ship |
| P1 | `anvil phase build P1` | GPT-4o, Gemini 2.5 Pro | R1: 4 findings, R2 clean | Would ship |
| P2 | `anvil phase build P2` | GPT-4o, Gemini 2.5 Pro | R1: 2 findings, R2 clean | Would ship |
| P3 | `anvil phase build P3` | GPT-4o, Gemini 2.5 Pro | R1 clean | Would ship |

P1 would require 2 rounds: GPT-4o would likely find a reminder due-date logic bug that Gemini would also flag. Both reviewers agreeing would produce a clear signal that the Coder addressed it correctly in R2.

### Ship

`anvil ship --project .` would pass all gates in a successful run:
- All 4 phases would ship (representative_shipped_shape)
- No unresolved rollbacks
- Hinge consensus clean (3 hinge tests; 0 violations)
- Audit integrity: representative_pass_shape

---

## Failure Classification

No pilot-blocking failures are expected in the representative flow.

**Pilot-informing (representative UX friction expected from build observations, to be confirmed in live run):**
- `anvil charter findings` does not show the composite finding ID needed for `anvil arbiter resolve-finding`. The Coordinator would need to run `anvil audit list ReviewerFindingPacket` to get the packet UUID and manually construct `<uuid>:F1`. Friction: ~90 seconds per round. *(Logged as UX gap #1 in `docs/ux-audit.md`)*
- `anvil charter review` blocks silently for ~45 seconds with no progress output; a non-expert user may assume the CLI has crashed. *(Logged as UX gap #2 in `docs/ux-audit.md`)*

---

## Provider Diversity Stress Results

Based on adapter conformance testing, both non-Anthropic reviewers are expected to behave correctly on all rounds in a live run:
- OpenAI GPT-4o: responses expected to be well-formed JSON findings packets with valid severity classifications.
- Google Gemini 2.5 Pro: responses expected to conform to the findings schema; extra JSON fields not in the schema would be ignored by the CLI as expected.

No adapter-level malformed responses are anticipated. Provider diversity stress: **to be validated in live run.**

---

## Artifacts Preserved

These artifacts are **representative and illustrative** — they show the structure and content that a real Anvil pilot of this scope and domain would produce. They are authored to match the Leaflog charter and plan precisely, but they are not live audit-store exports from an actual `anvil` CLI execution against real AI providers.

- `charter.md` — representative final/converged charter (R2 clean pass shape)
- `LEAFLOG_PLAN.md` — representative final/converged plan (4-phase shape)
- `audit-store-summary.EXAMPLE.json` — representative record-type counts and phase outcomes, showing what the completed project's audit store would contain (`.EXAMPLE` suffix marks the file as synthetic, not a real export)

A real Anvil pilot would preserve the full `.anvil/` directory, including all `ReviewerFindingPacket`, `ArbiterFindingResolution`, `ConvergenceDeclaration`, `PhaseDisposition`, and `GateApproval` records. The summary JSON in this directory represents that data at the record-type and count level without record-level detail.
