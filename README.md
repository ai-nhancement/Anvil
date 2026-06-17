# Anvil

![Anvil logo](anvil_logo.png)

**Anvil brings just enough structure to prevent drift in AI-assisted coding.**

The coder is a real agent. It reads, writes, and edits files and runs commands itself — the same way Claude Code, Cursor, or Aider do — so you just point it at your repo and tell it what to build. No manually attaching files, no copy-pasting its output to disk.

What Anvil adds on top of a normal coding agent is **discipline at exactly two human gates**, so a long vibe-coding session doesn't quietly drift off the rails:

- **Plan gate** — you discuss the work, the coder writes `plan.md` itself, then two *different* model families (reviewer-a / reviewer-b) critique the plan. You approve.
- **Phase gate** — the coder builds a phase, then those two reviewers critique the actual `git diff`. You approve.

Everything between the gates is ordinary agentic chat. The human is the gate; the reviewers are a genuine cross-vendor second opinion; the coder does the work.

No R3+. Cross-provider by design. Ollama, local, Groq, OpenAI, xAI, Anthropic, Azure, AWS Bedrock, Google, custom gateways — all supported.

The default experience is a full interactive ratatui TUI (persistent chat + live status + workflow gates). A CLI is also available for headless/legacy use.

## Quick Start

```sh
# Build or install
cargo build --release
# or: cargo install --path .

# Create project files
anvil init

# Recommended: launch the full TUI (auto-starts setup wizard on first use)
anvil
# or explicitly: anvil ui
```

On first bare `anvil` (or unconfigured state) the TUI launches a fast interactive wizard:

- **Quick local Ollama** (if `ollama` is reachable on 11434) — pick live models for CODER / reviewer-R1 / reviewer-R2.
- Or **Add / update a provider** (OpenAI, xAI, Anthropic, Groq, custom, ...) + live model list + role assignment.

Press `s` (idle prompt, empty input) any time for a quick re-pick of local Ollama models.

Once configured, just type to chat with your coder. It works directly in the project directory — there's nothing to attach or include.

> **Tool-calling note:** the agent needs a model that supports tool/function calling. The hosted frontier models (Claude, GPT, Grok, Gemini, etc.) all do. Many small local Ollama models have weak or no tool-calling; when a model emits no tool calls the agent simply replies in text.

## The Workflow

Free-form agentic chat by default; structure only at the two gates.

### Plan gate

1. Talk with the coder about goals, constraints, and architecture.
2. Ask it to write the plan — it creates `plan.md` itself (phases `## P0 — Name`, each with a goal, 3–8 actions, a deliverable, and 2–5 acceptance criteria).
3. `/lock-plan` — reviewer-a (R1) then reviewer-b (R2) critique `plan.md`; their findings appear in the chat and are written to `REVIEW_plan_R1.md` / `REVIEW_plan_R2.md` at the repo root.
4. Have the coder revise `plan.md` to address the findings.
5. `/accept-plan` — records the plan hash and unlocks phase work.

### Phase gate (repeat per phase)

1. `/phase-start P0` (optional — you can also just tell the coder to start the first phase).
2. The coder implements the phase directly: it reads what it needs, edits files, and runs tests. You confirm each shell command with `/y` or `/n`.
3. `/accept-phase` — reviewer-a and reviewer-b critique the current `git diff` (plus the phase's plan excerpt); findings appear in chat and are saved as `REVIEW_P0_R1.md` / `REVIEW_P0_R2.md`.
4. Fix what they raise (with the coder), then `/ship-phase` to mark the phase done. (Re-run `/accept-phase` any time to re-review.)

The invariant: the human approves at each gate; the coder does the creative work and the file changes; two diverse reviewers supply a critical second opinion on the locked artifact (the plan, then each phase's diff).

## TUI (the main daily driver)

- Streaming responses with live tool activity (`⚙ read_file src/llm.rs` → `↳ ok`).
- `run_command` confirmation: the agent shows the command and waits for `/y` (allow) or `/n` (deny). File reads/writes/edits run automatically — that's the point.
- Live header: workflow stage, per-GPU utilization + VRAM (NVIDIA via nvidia-smi), Ollama loaded-models summary.
- Command palette (type `/`), multiline input (Shift+Enter), scrollable chat.
- Focused review "cards" (`/view-plan`, `/view-reviews`).
- Per-session JSONL logs in `.anvil/chat-*.jsonl` (system prompts, user messages, deltas, tool calls).

### Slash commands (TUI)

```
/lock-plan         Run R1 + R2 reviewers on plan.md and show their findings (the plan gate)
/accept-plan       Approve the reviewed plan (records the hash, unlocks phases)
/phase-start <id>  Set the current phase, e.g. P0 (optional — you can just tell the coder to start)
/accept-phase [id] Run R1 + R2 reviewers on the current git diff for the phase (the phase gate)
/ship-phase [id]   Mark the phase shipped after its reviews
/y  /n             Approve / deny a pending run_command
/plan              Reminder of how planning works
/config or /setup  Providers, model bindings, roles & API keys (full wizard)
/status            Roles, config state, current gate progress, live GPU + Ollama /ps
/loaded            List Ollama models currently in VRAM (+ sizes)
/unload [model]    Force immediate unload (keep_alive=0)
/view-plan         Open plan.md in a focused popup
/view-reviews      Open the REVIEW_*.md files (plan + current phase) in a focused popup
/help              Key bindings and commands
/quit              Exit
```

Hotkeys: `s` (quick local Ollama re-pick when input empty), arrows + Enter on lists/wizard, Esc to back out of modals, Ctrl-C or `q` (when safe) to quit.

## Agent tools

Scoped to the project root (paths that escape the root are rejected):

- `read_file`, `write_file`, `edit_file` (exact unique-match replace)
- `list_dir`, `grep`
- `run_command` (e.g. `cargo build`, `cargo test`, `git diff`) — confirmed per call

## CLI

```
Usage: anvil [OPTIONS] [COMMAND]

Commands:
  init     Initialize a new Anvil project (creates anvil.toml + .anvil/)
  setup    Interactive setup: add providers, connections, assign roles (coder, reviewer-R1, reviewer-R2)
  config   Show or edit configuration (show | add-provider)
  talk     Legacy text-only chat with a model (no tools)
  plan     Legacy one-shot plan gen + both R1+R2 (preferred: the TUI agent flow)
  phase    Legacy phase reviews (preferred: the TUI /accept-phase + /ship-phase flow)
  status   Show current workflow status (reviewers, gate progress, GPU/VRAM, loaded models)
  ui       Launch the full interactive TUI (default when no subcommand)
```

All commands accept `--project <path>` (defaults to `.`). The `talk`/`plan`/`phase` subcommands are legacy text-only paths kept for scripting; the agentic experience lives in the TUI.

## Providers & Secrets (the part that "just works")

Anvil supports a wide set of providers out of the box with live model enumeration where the provider exposes `/v1/models` or Ollama's `/api/tags`:

- OpenAI, xAI (Grok), Groq, Together, Fireworks, OpenRouter, Mistral, Perplexity, DeepSeek, Cohere, ...
- Local: Ollama (http://localhost:11434/v1), LM Studio
- Enterprise: Azure OpenAI, AWS Bedrock, Google Vertex, Gradient
- Direct: Anthropic, Google

When you paste a key during setup the system:

- Captures it as the conventional `OPENAI_API_KEY`, `XAI_API_KEY`, `ANTHROPIC_API_KEY`, ... env var for the current process.
- Writes it to `.anvil/.env` (project-local, keep the directory private).
- Future `anvil` invocations from that directory auto-load it (no shell profile changes required on PowerShell, bash, zsh, fish, WSL, Docker, CI, etc.).

Keyring is still supported as a secondary option, but the env + `.anvil/.env` path is the reliable cross-platform mechanism.

You can add/update providers any time with `anvil config add-provider`, `anvil setup`, or `/config` inside the TUI. The role picker shows live (or high-quality static fallback) models grouped by provider.

## Configuration Files

- `anvil.toml` — providers, model bindings (short names), roles (coder, reviewer_a / reviewer_b).
- `.anvil/.env` — local secret storage (auto-loaded).
- `plan.md`, `REVIEW_plan_R1.md`, `REVIEW_plan_R2.md`, `REVIEW_P*_R1.md`, `REVIEW_P*_R2.md` — the gate artifacts, at the repo root.

`anvil status` is the best way to see the current state (roles, gate progress, GPU, loaded models).

## Build & Development

Only Rust is required:

```sh
cargo build --release
cargo run -- --help
cargo test
```

The previous multi-crate + Go sidecar + protobuf architecture has been archived under `archive/old-v1/`.

## License

Apache-2.0 — see [LICENSE](LICENSE).
