# Anvil Memory System — Planned Design

**Status:** Planned / Not yet implemented.  
**Date documented:** 2026-06-14  
**Context:** Captures the agreed direction after the interactive TUI (ratatui) was built and the conversation about how the AI partner (Grok) maintains continuity during long coding sessions.  
**Goal:** Bring structure to "vibe coding" by giving Anvil a memory approach that feels natural to strong coding agents while staying 100% faithful to Anvil's core contracts.

---

## Motivation

Anvil was originally created to solve **drift** — the main failure mode of earlier unstructured AI coding attempts. The current foundation (Talk → `/plan` (exactly R1 + R2 reviews) → explicit `/accept-plan` → phased build with the same two-review gate) already provides excellent long-term memory through reviewed artifacts.

However, the conversational / working memory inside the TUI is currently very lightweight and ephemeral:

- `messages: Vec<String>` lives only for the current TUI run.
- `active_context` (from `/include`) is powerful but manual and session-scoped.
- No cross-restart continuity for the chat log.
- LLM turns in the TUI are mostly stateless (system + current user message + optional active_context). The old headless `anvil talk` does a crude history replay.
- "Reality" (git state, current plan slice, changed files) is only pulled in limited ways (e.g. crude phase excerpt in `phase.rs`).

The user observed that the AI coding partner (this session) stays remarkably coherent across a very long thread by using:
- High-signal **compacted summaries** injected at boundaries.
- **Aggressive re-grounding** (fresh tool calls to `git status`, reading specific source files, etc. on every significant turn instead of trusting internal recall).
- **Explicit lightweight "what am I working on right now"** tracking that is separate from the raw conversation firehose.
- Layered memory (short / medium / long term).
- Everything important remains **observable and user-controllable**.

The request: Reflect **something similar** into Anvil — not a direct copy, but the same spirit — so the TUI feels like a production-quality, continuous coding partner while preserving Anvil's unique strengths.

---

## Core Principles (Sacred — must never be compromised)

1. **Disk + reviewed artifacts remain the source of truth** (see `src/state.rs:2-3`).
   - Real durable memory lives in `plan.md`, `reviews/REVIEW_*.md` (exactly two per gate), `.anvil/state.json`, git history, and user-saved artifacts.
   - New memory features must make these artifacts *more* visible and useful, never replace them.

2. **Exactly two diverse reviews + explicit human accept** for any important plan or phase decision. Memory compaction or summaries that become part of the permanent record should be capable of going through the same gate process when they matter.

3. **Model-agnostic first.** Everything must work excellently with local Ollama (including `CredentialRef::None`), cheap/fast models, and all cloud providers. No reliance on heavy local embedding models or provider-specific magic unless opt-in.

4. **Add features and commands individually.** Do not build a giant memory subsystem in one go. Small, observable, shippable increments.

5. **Observable and user-controlled context.** The model should never silently receive hidden retrieved chunks. Users (and the two reviewers) must be able to see "here is exactly what context was provided."

6. **Anti-drift > convenience.** A memory feature that makes it easier to drift is worse than no memory feature.

7. **TUI is primary UX** (bare `anvil` or `anvil ui`). Headless CLI subcommands (`anvil plan`, `anvil talk`, `anvil phase ...`) must continue to work unchanged and produce identical on-disk artifacts.

8. **Someone off the street can still get started quickly.** The first-run wizard and `/config` must not become more complex because of memory features.

---

## Current Memory Situation (as of commit 352dec7)

- **Long-term / gate memory (strong):**
  - `plan.md` + `reviews/REVIEW_plan_R1.md` + `REVIEW_plan_R2.md`
  - Per-phase reviews when implemented
  - `.anvil/state.json` (very small): `current_phase`, `accepted_plan_hash`, `shipped_phases`
  - `reconcile_stage_from_disk()` derives `WorkflowStage` (Talk / PlanReviewsComplete / PlanAccepted / Unconfigured) from disk artifacts + hash. This is the mechanism that makes the gates visible in the TUI.
  - Artifacts saved from `anvil talk` (via `<artifact>` tags or `:save`).

- **Working / conversational memory (minimal):**
  - TUI `App.messages: Vec<String>` — in-memory only, capped display (last ~20), scrollable. Lost on quit.
  - `active_context: Vec<(PathBuf, String)>` — user-driven via `/include <path>`, `/context`, `/clear-context`. Full file contents (soft 12k char budget + truncation) injected into chat turns in `start_real_chat`. Survives only for the current TUI session.
  - LLM calls in TUI chat path: almost always `(system + current user text + optional active_context block)`. No prior turns carried.
  - Headless `talk.rs`: maintains a `history: Vec<(String,String)>` for the duration of that `anvil talk` session only; crude flat replay.
  - Gate calls (`plan.rs`, `phase.rs`): one-shot with full artifact + strong skeptical system prompts. Very little history fed.
  - No automatic git awareness, no changed-files list, no plan/reviews injection beyond what the user manually `/include`s or what the gate code does internally.
  - `.anvil/` exists (via `ensure_anvil_dir` in `config.rs`) and already contains `state.json` + `.gitkeep`. `reviews/` lives at project root.

- **Re-grounding:** Limited. `reconcile_stage_from_disk()` and `/status` provide some, but the chat LLM does not automatically see a fresh "what does the repo actually look like right now?"

This is deliberately minimal by design (see the comment in `state.rs`). It was the right starting point.

---

## Proposed Design: Layered Memory System (Anvil Flavor)

Mirror the successful pattern used by strong coding agents (including the AI partner in this session) while staying inside Anvil's architecture.

### 1. Long-term Reviewed Memory (Strengthen & Leverage — Mostly Existing)

- Keep `plan.md` + the two `REVIEW_*.md` files + shipped phase reviews as the canonical, human+model-reviewed record.
- After `/accept-plan` or phase accept, these become privileged context for subsequent work.
- Future enhancement: Make it easy for chat (post-PlanAccepted) to reference specific phases or review findings without the user having to manually copy text.

### 2. Medium-term Working Memory (New — the "Injected High-Signal Summary")

- New file: `.anvil/working-memory.md` (human-readable Markdown, user-editable).
- Contents (curated, not a raw dump):
  - Compact summary of the current conversation thread / session goals.
  - Fresh "Reality Snapshot" block:
    - Short `git status` / changed files since last accepted plan.
    - Current `WorkflowStage` + accepted plan hash verification.
    - Relevant slice of the current plan (or current phase) + key findings from the two reviews.
  - Key decisions, assumptions, open questions, and risks surfaced during Talk or chat.
- This file acts like the rich compaction summary that gets injected into the AI partner's context at session boundaries.
- User can edit it directly. It can be `/include`d like any other file.
- At gate points (`/plan`, `/accept-plan`, phase transitions) the system can offer to update or append to it.

### 3. Short-term Session Memory (New — Continuity)

- Persist the TUI chat transcript (or a bounded recent window of it) to `.anvil/session.json` (or a simple transcript file).
- On `App::new` (TUI startup), reload recent messages so the visible chat history and context feel continuous across `anvil` quit/restart cycles.
- For actual LLM calls (in `start_real_chat` and equivalent headless paths):
  - Send a small rolling window of recent user/assistant turns (e.g. last 6–10 exchanges).
  - Plus the current `working-memory.md` content (bounded).
  - Plus user `active_context` files (existing behavior).
  - Plus an optional fresh reality snapshot (see below).
- This gives the "recent turns" feeling without ever sending the entire history.

### 4. Re-grounding & Reality Probes (New — "I just ran git status and read the files")

- New or enhanced commands:
  - `/status` (already exists) — expand to show a clear "Reality Snapshot".
  - `/refresh` or `/reprobe` — explicitly re-scan git state, re-read the current plan + latest reviews, and update any in-memory working view.
- Automatic light re-grounding at important transitions (after `/accept-plan`, when entering a phase, on TUI start if a plan exists).
- The generated reality snapshot can be injected into chat turns (similar to active_context today) when the stage is `PlanAccepted` or later. Before the plan gate it stays minimal to avoid premature commitment.
- This directly mirrors the aggressive tool-reprobing behavior that keeps long sessions coherent.

### 5. Compaction as a First-Class Action

- New command: `/compact` (or `/summarize`).
  - Takes the current chat log + active context + reality snapshot.
  - Asks the configured planner (or a designated cheap model) for a tight, structured summary.
  - Offers to:
    - Write / append it to `.anvil/working-memory.md`.
    - Save it as a reviewable artifact under `reviews/`.
    - Use it immediately as additional context for the next turn.
- This creates the "high-quality injected summary at boundaries" effect inside Anvil.
- Because it goes through a model, important summaries can later be reviewed via the normal gate process if the user promotes them.

### 6. Observability & Control (Mandatory)

- Extend `/context` (or add `/memory`, `/ctx` alias already exists) to show:
  - What will be sent on the next chat turn (recent turns count + working-memory excerpt + reality snapshot + active files + token budget notes).
- `/clear-memory` or similar to reset session/working memory without clearing user `active_context` or the on-disk reviewed artifacts.
- All injected blocks should be clearly delimited in the actual prompt (e.g. `--- BEGIN WORKING MEMORY ---`, `--- REALITY SNAPSHOT ---`) so users and reviewers can audit them.

---

## Commands (Proposed — Added Individually)

- Existing that become more powerful: `/include`, `/context`, `/clear-context`, `/status`, `/view-plan`, `/view-reviews`.
- New (in rough priority order):
  - `/refresh` — re-ground reality snapshot.
  - `/compact` — generate and store working memory summary.
  - `/memory` or enhanced `/context` — inspect current injected memory layers.
  - (Later) `/notes` or direct editing flow for `.anvil/working-memory.md`.

All new commands appear in the `/` palette with good descriptions.

---

## Persistence Rules & .anvil/ Layout

Use the existing infrastructure:

- `ensure_anvil_dir()` (config.rs) already creates `.anvil/` + `.gitkeep`.
- `state.json` continues to hold the tiny workflow state.
- New files (examples):
  - `.anvil/session.json` — short-term chat transcript / recent turns (gitignored by convention).
  - `.anvil/working-memory.md` — medium-term curated memory (user may choose to commit this if they want it as part of the project's permanent record).
  - `.anvil/reality-snapshots/` (optional, later) — timestamped or latest-only reality blocks if we want history of probes.
- `reviews/` stays at project root (as today) because those are the reviewed, shareable artifacts.
- Update `.gitignore` (already hardened in the TUI era) to ignore `*.log`, runtime toml copies, and anything under `.anvil/` except `.gitkeep` and explicitly committed working memory.

Users can always `git add .anvil/working-memory.md` if they want durable project memory checked in.

---

## Integration with Existing Workflow

- **Pre-plan (Talk stage):** Memory is lightweight. `/include` + small recent turns + optional compaction for long exploratory chats.
- **Plan gate:** The two reviewers continue to see the full plan content + strong skeptical prompts. Working memory / recent chat can be offered as *additional* bounded context if the user wants (opt-in, visible in the review documents?).
- **Post `/accept-plan`:** Chat becomes significantly more powerful because reality snapshots, plan excerpts, and working memory are now safe and valuable to inject.
- **Phases:** `current_phase` in state + phase-specific slices from plan.md become part of the reality snapshot. Phase reviews can reference the working memory if the user chooses.
- `reconcile_stage_from_disk()` continues to be the single source of truth for `WorkflowStage`. Memory features read from it rather than maintaining parallel state.

The TUI's rich renderer (`render_message_as_lines`) and document viewer popups (`/view-plan`, `/view-reviews`) will naturally make the new memory artifacts pleasant to read (code cards, etc.).

---

## Implementation Approach (Add Individually)

Follow the established rhythm: small slices, each deliverable, observable, and reversible.

**Slice 1 (Highest immediate value, lowest risk)**
- Persist bounded chat history to `.anvil/session.json`.
- Reload recent messages on TUI startup.
- Show a small "session continued" note in the welcome/status.
- No LLM prompt changes yet.

**Slice 2**
- Reality snapshot generation (git + stage + plan slice).
- `/refresh` command + automatic light refresh on key transitions.
- Optional injection of the snapshot in chat turns (behind the PlanAccepted stage or user toggle).

**Slice 3**
- `.anvil/working-memory.md` read/write helpers.
- `/compact` command that uses the planner to produce a summary and writes it.
- Expose the working memory in the "what will be sent" view.

**Slice 4+ (Later)**
- Smarter LLM context assembly (rolling turns + working memory + reality + active_context with budgets).
- Phase-aware memory.
- Optional promotion of working memory into reviewable artifacts.
- Polish: better `/memory` inspector, token usage hints, etc.

Each slice should be small enough that it can be reviewed (even informally) before the next.

---

## Risks & Tradeoffs

- **Context bloat:** Even with budgets and compaction, long projects can still grow. Mitigation: strict truncation, stage-aware injection (pre-gate = minimal), user `/clear-*` commands, and the fact that the *real* memory is the reviewed plan.
- **Drift via summaries:** A bad compaction could mislead later turns. Mitigation: summaries are visible/editable, compaction is opt-in via `/compact`, and important summaries can be reviewed through the normal gate.
- **Local / low-resource models:** Compaction and reality snapshots must be cheap. Use the planner role (which the user has already chosen) or fall back to coder. Never require a separate heavy embedding model.
- **Complexity creep:** Every new command or file adds surface area. Mitigation: strict "add individually" rule + excellent in-palette help text.
- **Headless compatibility:** New TUI memory must not change on-disk contracts for `anvil plan`, `anvil phase`, etc. The CLI paths can optionally read the same `.anvil/working-memory.md` for context if desired, but must not be required.

---

## Open Questions (to resolve when picking this back up)

- Should compaction be fully automatic at certain gates, or always explicit via `/compact`?
- How large should the "recent turns" window be by default, and should it be configurable per binding/role?
- Do we want a separate cheap "summarizer" binding, or always reuse planner/coder?
- Should reality snapshots include actual diff content (bounded) or just file lists + stats?
- How do we surface "this memory was used in the last turn" in the chat UI for transparency?
- Should `working-memory.md` ever be sent to the two reviewers during a plan/phase gate by default, or only when the user explicitly includes it?
- Do we eventually want a "memory browser" popup (Cline-style card) for inspecting all active layers?

---

## How This Reflects the AI Partner's Memory Approach

- **Compacted summaries at boundaries** → `/compact` + `.anvil/working-memory.md`
- **Aggressive re-grounding with fresh probes** → `/refresh`, reality snapshots, automatic re-read of git/plan/state on key events
- **Explicit "what am I working on" state** → `working-memory.md` + enhanced `ProjectState` + visible `/memory` inspector (separate from the raw chat log)
- **Layered** → Long-term (reviewed artifacts), Medium (working memory + reality), Short (session transcript + rolling turns)
- **Observable & controllable** → Everything the model sees is showable via commands; user can edit, clear, or override
- **External truth wins** → Disk files, git, and the two-reviewer artifacts remain primary. The new layers are supporting context, not replacement truth.

This is the same pattern that has kept a very long, complex development thread coherent without the AI partner hallucinating prior decisions or losing track of constraints.

---

## When Picking This Back Up

1. Re-read this document + the current `src/state.rs`, relevant parts of `src/ui.rs` (App struct, start_real_chat, reconcile_stage_from_disk, /include handling, render), `src/config.rs` (ensure_anvil_dir, paths), and `src/plan.rs`.
2. Run the current TUI and note pain points around lost context.
3. Decide on the first slice (recommended: Slice 1 — persist/reload chat history).
4. Follow the established process: small implementation, test manually (or with cargo check), then consider a light review pass if the change touches gates or prompt construction.
5. Update this document with implementation notes and move completed slices to "Done" sections.

The memory system should ultimately make Anvil feel like one of the most thoughtful, structured, low-drift coding partners while remaining dead simple to start with (Ollama quick setup still works in < 30 seconds) and powerful for users who care about the two-reviewer workflow.

---

**End of planned design document.** Ready for future resumption.