# Roadmap: subscriber coder backend (run the coder on a Claude Pro/Max subscription)

A feasibility study for letting **Claude subscribers** run Anvil's coder on their existing
Pro/Max plan — at **$0 marginal cost** — instead of requiring a metered API key. Goal: stop
turning away the (large) pool of users who have a Claude subscription but no API billing set up.

**Updated version (2026-06-29)** — incorporates the "standalone module" architecture.

This is a **living design doc**. Originally parked after the initial analysis; refreshed with
a cleaner integration strategy based on a dedicated module boundary.

Decision context:
[[project-anvil-future-directions]] (idea #3) and [[project-build-own-core-decision]]
(we keep our own Rust core; this is the *controlled, opt-in* re-entry of an external coder).
Pairs with [[user-model-setup-preference]] (cloud coder + local reviewers).

Source: original analysis read 2026-06-20; module design updated 2026-06-29 against the live tree.

---

## 0. The constraint that forces the whole design

Anthropic's **subscription** (Pro/Max) and the **API key** are two separate billing paths:

- **API key** → metered per token on `/v1/messages`. This is what Anvil's core uses today.
- **Subscription** → a flat plan whose entitlement is **scoped to Claude Code** (and Anthropic's
  own apps). The `/login` OAuth credential authorizes *Claude Code*, not arbitrary third-party apps.

**Consequence:** you cannot point Anvil's own core at the raw API and bill it to a subscription.
Reusing the OAuth token in our own client is outside ToS and will break. The **only legitimate
way** to ride a subscription is to route the model calls **through headless Claude Code**
(`claude -p` / the Agent SDK) — which is a sanctioned, documented use. So "support subscribers"
necessarily means "delegate the coder to a Claude Code subprocess," not "add a provider type to
our HTTP client."

**Scope decision (the user's call): coder ONLY.** The coder is ~80% of token spend. Reviewers
stay on local models (Ollama). Net for a subscriber: **bring your Claude sub + your own GPU →
Anvil costs nothing extra.**

---

## 1. Where the seam is (and where it is NOT)

Format: **candidate point → what's there today → verdict.** (analysis still valid in 2026-06 update)

### ✗ NOT at the per-turn call boundary — `llm.rs` `chat_turn_stream`

The client is already provider-dispatched on `conn.type` (`anthropic` / `openai_compat` /
`google`). Adding a `claude_code` type *here* is the obvious-but-wrong move. This function's
contract is **"stream text, return the `tool_calls` the model wants, and Anvil executes them"**.
Headless Claude Code runs **its own** tool loop and writes files **itself** — it never hands
tool calls back for us to run. It cannot satisfy this contract. Dead end.

### ✓ At the coder construction / execution site

This is where the decision is made whether to use the native `Agent` or delegate to the
Claude Code module. When the coder role resolves to a `claude_code` provider, Anvil does
**not** build the normal `Agent`. Instead it instantiates the standalone `ClaudeCodeModule`
and calls a high-level `run_task(...)`.

The module (not Anvil core) is then responsible for spawning `claude -p`, parsing its
stream-json, and streaming progress back. See the updated design in sections 4 and 7.

---

## 2. What makes it clean (the genuinely favorable parts)

1. **Config is already type-dispatched.** Roles → bindings → `ProviderConnection.type`
   (`src/config.rs`). Adding a `claude_code` provider type is trivial plumbing; the structure was
   built for exactly this kind of extension. Role resolution (`resolve_role_full`) is untouched.

2. **The TUI streaming contract is a clean, narrow interface.** The coder talks to the UI through
   one `UnboundedSender<String>` with small tagged prefixes — `[tool-start]`, `[tool-end]`,
   `[confirm]`, `[risk]`, `[note]`, plain token deltas (documented at `src/agent.rs:11-16`, drained
   in `ui.rs`). A Claude Code adapter only needs to **emit those same strings**. It's a mapping
   layer, and `llm.rs` already has the SSE line-parsing patterns to mirror for stream-json.

3. **The gated workflow is engine-agnostic — confirmed in code, not just in theory.**
   `run_phase_review` / `run_phase_accept` (`src/phase.rs:210` / `316`) operate on the **git diff**
   and call reviewers through independent `client.chat` calls (`phase.rs:297`, `plan.rs:217`). They
   **never reference the coder.** So reviewers (local), the gates, the plan, and the audit trail are
   **completely untouched** by swapping the coder. This is the architectural property that makes
   "swap just the coder" viable instead of a rewrite: review sits downstream of the writer by design.

---

## 3. What makes it NOT a drop-in (the real cost)

The coder is not an HTTP call — it's **`src/agent.rs`**, a tool loop wrapped in a large value-add
layer that Claude Code **replaces wholesale**. Every turn, `run_turn` (`agent.rs:745`) injects and
maintains:

- task anchor + working memory + `decisions.md` / `assumptions.md` + repo map + live reality
  snapshot (`agent.rs:761-807`)
- read-dedup, the 150-step runaway cap, inline auto-compaction (`maybe_auto_compact`)
- the **immutable append-only ledger** `session.json` — the coder-side audit trail
- per-command **confirmation gating** via `ConfirmHandle` (`agent.rs:913`) — the safety story

**None of this applies to a Claude Code coder.** It does its own context assembly, its own memory
(`CLAUDE.md`), its own permissions, its own transcript. Therefore:

1. **You maintain two different coders, not one coder with two backends.** A subscriber gets a
   meaningfully different build experience than an API user.
2. **Confirmation gating doesn't translate cleanly.** Claude Code owns permissions. You either
   accept its permission model or bridge through its permission-prompt mechanism (more work).
3. **The ledger / audit trail is bypassed for subscribers.** The coder's reasoning won't land in
   `session.json`; `/memory` and `/compact` won't apply to it. The audit-trail selling point
   weakens for exactly the coder role.
4. **New dependency + fragility class.** Requires `claude` installed, logged in, on PATH, at a
   compatible version; the stream-json schema is an **external contract that can shift**. "It broke
   because Claude Code updated" becomes a support burden — and it cuts against the single-static-
   binary, one-line-install ethos for that user.
5. **Subscription is rate-limited, not unlimited.** A heavy coder loop (esp. Opus) can hit rolling
   caps mid-phase. "$0" is true on the bill, but the adapter must surface throttle state gracefully
   rather than letting a phase silently stall.

Some of these (1–4) are **not reconcilable** — you cannot make Claude Code write Anvil's ledger or
use Anvil's grounding. They are inherent to delegating the loop.

---

## 4. Proposed shape (if built) — Updated Module-Centric Design

The original thin-adapter idea still holds in spirit, but the **recommended realization is a
separate standalone module** rather than a small inline branch.

### Core principle
> Anvil ↔ Module ↔ Claude Code ↔ Module ↔ Anvil  
> "Let the module be where all the work is done and Anvil talks to the module."

### Why a dedicated module (not just "spawn inside the Agent path")

- All Claude-Code-specific complexity is isolated in one place:
  - Binary discovery and version checks
  - Child process spawning (`tokio::process`)
  - Environment hygiene (clear `ANTHROPIC_API_KEY` for the child only; the subscription
    OAuth lives in `~/.claude/.credentials.json`)
  - Flag selection and evolution (`--print`, `--output-format stream-json --verbose`,
    `--permission-mode`, `--no-session-persistence`, `--model`, `--append-system-prompt`, etc.)
  - Parsing the line-delimited stream-json protocol (`system`/`assistant`/`result`/`rate_limit_event` etc.)
  - Session ID continuity and resumption
  - Translation of Claude Code events into Anvil's UI stream (or a richer protocol)
  - Handling of heavy auto-context, caching behavior, and rate-limit signals observed in practice
- Anvil core never imports or understands Claude Code internals.

### Where the branch actually happens

- **Not** in `llm.rs` (per-turn contract is wrong).
- **Not** by forcing the native `Agent` to use a different LLM client.
- **At the coder execution site** (around `ui.rs` where `Agent::new` + `run_turn` is built for the coder role).

When the resolved coder binding points at a `claude_code` (or `claude_code_cli`) provider:

```text
if provider.type == "claude_code" {
    let module = ClaudeCodeModule::new(root, model, ...);
    module.run_task(high_level_task, tx).await?;
} else {
    let agent = Agent::new(...);
    agent.run_turn(...).await?;
}
```

The native `Agent` path is untouched for everyone else.

### Interface the rest of Anvil sees (narrow & high-level)

The module exposes a small surface:

```rust
pub struct ClaudeCodeModule { ... }

pub struct ClaudeCodeOutcome {
    pub session_id: Option<String>,
    pub final_text: String,
    pub success: bool,
    // optional: cost_info, files_touched summary, warnings, ...
}

impl ClaudeCodeModule {
    /// Primary entry point. Anvil sends a high-level directive.
    pub async fn run_task(
        &self,
        task: &str,                    // e.g. "Read plan.md and implement the next phase..."
        stream: UnboundedSender<String>,
    ) -> Result<ClaudeCodeOutcome>;
}
```

Anvil owns:
- The plan (`plan.md`)
- Phase briefings (`REVIEW_<id>_BRIEF.md`)
- The overall workflow state machine
- Git diff capture after the module finishes
- Both review gates + reviewers (unchanged)

The module owns the execution of the *writing* work using the subscriber's Claude Code login.

### High-level tasks vs per-turn loop

Because Claude Code runs its own agent loop and edits the tree directly, Anvil sends
coarse-grained tasks such as:

- "Implement the approved plan in plan.md. Make clean, reviewable changes."
- "Address the findings in REVIEW_<id>_R1.md for the current phase."
- "Write the phase review briefing for the work just completed."

The module constructs the actual prompt passed to `claude -p`, manages any resume/session
state, and streams progress back so the TUI feels alive.

### Internal module first, external process possible later

- **Phase 1 (recommended start)**: A self-contained `src/claude_code.rs` (or `src/coders/claude_code.rs`)
  compiled into the single Anvil binary. Keeps the "one-line install, single static binary"
  experience.
- **Future**: The same module can be extracted or re-implemented as a small side process that
  speaks a stdio/JSON protocol. Anvil would simply spawn `anvil-claude-module --stdio` (or let
  the user point at any compatible binary). The boundary is already designed for this.

This "standalone module" organization gives clean ownership, easier testing of the adapter
in isolation, and a natural evolution path without touching Anvil's core agent or LLM layers.

---

## 5. Effort (updated for module design)

- **MVP** (standalone module + high-level task delegation):
  - New focused `src/claude_code.rs` containing all CLI interaction, env handling, stream-json
    parsing, and translation.
  - One branch point in the coder construction flow (`ui.rs`).
  - Minimal provider type or special-case handling in config + wizard so users can select
    "Claude Code (via installed CLI + your subscription)".
  - High-level task strings from plan/phase flows.
  - Live streaming to the existing TUI channel.
  - After completion, normal git-diff + reviewer gates.
  Estimated: **a few focused days** for a working thin integration.

- The heavy parts inside the module (process mgmt, parsing the observed stream-json shape
  including `assistant` + final `result` records, correct `--verbose` + permission flags, safe
  `ANTHROPIC_API_KEY` clearing) are self-contained.

- **Parity / deeper fidelity** (trying to make the Claude Code path feel identical to the native
  `Agent` with Anvil grounding, ledger, compaction, and confirmation): still largely impossible
  and not recommended. The module approach makes this explicit and contained.

---

## 6. Verdict + the decision that gates "worth it" (updated)

The architecture remains favorable, and the **standalone module pattern makes the trade-offs
even cleaner**.

Key remaining truths:
- You will have two different coders (native `Agent` vs. Claude Code via the module).
- Anvil's grounding, ledger, auto-compaction, and per-command confirmation do not apply during
  a Claude Code run.
- Reviewers, plan gate, phase gates, diff capture, and shipped-work audit trail are 100%
  unaffected and continue to provide Anvil's discipline.

The new question is even simpler:

> Can the writing phase be delegated to a well-isolated module that drives the user's existing
> Claude Code subscription, while Anvil retains ownership of planning and the review gates?

- **Yes** (recommended path) → build the dedicated module. It becomes the place "where all the
  work is done." Anvil talks to the module at a high level. The single-binary experience is
  preserved initially; an external-process future is possible without redesigning the boundary.
- **Must keep a single identical coder implementation** → do not pursue the subscriber path.

**Recommendation (updated):** Pursue the **standalone module design**. Frame the feature as
"Claude Code coder module inside Anvil's workflow." Keep the native `Agent` as the default and
primary experience. The module gives a clean, maintainable on-ramp for subscribers at $0 marginal
cost while protecting the integrity of Anvil's review system.

Practical observations from live testing (2026-06-29):
- `claude auth status` (with `ANTHROPIC_API_KEY` cleared) shows a working claude.ai subscription login.
- `claude -p ... --output-format stream-json --verbose` is the working non-interactive form.
- Even trivial prompts cause substantial context loading + cache creation inside Claude Code.
- Final results come with rich metadata (`result`, `session_id`, usage, `total_cost_usd`).
- The module must robustly handle these characteristics.

**Status: Updated design — ready for implementation of the module.**

---

## 7. Module Interaction Model (Anvil ↔ Module ↔ Claude Code)

Explicit flow the user requested:

```
Anvil (plan / phase / chat / gates)
   │
   │  high-level task + context (plan.md, brief, state, etc.)
   ▼
ClaudeCodeModule  (the standalone module — "where all the work is done")
   │
   │  spawn + env hygiene + flags + prompt construction
   ▼
claude -p "..." --output-format stream-json --verbose ...
   │
   │  line-delimited JSON events (system, assistant, result, rate limits, ...)
   ▲
ClaudeCodeModule
   │  parse • translate • manage session • stream progress
   │
   │  outcome (final text, session_id, success, ...)
   ▼
Anvil (receives stream for TUI • captures git diff • runs reviewers • ships)
```

### What lives in the module (complete ownership)

- All direct interaction with the `claude` binary
- Correct clearing of `ANTHROPIC_API_KEY` for the child process only
- Optimal non-interactive flags and their evolution
- Parsing of Claude Code's stream-json (and fallbacks)
- Session lifecycle (persist `session_id` per project or per phase)
- Mapping events to Anvil's `UnboundedSender<String>` (or a future richer event type)
- Error, throttle, and "not logged in" handling with good user messages
- Any Claude-Code-specific instructions or context injection

### What Anvil continues to own

- The plan artifact and phase briefings
- The overall state machine (`GateFlow`)
- Reviewers (R1 + R2) and the two gates
- Git diff capture and comparison against baselines
- The immutable audit trail of *accepted* work
- TUI, slash commands, approvals policy, web search specialists, etc.
- Configuration surface for choosing the coder backend

### Communication today (same process)

Use the existing narrow streaming channel + a small outcome struct returned from `run_task`.

### Future external module path

The module boundary is deliberately designed so the implementation can later become:

- A separate small Rust binary (`anvil-claude-adapter`)
- Launched by Anvil when the coder role is bound to a claude_code provider
- Or even a user-supplied compatible executable

Only the module implementation changes; Anvil's call sites stay the same.

This satisfies the request for a clean "Anvil to module to claude code to module to Anvil" architecture.

---

## Revision History

- **2026-06-20** — Initial feasibility study. Diagnosed auth separation, why delegation through `claude -p` is the only legitimate path, and why it cannot be a normal LLM provider. Proposed thin inline adapter. Marked PARKED.
- **2026-06-29** — Major update. Introduced the **standalone module** architecture in response to the request for a clean "Anvil ↔ Module ↔ Claude Code" boundary. The module owns all CLI work; Anvil talks to the module at high level. Internal module first, external process path preserved. Updated recommended shape, effort, verdict, and added explicit interaction model. Status changed to ready for implementation of the module.
