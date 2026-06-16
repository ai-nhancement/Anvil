# Anvil

![Anvil logo](anvil_logo.png)

**Anvil brings just enough structure to prevent drift in AI-assisted coding.**

Talk with a model to capture intent. Produce a plan, reviewed by *exactly two* different models from different providers. Implement phase by phase with the tool's help — each phase also gets exactly R1 + R2 reviews before you move on.

No R3+. Cross-provider by design. Ollama, local, Groq, OpenAI, xAI, Anthropic, Azure, AWS Bedrock, Google, custom gateways — all supported.

The default experience is a full interactive ratatui TUI (persistent chat + live status + workflow gates). A complete CLI is also available for headless use, scripting, or CI.

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

# Headless alternatives
anvil talk
anvil plan --fresh
anvil status
```

On first bare `anvil` (or unconfigured state) the TUI launches a fast interactive wizard:

- **Quick local Ollama** (if `ollama` is reachable on 11434) — pick live models for CODER / reviewer-R1 / reviewer-R2.
- Or **Add / update a provider** (OpenAI, xAI, Anthropic, Groq, custom, ...) + live model list + role assignment.

Press `s` (idle prompt, empty input) any time for quick re-pick of local Ollama models.

In the TUI just type to chat with your configured "coder". Use the slash commands below to drive the gates.

## The Workflow (Talk → Plan (R1+R2) → Phases)

1. **Talk** — Capture goals, constraints, open questions. Use `/include <relative/path>` liberally so the model sees real source instead of guessing.
2. **Plan gate** — `anvil plan` (or `/plan` in TUI). Generates `plan.md`, then automatically runs *exactly* two independent reviews (R1 then R2) from two different configured model bindings (different providers preferred).
3. Address the findings in the reviews, then `/accept-plan` (or `anvil plan --accept`). This records the gate and unlocks phased work.
4. **Phases** — `anvil phase start P3`, work with the coder (same grounded context tools), `anvil phase review P3` (another forced R1+R2), then `anvil phase accept P3`.

The invariant is strict: exactly two diverse reviews per gate. This is the mechanism that keeps vibe coding from drifting.

## CLI

```
Usage: anvil [OPTIONS] [COMMAND]

Commands:
  init     Initialize a new Anvil project (creates anvil.toml + .anvil/)
  setup    Interactive setup: add providers, connections, assign roles (coder, reviewer-R1, reviewer-R2)
  config   Show or edit configuration (show | add-provider)
  talk     Open an interactive Talk session with a model (captures intent, goals, open questions)
  plan     Generate / refine the phased Plan, then run exactly R1 + R2 reviews (different providers)
           (--fresh, --accept, --context <FILE>)
  phase    Work on a phase: implementation assistance + exactly two reviews when ready
           (start | review | accept | list)
  status   Show current workflow status (reviewers, gate progress, GPU/VRAM, loaded models)
  ui       Launch the full interactive TUI (default when no subcommand)
```

All commands accept `--project <path>` (defaults to `.`).

Examples:
```sh
anvil init
anvil setup
anvil talk --model coder
anvil plan --fresh
anvil plan --accept
anvil phase start P1
anvil status
```

## TUI (the main daily driver)

- Streaming LLM responses with spinner.
- Live header: workflow stage, context badge, 1–5 lines of per-GPU utilization + VRAM (NVIDIA via nvidia-smi), Ollama loaded models summary.
- Command palette (type `/`).
- Multiline input (Shift+Enter).
- Scrollable chat + commands list.
- Focused review "cards" (`/view-plan`, `/view-reviews`).
- Per-session JSONL logs in `.anvil/chat-*.jsonl` (exact system prompts, user messages with injected context, full deltas).
- `/include <path>` context injection appears to the model (and you) as a clean `--- BEGIN PROJECT CONTEXT ...` block.

### Important Slash Commands (TUI)

```
/plan              Generate (or refresh) the plan, then run exactly R1 + R2 reviews
/accept-plan       Record that R1+R2 findings were addressed; unlocks phases
/config or /setup  Providers, model bindings, roles & API keys (full wizard)
/status            Reviewers, config state, current gate progress, live GPU + Ollama /ps
/loaded            List Ollama models currently in VRAM (+ sizes)
/unload [model]    Force immediate unload (keep_alive=0)
/include <path>    Include a project file's content as context for the model
/context           List currently included files
/clear-context     Remove all included context
/view-plan         Open plan.md in a focused review popup
/view-reviews      Open the two REVIEW_*.md files in a focused popup
/help              Key bindings and commands
/quit              Exit
```

Hotkeys: `s` (quick local Ollama re-pick when input empty), arrows + Enter on lists/wizard, Esc to back out of modals/wizard, Ctrl-C or `q` (when safe) to quit.

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

Keyring is still supported as a secondary option but the env + `.anvil/.env` path is the reliable cross-platform mechanism.

You can add/update providers any time with `anvil config add-provider`, `anvil setup`, or `/config` inside the TUI. The role picker always shows live (or high-quality static fallback) models grouped by provider with provider-specific colors.

## Configuration Files

- `anvil.toml` — providers, model bindings (short names), roles (coder, reviewer_a / reviewer_b).
- `.anvil/.env` — local secret storage (auto-loaded).
- `plan.md`, `REVIEW_plan_R1.md`, `REVIEW_plan_R2.md`, phase artifacts — the source of truth for the gates.

`anvil status` is the best way to see the current state (roles, last gates, GPU, loaded models).

## Build & Development

Only Rust is required for the current version:

```sh
cargo build --release
cargo run -- --help
cargo test
```

The previous multi-crate + Go sidecar + protobuf architecture has been archived under `archive/old-v1/`.

## License

Apache-2.0 — see [LICENSE](LICENSE).
