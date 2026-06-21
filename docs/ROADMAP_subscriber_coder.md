# Roadmap: subscriber coder backend (run the coder on a Claude Pro/Max subscription)

A feasibility study for letting **Claude subscribers** run Anvil's coder on their existing
Pro/Max plan — at **$0 marginal cost** — instead of requiring a metered API key. Goal: stop
turning away the (large) pool of users who have a Claude subscription but no API billing set up.

This is a **PARKED** study, not a commitment to build. Decision context:
[[project-anvil-future-directions]] (idea #3) and [[project-build-own-core-decision]]
(we keep our own Rust core; this is the *controlled, opt-in* re-entry of an external coder).
Pairs with [[user-model-setup-preference]] (cloud coder + local reviewers).

Source: read directly against the current tree on 2026-06-20. File:line references below are
live as of that read.

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

Format: **candidate point → what's there today → verdict.**

### ✗ NOT at the per-turn call boundary — `src/llm.rs:1147` `chat_turn_stream`

The client is already provider-dispatched on `conn.type` (`anthropic` / `openai_compat` /
`google`). Adding a `claude_code` type *here* is the obvious-but-wrong move. This function's
contract is **"stream text, return the `tool_calls` the model wants, and Anvil executes them"**
(consumed at `src/agent.rs:859`). Headless Claude Code runs **its own** tool loop and writes files
**itself** — it never hands tool calls back for us to run. It cannot satisfy this contract. Dead end.

### ✓ At the coder construction point — `src/ui.rs:3036`

This is where the coder `Agent` is built and driven (`Agent::new(...)` at 3036, `run_turn` at
3075). A Claude Code backend branches **here**: when the coder role resolves to a `claude_code`
provider, *don't* build the native `Agent` — instead spawn `claude -p --output-format stream-json`
and translate its events onto the channel the TUI already drains.

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

## 4. Proposed shape (if built)

A `CoderBackend` resolved from the coder role's provider type:

- **`Native`** → today's `Agent` (default; the product identity). Unchanged.
- **`ClaudeCode`** → a new `src/claude_code.rs`:
  - spawn `claude -p <brief> --output-format stream-json [--resume <session_id>]` via
    `tokio::process`
  - parse the stream-json event lines; map text deltas → token sends, `tool_use`/result events →
    `[tool-start]` / `[tool-end]` strings on the existing channel
  - persist the returned `session_id` per project (mirror how `state.json` is stored,
    `config.rs:175`) for cross-turn continuity
  - on completion, files are already written to the working tree → the existing
    `/accept-phase → reviewers → gates` flow proceeds unchanged

Anvil still owns the **plan** and the **phase brief** (the *what*); Claude Code handles the *how*.
Everything above the backend line — plan, gates, reviewers, audit of the *committed work* — is
literally untouched, so "keep Anvil the way it is" stays true at the workflow level.

---

## 5. Effort

- **MVP** (spawn `claude -p` for the coder, pipe tokens to the TUI, files land in the tree,
  reviewers/gates run unchanged): a focused **few days**. Bulk = stream-json→tagged-channel
  translation + session-id continuity. New `src/claude_code.rs` + one branch at `ui.rs:3036` + a
  config type.
- **Parity version** (preserve confirmation gating, ledger/audit, grounding equivalence, graceful
  throttle handling): **substantially more**, and partly **impossible** (ledger/grounding can't be
  forced onto Claude Code).

---

## 6. Verdict + the decision that gates "worth it"

The architecture is **more favorable than expected**: one branch point (`ui.rs:3036`), an
engine-agnostic gate layer (`phase.rs`), and type-dispatched config (`config.rs`). The wiring is
easy. The cost is **parity**: the subscriber path gives up the coder-side machinery (grounding,
ledger, gating) that is arguably part of what makes Anvil *Anvil*.

So the build/no-build hinge is one question:

> **Is the subscriber coder allowed to be a different, thinner coder** — Claude Code doing its thing
> *inside* Anvil's gates, without Anvil's grounding/ledger/gating?

- **Yes** → cheap, the MVP is real, the wedge ("$0 for subscribers") is reachable soon. The gates
  still carry the discipline story; we just don't own the writer's internals for that user.
- **Must match the native coder** → expensive and partly impossible → not worth it.

**Recommendation:** if pursued, build the **thin MVP** explicitly framed as "Claude Code coder,
inside Anvil's gates" — accept the reduced coder-side fidelity as the deliberate trade for $0
onboarding, and keep `Native` the default. Do **not** chase parity. Revisit only after the MVP
proves subscribers actually convert.

**Status: PARKED — ideation, awaiting a go-ahead.**
