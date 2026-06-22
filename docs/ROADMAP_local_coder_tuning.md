# Roadmap: tune a local model to natively use Anvil's tools

A study note for fine-tuning a **local open-weight model** (LoRA/QLoRA) to use **Anvil's tool
surface and agent loop** natively — so the coder stops fighting habits learned for some other
harness and actually drives Anvil's tools well.

**PARKED — ideation, no go-ahead to build.** Decision context: [[project-anvil-future-directions]]
(idea #4), [[user-model-setup-preference]] (local coder + self-host direction),
[[reference-vertex-setup]] (why frontier/hosted coders have friction here). Sibling study:
`docs/ROADMAP_subscriber_coder.md`.

---

## 0. The motivating observation

Frontier "agentic coder" models (e.g. **Grok Build**) are trained against **their own** tool
schemas and harness conventions. Inside Anvil they must instead use Anvil's tools and loop
discipline, which differ — so they underperform their native environment (observed: grok-build
struggling as the Anvil coder; today's Gemini 3.x `thought_signature` 400s are a related
schema-mismatch symptom). A **local model has no strong competing prior**; SFT'd on Anvil's own
trajectories it can learn Anvil's exact tool surface + conventions instead of approximating them.

Goal of v1: not "smarter code," just **reliable tool use** — call the right Anvil tool, with valid
args, in the right order, and follow the loop discipline (read before edit, prefer `apply_patch`,
verify with a terminating command, don't re-read, ACT don't narrate).

---

## 1. The data is already there (and self-labeled)

Anvil records exactly what supervised tool-use training needs:

- **`chat-*.jsonl`** `[prompt-log]` events = the **exact input the model saw** — system prompt +
  Anvil's tool definitions + assembled context/history (`render_prompt_for_log`, `agent.rs:392`).
- **assistant turns** = the **target**: text + `tool_calls` in Anvil's format.
- **tool results** = the loop structure (so the model learns the read→act→observe rhythm).
- **`.anvil/session.json`** = the clean append-only ledger as `ChatMessage` JSONL
  (`role/text/tool_calls/tool_call_id`, `agent.rs:34`) — near training-ready.

**Quality labels come free from the workflow:** phases that passed R1/R2 and reached
`/ship-phase` are "good" trajectories; the R1→fix→R2→fix cycles yield natural
**(rejected, chosen)** pairs for later preference tuning. Anvil's review gate is a data-labeling
engine most agent-training pipelines have to build from scratch.

---

## 2. The tool surface to teach (v1 target)

From the coder system prompt + `tools::tool_defs()` (`tools.rs`, dispatched in `agent.rs`):
`read_file`, `write_file`, `apply_patch` (preferred edit path), `edit_file`, `list_dir`, `grep`,
`run_command` (user-confirmed), `delegate` (scoped specialist), `flag_risk`.

Plus the **conventions** that make a good Anvil coder, which are exactly what a mistrained model
violates:
- read the file before editing; prefer `apply_patch` over `edit_file`; minimal diffs.
- don't re-run an identical read/list in one turn.
- verify with **terminating** commands (no watch/hang); record working commands in `decisions.md`.
- on a failed `run_command`, read the error and fix — don't stop.
- ACT with tool calls; don't narrate intent without doing it.

---

## 3. Approach

- **Phase 1 — SFT/LoRA** on accepted trajectories. Input = assembled prompt (incl. Anvil tool
  defs); output = the assistant turn (text + `tool_calls`). Teaches format + tool selection +
  loop discipline. LoRA/QLoRA fits the data volume (hundreds–thousands of turns).
- **Phase 2 — DPO** on pre-fix vs post-fix review pairs → write code/edits that pass review the
  first time. Unique to Anvil's structure; do only after SFT proves out.
- **Cheaper pre-step (try first):** distill the best trajectories into **few-shot exemplars** or a
  tighter coder system prompt / `decisions.md`. Often ~most of the benefit for ~5% of the effort,
  and fully reversible — validates whether the gap is teachable before spending on a tune.

## 4. Candidate base + tooling

- **Base model:** a small model already decent at function-calling — **Qwen2.5-Coder** (already a
  binding: `qwen2.5-coder:32b`) or the existing **`aime-imprint`** base. Pick by VRAM headroom.
- **Tune:** LoRA/QLoRA via Unsloth / Axolotl / PEFT.
- **Serve:** Ollama or vLLM behind an OpenAI-compatible endpoint — **Anvil already speaks
  `openai_compat`**, so a tuned local coder drops in as a provider with zero Anvil code changes.

## 5. Hard parts / honest caveats

- **Curation is the real work.** Filter out failures (abandoned turns, the `thought_signature`
  400s, declined commands, flailing). Garbage in = garbage out; the gates help but it's not
  automatic.
- **Volume.** One user's ledgers are LoRA-sized, not full-SFT-sized — fine for adapters, thin for
  more. It compounds as you use Anvil; **log what you drop** so coverage stays honest.
- **Eval before trusting it.** Need a held-out task set scored on *tool-use success* (valid tool,
  valid args, phase completes, passes review) — not vibes. Without this you can't tell if the tune
  helped.
- **Overfitting / personalization.** Tuning on one user's repos narrows *and* personalizes — for a
  personal/self-hosted coder that's a feature; for a shipped default it's a risk.

## 6. Payoff + flywheel

A local Anvil-native coder = **no rate limits, private, $0 marginal, no token expiry, no
schema-mismatch friction** — and it turns the workflow into a self-improving loop: use Anvil →
labeled trajectories accumulate → re-tune → better local coder → use it more. Lands exactly where
the user is headed (local coder, self-host, the `aime-imprint` experiment).

**Status: PARKED — ideation. Next step if pursued: a ledger→JSONL extraction + curation script,
then validate with the cheap few-shot pre-step before any LoRA run.**
