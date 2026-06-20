# Anvil Polish Plan (from v0.4.7 coder-eye feedback)

Addresses the remaining suggestions in `docs/anvil_feedback.md`. Each phase is
independently shippable; built + `fmt`/`clippy`/`test` green + committed per phase.
Consolidated release at the end as **v0.5.0** (intermediate releases on request).

Already done (v0.4.8 + follow-ups): hang fix, real ARCHITECTURE.md, fmt baseline
verified + stale decisions note removed, stray `.log` cleanup.

## P1 — Terminology & metaphor clarity (no logic change)
Goal: every user-facing/help string describes the SAME current workflow, and
forge metaphors are paired with plain meaning on first use.
- Align README, CLI help/comments (`main.rs`/`cli.rs`), slash help + onboarding
  text (`ui.rs`), and module docs to: build → `/accept-phase` (briefing → R1 → fix
  → /continue → R2 → fix → /continue → summary) → `/ship-phase`.
- First-use gloss for forge terms (quench/temper/clinker/smith).
Deliverable: consistent wording; no behavior change. Accept: grep shows no stale
"`/save-r1`/`/critical-r1`"-era flow text in user-facing strings.

## P2 — Config provenance in `/status`
Goal: answer "why is this repo using that model?" at a glance.
- `/status` shows each role's active model AND whether it came from the global or
  project config, plus how to change it (`/swap`, `/config`).
Deliverable: provenance lines in `/status`. Accept: with a project override, the
overridden role is labeled; global-only roles labeled global.

## P3 — `/context` readout (feedback F)
Goal: make the memory/context budget visible.
- New `/context` command: tokens used / budget / % full / compaction-imminent,
  history message count, working-memory presence. Reuses `Agent::context_chars`,
  `history_len`, per-model budget from `modelsdev`.
Deliverable: `/context` popup/lines. Accept: shows numbers that move as the
session grows; flags when near the compaction threshold.

## P4 — Risk-flag channel (feedback E)
Goal: let the coder surface "look at this NOW" mid-phase, not only at a gate.
- A `flag_risk` agent tool: renders a prominent `[risk]` line in the TUI and
  appends to `.anvil/risks.md` (visible, user-readable). Bounded; no gate needed.
- Mention in the coder system prompt as the channel for mid-task uncertainty.
Deliverable: tool + UI surfacing + file. Accept: a coder `flag_risk` call shows
in the transcript and lands in `.anvil/risks.md`.

## P5 — Gate/state smoke tests (feedback D)
Goal: a safety net around the workflow core before refactoring it.
- Tests over `state.rs` (plan-hash accept, `phase_base` on start/ship,
  `shipped_phases`), `phase.rs` (brief/diff/extract/annotate transitions,
  re-accept after new diff), `plan.rs` helpers.
Deliverable: focused test suite. Accept: `cargo test` covers start→brief→R1→R2→
ship state transitions + missing/stale artifact handling.

## P6 — Split `src/ui.rs` — DEFERRED to its own dedicated plan
`src/ui.rs` is ~7.4k lines. The feedback itself flags this as a broad, risky
refactor to do deliberately (and the TUI internals aren't yet test-covered, unlike
the gate core now covered by P5). Carving it (`ui/popups.rs`, `ui/wizard.rs`,
`ui/render.rs`, `ui/commands.rs`, keeping the `App` core + gate machine together)
will be its own plan so it gets the care + verification it needs — not bolted onto
this batch while the tool is in active use on a real project.

---

**Status: P1–P5 shipped (release v0.5.0). P6 deferred to a dedicated plan.**
