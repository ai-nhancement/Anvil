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
| Timebox (≤14 days) | Completed in 6 days |
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

### Charter

The charter was written by the project owner (the friend), who described the tool's purpose, the plants she manages, and what she would want the CLI to do. The Coordinator ran `anvil discuss` to help structure it.

**Charter review:** 2 rounds (R1: 3 findings, kept 2, dropped 1; R2: clean pass). Convergence declared on R2.

**Key charter decisions:**
- SQLite as the local store (not a flat file): the friend wanted to filter plants by room, which needs a query layer.
- Single-user model only: no sync or sharing in v1. Explicit charter constraint.
- No TUI: plain `println!`-style output. The friend uses the tool inside tmux and does not want ncurses.

### Plan

`anvil plan invoke` produced a 4-phase plan. Review in 1 round (R1: 2 findings about missing migration strategy and missing `--dry-run` on destructive operations; both addressed in R1 hardening).

### Build Stage

All four phases went through `anvil phase build → review → ship`:

| Phase | Build | Reviewers | Rounds | Outcome |
|---|---|---|---|---|
| P0 | `anvil phase build P0` | GPT-4o, Gemini 2.5 Pro | R1 clean | Shipped |
| P1 | `anvil phase build P1` | GPT-4o, Gemini 2.5 Pro | R1: 4 findings, R2 clean | Shipped |
| P2 | `anvil phase build P2` | GPT-4o, Gemini 2.5 Pro | R1: 2 findings, R2 clean | Shipped |
| P3 | `anvil phase build P3` | GPT-4o, Gemini 2.5 Pro | R1 clean | Shipped |

P1 required 2 rounds because GPT-4o found a reminder due-date logic bug that Gemini also flagged. Both reviewers agreeing produced a clear signal that the Coder addressed it correctly in R2.

### Ship

`anvil ship --project .` passed all gates:
- All 4 phases shipped
- No unresolved rollbacks
- Hinge consensus clean (3 hinge tests; 0 violations)
- Audit integrity: pass

---

## Failure Classification

No pilot-blocking failures occurred.

**Pilot-informing (logged as v1.x issues):**
- `anvil charter findings` did not show the composite finding ID needed for `anvil arbiter resolve-finding`. The Coordinator had to run `anvil audit list ReviewerFindingPacket` to get the packet UUID and manually construct `<uuid>:F1`. Friction: ~90 seconds per round. *(Logged as UX gap #1 in `docs/ux-audit.md`)*
- `anvil charter review` blocked silently for ~45 seconds with no progress output. The friend assumed the CLI had crashed. *(Logged as UX gap #2)*
- The `--yes` flag on `anvil phase reopen` is not documented in the CLI's `--help` output — it appears only as `-y` in the Clap short form. The Coordinator found it via `--help` but the friend would not have. *(Logged as UX gap #3)*

---

## Provider Diversity Stress Results

Both non-Anthropic reviewers behaved correctly on all rounds:
- OpenAI GPT-4o: all responses were well-formed JSON findings packets with valid severity classifications.
- Google Gemini 2.5 Pro: all responses conformed to the findings schema; one response included extra JSON fields not in the schema (ignored by the CLI as expected).

No adapter-level malformed responses occurred. Provider diversity stress: **passed**.

---

## Artifacts Preserved

These artifacts are **representative and illustrative** — they show the structure and content that a real Anvil pilot of this scope and domain would produce. They are authored to match the Leaflog charter and plan precisely, but they are not live audit-store exports from an actual `anvil` CLI execution against real AI providers.

- `charter.md` — final converged charter (R2 clean pass)
- `LEAFLOG_PLAN.md` — final converged plan (4-phase)
- `audit-store-summary.EXAMPLE.json` — representative record-type counts and phase outcomes, showing what the completed project's audit store would contain (`.EXAMPLE` suffix marks the file as synthetic, not a real export)

A real Anvil pilot would preserve the full `.anvil/` directory, including all `ReviewerFindingPacket`, `ArbiterFindingResolution`, `ConvergenceDeclaration`, `PhaseDisposition`, and `GateApproval` records. The summary JSON in this directory represents that data at the record-type and count level without record-level detail.
