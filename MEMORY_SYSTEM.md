# Anvil Memory System

**Status:** Implemented (memory phases M1–M4).
**Last updated:** 2026-06-17

This document describes Anvil's memory & continuity system **as built**. It supersedes
the original planned design (which predated the agent rebuild and assumed a stateless
chat with `/include` + `active_context`, both now removed).

---

## Philosophy

Memory is **context, never authority**. The source of truth is always on disk — the
reviewed `plan.md` / `REVIEW_*` files, git, and `.anvil/state.json`. Everything injected
into the coder's context is **bounded, delimited, and inspectable**; nothing is hidden.
No embeddings, no vector database — pure files + git, so it runs anywhere (including
local Ollama) and stays auditable.

The model is borrowed from a larger living-memory system but pared to the two ideas that
fit a coding agent: an **immutable ledger** + a **derived, decaying working set**.

---

## What the coder receives every turn

At the start of each turn a single **grounding message** is prepended to the request. It
is kept *out* of persistent history, so it is always current and never accumulates:

1. **`--- WORKING MEMORY ---`** — the contents of `.anvil/working-memory.md`, bounded to
   ~4000 chars. If its newest entry is older than a 10-day halflife, a staleness note is
   prepended: *"last updated X — about N days ago; verify against plan/git, may be
   outdated."*
2. **`--- REALITY SNAPSHOT ---`** — rebuilt fresh from disk + git every turn (bounded
   ~4000 chars): workflow stage (with a stale-plan hash check), the current phase + its
   `plan.md` excerpt, shipped phases, and git branch / `status --short` / diff stat.

Then a **recent send-window** of the conversation: the last ~40 messages, capped at a
~60k-token char budget, trimmed to start on a clean user turn. Older turns are *not* sent
verbatim — their signal lives in working memory.

---

## The three layers

| Layer | Where | Behavior |
|---|---|---|
| **Ledger** (fossil record) | `.anvil/session.json` (append-only JSONL) | Every message appended as it is committed; **never truncated**. Loaded in full on launch. Honors a `{"reset":true}` marker. Legacy single-array format auto-migrates. |
| **Working memory** (curated) | `.anvil/working-memory.md` | User-editable Markdown; written/extended by `/compact`; injected (bounded) each turn; decays past a halflife. |
| **Reality snapshot** (live) | *(none — rebuilt each turn)* | Pure disk + git; also exposed to the coder as the `project_state` tool. |

---

## Project context files

A small set of legible, user-editable Markdown files the coder maintains with its own
`write_file`/`edit_file` tools. Each has an **explicit injection policy** — no retrieval,
no ranking, no hidden mutation. They're seeded with explanatory templates on launch and
are *not* injected until they contain real content (a template's headers/comments don't
count).

| File | Holds | Injected each turn? |
|---|---|---|
| `.anvil/decisions.md` | durable preferences/conventions **+ verification commands** that worked | yes (bounded ~2k) |
| `.anvil/assumptions.md` | working hypotheses the coder has **not verified** (kept separate from facts) | yes (bounded ~2k) |
| `.anvil/scratch.md` | disposable investigation notes — not memory, not truth | **never** |
| `ARCHITECTURE.md` (repo root) | a small maintained map of the codebase (committable) | on demand |

The coder records standing preferences and confirmed verification commands in
`decisions.md`; tracks unverified beliefs in `assumptions.md` and promotes/deletes them as
it verifies; keeps `ARCHITECTURE.md` current; and uses `scratch.md` for throwaway notes.
A phase checklist (read files → minimal diff → tests → run verification → inspect diff) is
part of the coder's system prompt.

View them with `/decisions`, `/assumptions`, `/scratch`, `/architecture`; `/memory` lists
them all with sizes and injection status.

## Commands

- **`/compact`** — summarize the conversation (via the coder model) into
  `.anvil/working-memory.md` under a timestamped heading, then trim the in-memory history
  to the recent tail. The ledger is untouched; signal survives in working memory.
- **`/refresh`** — print the live reality snapshot so you can see exactly what the coder
  is grounded on.
- **`/memory`** — inspect all layers: ledger entry count, in-session history window,
  working-memory size, snapshot size, and ≈ tokens sent next turn.
- **`/clear-memory`** — reset the in-session history + empty working memory, **keeping**
  the append-only ledger (writes a reset marker so reloads start fresh). `plan.md` /
  `REVIEW_*` are never touched.
- The coder can also call the **`project_state`** tool itself anytime to re-ground.

---

## Continuity & bounds

- **Across restarts:** the agent reloads the ledger on launch (trimmed to start on a
  clean user turn); the TUI shows a *"Session continued — N prior message(s)"* note plus
  a short transcript tail.
- **Token control:** the send-window + char budget keep each request bounded no matter
  how long the project runs; when the conversation outgrows the window, a one-time note
  suggests `/compact`.
- **Auditable:** every injected block is delimited and labeled; `/memory` reports the
  totals; the ledger is the permanent, replayable record.

---

## Key parameters

- Send window: **40 messages** · context budget: **~240k chars (~60k tokens)**
- Working-memory / snapshot injection cap: **~4000 chars** each
- `/compact` keeps the last **8 messages** in memory
- Working-memory staleness halflife: **10 days** · token estimate: **chars ÷ 4**

---

## Source files

- `src/reality.rs` — `snapshot(root)` + `git_summary(root)` (the reality snapshot).
- `src/agent.rs` — the ledger (`session_path`, `append_to_ledger`, `load_session`,
  `append_reset_marker`), working-memory injection + decay (`working_memory_block`,
  `staleness_note`), `/compact` (`compact`), the send-window/budget (`context_window`),
  and the inspector hooks (`history_len`, `context_chars`, `clear_history`).
- `src/tools.rs` — the `project_state` tool.
- `src/ui.rs` — the `/compact`, `/refresh`, `/memory`, `/clear-memory` commands, the
  session-continued restore, and the `[note]` compaction nudge.
