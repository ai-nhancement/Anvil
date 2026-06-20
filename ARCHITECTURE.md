# Architecture Map

Anvil is a single Rust binary: a real coding agent wrapped in a disciplined,
cross-vendor **two-gate review workflow**. The coder is a tool-using agent; the
value is the governed workflow around it (plan gate + per-phase gate, each with
two independent reviewers from different model families).

This map is the high-level guide; read the module's own doc-comment + code for
detail. Keep it current when module responsibilities change.

## Entry point & module layout (`src/`)

- **`main.rs`** — binary entry; declares the module list; routes CLI subcommands
  (via `cli.rs`) vs. launching the TUI (`ui::run_ui`).
- **`cli.rs`** — clap CLI surface for headless/scripted use (plan, phase, setup, update).
- **`ui.rs`** — **the TUI and the heart of the workflow** (largest module). Owns:
  ratatui rendering, the input/key handling, slash-command dispatch, the config
  wizard, popups (palette / docs / confirm / approvals), the run_command approval
  flow, and **the review-gate state machine** (`GateFlow`/`GateStep`). The coder
  agent is constructed and driven from here. Likely future split point
  (render/input/wizard/commands/popups) — do it opportunistically, not as a big bang.
- **`agent.rs`** — the coder **agent loop**: owns conversation history, the
  per-turn context assembly (task anchor + working memory + decisions/assumptions +
  repo map + reality snapshot), the immutable session **ledger** (`session.json`,
  append-only JSONL), auto-compaction ("clinkering"), the tool-dispatch loop, and
  `delegate` → specialist hand-off. `ConfirmHandle` gates `run_command`.
- **`tools.rs`** — the agent's **hands**: `read_file`, `write_file`, `edit_file`,
  `apply_patch` (preferred), `list_dir`, `grep`, `project_state`, `run_command`
  (timeout + process-tree kill + Ctrl+B interrupt), and the `delegate` tool def.
  All paths are sandboxed to the project root. Also the command-approval prefix
  matcher (`command_matches_prefixes`) and the read-only tool subset for reviewers.
- **`plan.rs`** — plan generation + the shared **reviewer**: `run_single_review`
  is the bounded, read-only investigating-reviewer loop (verifies claims against
  real files; injects decisions, prior-review memory, and R1 findings on R2).
  `run_plan_r1`/`run_plan_r2` drive the plan gate.
- **`phase.rs`** — per-phase build/review: the coder **review briefing**
  (`briefing_prompt` → `REVIEW_<id>_BRIEF.md`), the phase **diff capture**
  (`capture_git_diff`, base..tree, excludes `.anvil`/`REVIEW_*.md`),
  `build_phase_diff_content` (briefing + whole plan + diff), `run_phase_r1/r2_diff`,
  and the phase ship annotation into `plan.md`.
- **`state.rs`** — `.anvil/state.json` (workflow stage, `current_phase`,
  `phase_base`, `shipped_phases`, `accepted_plan_hash`) + path helpers
  (`active_plan_path`, `reviews_dir` = repo root).
- **`config.rs`** — `anvil.toml` schema (providers / model_bindings / roles +
  `[web_search]` + `[approvals]`); **global config** (`<OS config>/anvil/`) as base,
  per-repo `anvil.toml` overlays it (`merge`); `.env` loading; role resolution
  (`resolve_role_or_binding`).
- **`llm.rs`** — provider client (`LlmClient`): credentials, streaming
  `chat_turn_stream` (OpenAI-compat / Anthropic / Google), tool-call plumbing,
  retries, `block_on` helper.
- **`reality.rs`** — the live **reality snapshot** (stage / phase / plan slice /
  git status+diffstat) injected into the coder each turn; `cap()` bounding helper.
- **`repomap.rs`** — lightweight ranked, budgeted symbol map injected per turn so
  the coder reads fewer whole files.
- **`specialist.rs`** — scoped evidence-gathering sub-agents (`researcher`,
  `repo-scout`): registry (`SpecialistDef`), bounded `run_specialist` runner;
  outward actions gated like `run_command`.
- **`websearch.rs`** — outward ops for specialists: `search` (Tavily/Brave),
  `fetch` (URL→text), `pull_repo` (shallow clone).
- **`modelsdev.rs`** — models.dev metadata (context window / tool-call / price);
  drives per-model context budget + `/models`.
- **`talk.rs`** — pre-plan freeform discussion mode.
- **`update.rs`** — self-update: GitHub release lookup, host-target mapping,
  in-place replace; short on-disk update-check cache.

## The two gates (state machine in `ui.rs`)

`GateFlow { artifact: Plan | Phase(id), step: GateStep }` advances on async
completions (reviewer-done via `gate_rx`; coder-turn-done via `llm_rx`):

**Phase gate** (`/accept-phase`):
`BriefWriting` (coder writes `REVIEW_<id>_BRIEF.md`) → `R1Reviewing` → `R1Fixing`
→ `PausedAfterR1` (user `/continue`) → `R2Reviewing` → `R2Fixing` →
`PausedAfterR2` → `Summarizing` → `Done` → user runs `/ship-phase`.

**Plan gate** (`/lock-plan`): same loop minus the briefing (starts at `R1Reviewing`),
reviewing `plan.md` directly → user runs `/accept-plan`.

R2 deliberately re-reviews after R1's fixes (catches regressions the fixes
introduce) and is given R1's findings as a checklist.

## Artifact flow (all at repo root unless noted)

- **`plan.md`** — the coder writes it; the plan gate reviews it; hash recorded on accept.
- **`REVIEW_<id>_BRIEF.md`** — coder's per-phase handoff (what was built + why).
- **`REVIEW_<id>_R1.md` / `_R2.md`** — reviewer findings (plan: `REVIEW_plan_R*`).
- These `REVIEW_*.md` are excluded from the phase review diff so prior-round review
  text never pollutes the next round; they ARE fed to the reviewer as context.

## Memory (model-agnostic; survives hot-swapping the model)

Continuity lives on disk + re-injection, not in any model's context:
- **`.anvil/session.json`** — append-only conversation ledger (never truncated).
- **`.anvil/working-memory.md`** — auto-compacted session summary (temporal decay).
- **`.anvil/decisions.md`** + **`.anvil/assumptions.md`** — durable conventions /
  unverified hypotheses, injected every turn.
- **task anchor** (last substantive instruction) + **reality snapshot** + **repo
  map**, assembled fresh each turn in `agent.rs`.
- Reviewers get continuity via injected prior `REVIEW_*` findings (file-based memory).

## Build / verify gate

`cargo fmt` → `cargo clippy --all-targets -- -D warnings` → `cargo test`, in that
order. CI enforces fmt on master; the release workflow does not — always fmt first.
