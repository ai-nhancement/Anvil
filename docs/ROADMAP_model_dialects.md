# Roadmap: a model-agnostic tool "toolbox" (per-model tool dialects)

A design note for letting Anvil hand each model the editing surface it works best with — its
*native* tool dialect — instead of one Codex-derived dialect for everyone, while keeping a single
canonical execution core, the sandbox, and the review gates untouched. The principle:
**don't fight the model, work with it.**

**PARKED — design, no go-ahead to build.** Decision context: [[project-anvil-future-directions]]
(idea #5), [[user-model-setup-preference]] ("make Anvil model-agnostic-friendly"; not committed to
any model in any role). Companion: `docs/ROADMAP_tool_dialect_bench.md` (the empirical harness that
*chooses* a model's dialect from data — selection in §4 here is informed by it), and
`docs/ROADMAP_local_coder_tuning.md` (a tuned local model can target a chosen dialect; or the
Generic dialect may make local models viable with no tuning at all).

---

## 0. The motivating observation

Claude and GPT work best as the Anvil coder because their native edit formats are closest to the
single dialect Anvil hands out. `apply_patch` **is** GPT/Codex-native (Anvil borrowed it;
`tools.rs:516` says "Codex-style"); Claude's native `str_replace` is one rename from Anvil's
`edit_file`. The families that struggle — Grok-build, Gemini 3.x, local open-weights — are the ones
furthest from that one dialect. The fix isn't a smarter prompt; it's handing each model the surface
it was trained to use.

---

## 1. Three layers — split the middle one

Anvil's tool stack is already three layers. Only the middle one is monolithic:

| Layer | What it is | Today | Per-model? |
|---|---|---|---|
| **1. Transport** | how a tool call is encoded on the wire | `openai_turn_stream` / `anthropic_turn_stream` / google branch (`llm.rs`) | ✅ per-provider |
| **2. Dialect** | the tool *names, schemas, edit-format* the model is asked to produce | single `tools::tool_defs()` (`tools.rs:58`) | ❌ one-size |
| **3. Execution** | the actual filesystem/command op | `tools::execute()` → `run()` (`tools.rs:414`), name-dispatched | ✅ single — KEEP |

The high-value difference between families is **the edit format**, not the tool names. Renaming
`grep`→`search` buys nothing; how you express an *edit* is what each family is RL'd on:

| Family | Native edit surface | Anvil today |
|---|---|---|
| OpenAI / Codex | `apply_patch` envelope (`*** Begin Patch`) | ✅ already this |
| Anthropic / Claude | `str_replace`(old_str/new_str, unique), `view`(range), `create`, `insert` | ✗ handed `apply_patch` |
| Grok / generic FC | plain typed args — `edit_file(old_string, new_string)`, `write_file` | ✓ has these, buried under "prefer apply_patch" |

---

## 2. The `Dialect` abstraction (`src/dialect.rs`) — applied at the transport boundary

**A dialect is a translation layer at the LLM gateway (`llm.rs`), NOT a step inside the agent loop.**
The agent loop, `tools.rs`, conversation history, and the ledger only ever see Anvil's **canonical**
tools (`edit_file` / `read_file` / `run_command` / …). The dialect translates on the wire:

```rust
pub enum Dialect { Codex, Anthropic, Generic }   // open to extension

impl Dialect {
    /// OUTBOUND: render the canonical tool set into this family's advertised schema.
    fn advertise(&self, canonical: &[ToolDef]) -> Vec<ProviderTool>;

    /// OUTBOUND: render a canonical history tool-call into this family's idiom, so a model
    /// always sees its OWN vocabulary on replay (str_replace for Claude, apply_patch for GPT).
    fn format_call(&self, msg: &ChatMessage) -> Value;

    /// INBOUND: map a model-emitted call back to a CANONICAL ToolCall — dispatching on args
    /// where the family uses one consolidated tool (see §3, Anthropic). Pure rewriting, no I/O.
    fn to_canonical(&self, raw: RawToolCall) -> ToolCall;

    /// Family-specific system-prompt addendum (loop discipline is family-tuned too — e.g. the
    /// Anthropic dialect must NOT say "prefer apply_patch").
    fn prompt_addendum(&self) -> &str;
}
```

**Why the boundary, not the loop.** The agent's confirmation gate, read-only dedup, and loop-breaker
all key on the **raw call name** (`agent.rs:866`/`870`/`913`). Normalizing *inside* the loop would
have to run before those checks or they'd see `bash` / `view` / `str_replace_based_edit_tool` and
(1) skip the `run_command` confirmation gate (a `bash` call slips through — `requires_confirmation`
only matches `run_command`) and (2) miscount reads (a `view` clears `seen_reads`, killing dedup).
Translating at the gateway means the loop only ever sees canonical names, so every existing gate
keeps working untouched. It also keeps the **ledger canonical and portable**: a session can switch
models mid-stream — Anvil's whole cross-vendor-review thesis — because history is stored canonically
and re-rendered per-model on the way out.

The dialect is still a thin adapter *over* the canonical core, never a fork of it; adding a model is
writing one small adapter at the boundary, not touching execution, sandboxing, the gates, or the
ledger.

---

## 3. The dialects (v1)

All three normalize down to Anvil's existing canonical ops
(`read_file`/`write_file`/`edit_file`/`apply_patch`/`list_dir`/`grep`/`project_state`/`run_command`/
`flag_risk`/`delegate`), so a dialect that works is immediately shippable.

### Codex — today's behavior, formalized

`apply_patch` envelope + the rest of `tool_defs()` as-is. `normalize()` is a pass-through (already
canonical). Zero functional change; this is the baseline and stays GPT's native surface.

### Generic — the agnostic floor

Plain typed function calls: `edit_file(old_string, new_string)`, `write_file`, `read_file`,
`grep`, `list_dir`, `run_command` — no patch DSL, no family-specific envelope. The
lowest-common-denominator surface every function-calling model handles. `tool_defs()` = current set
minus `apply_patch`, with the "PREFER apply_patch" framing removed from `edit_file`'s description;
`normalize()` is pass-through. **Unknown models land here and just work** — coverage degrades
gracefully instead of failing.

### Anthropic — native, highest ceiling

Swap the *edit + shell* surface to Claude's built-in tool types; keep Anvil's search/nav tools as
ordinary tools (dialects need not be all-or-nothing):

- `text_editor_20250728` + name `str_replace_based_edit_tool` — `view` / `create` / `str_replace` /
  `insert`
- `bash_20250124` — name `bash`
- plus Anvil's `read_file` / `list_dir` / `grep` / `project_state` / `flag_risk` / `delegate` as-is

`to_canonical()` maps the native surface back to canonical. **Note the editor is a *single* tool
`str_replace_based_edit_tool` carrying a `command` arg — dispatch on `args["command"]`, not the tool
name** (a name-only map fails; the action lives in the args):

| Native (Claude) | Canonical (Anvil `execute()`) |
|---|---|
| `str_replace_based_edit_tool{command:"str_replace", path, old_str, new_str}` | `edit_file{path, old_string, new_string}` |
| `str_replace_based_edit_tool{command:"view", path, view_range}` | `read_file{path, offset, limit}` |
| `str_replace_based_edit_tool{command:"create", path, file_text}` | `write_file{path, content}` |
| `str_replace_based_edit_tool{command:"insert", path, insert_line, insert_text}` | `insert_lines{...}` — **the one additive exec op needed** (see §6) |
| `bash{command}` | `run_command{command}` (re-enters the confirmation gate, `tools.rs:188`) |

These tools are **client-executed**, so Anvil still runs them through its own sandbox (`resolve()`
path confinement, `tools.rs:1122`) and the review gate still reviews the committed diff. The model
speaks its mother tongue; Anvil keeps every safety rail. **Design fork to resolve (let the bench
decide):** advertise the *real built-in tool types* — max RL fidelity, but Anthropic-transport-only
and requires a `ToolDef` extension (§8) — **vs** mimic the str_replace shape with ordinary custom
tools — portable across providers, slightly less native.

---

## 4. Selection

```
select_dialect(role_binding):
    explicit  binding.dialect = "..."   -> that
    Anthropic provider/model family     -> Anthropic
    OpenAI/Codex family                 -> Codex
    everything else                     -> Generic   (the floor — always works)
```

Per-binding override in config:

```toml
[roles.coder]
provider = "xai"
model    = "grok-code-..."
dialect  = "generic"   # optional; else inferred; else Generic
```

The inference table is a default; the **bench** (`ROADMAP_tool_dialect_bench.md`) replaces guesswork
with the argmax dialect per model from measured tool-fit. New model → Generic by default → tune a
dialect only if the ledger/bench shows it fumbling.

---

## 5. Wiring — translate on the wire, keep the core canonical

**Dialect resolution happens where the binding is known** — `src/ui.rs`, coder construction
(~`3036`) — not inside `Agent::new` from the raw model id. Two logical bindings can share a model
but differ in dialect, so resolve `binding.dialect` (override → family inference → `Generic`) at the
binding site and thread the resolved `Dialect` down to the transport call. (Review finding #3 /
Recommendation B.)

At the transport (`src/llm.rs`):
- **Outbound — advertise tools:** `openai_turn_stream` / `anthropic_turn_stream` build the advertised
  schema via `dialect.advertise(canonical_defs)` (replaces the fixed `parameters` / `input_schema`
  serialization at `llm.rs:1234`/`1437`).
- **Outbound — render history:** `build_openai_messages` / `build_anthropic_messages`
  (`llm.rs:1563`/`1608`) render canonical history tool-calls via `dialect.format_call()`, so each
  model replays its own vocabulary.
- **Inbound — parse calls:** `handle_openai_tool_stream` / `handle_anthropic_tool_stream`
  (`llm.rs:1316`/`1469`) run `dialect.to_canonical()` on each assembled call **before** it leaves
  the transport.

**Unchanged — and now provably so, because none of it ever sees a dialect:**
- `src/agent.rs` loop + every gate (confirmation, dedup, loop-breaker) — all key on canonical names.
- `tools::execute()` / `run()` — canonical core (one additive op, §6).
- Conversation history + the **ledger** — canonical only → portable across vendors mid-session.
- `summarize_args` / `result_summary` (`tools.rs:319`/`352`) — canonical names, no change.
- Reviewers / specialists — canonical read-only subset (`read_only_tool_defs`, `tools.rs:179`).
- Gates, plan, audit, sandbox.

---

## 6. The canonical invariant + the one additive exec change

Canonical execution is the fixed point: every dialect rewrites *to* it, nothing rewrites it. The
only execution-layer change v1 needs is **additive**: a small `insert_lines{path, after_line, text}`
op in `tools.rs::run()` to receive the Anthropic `insert` command (and a natural file op in its own
right). `normalize()` stays pure name/arg mapping — it must not do I/O, so `insert` can't be
synthesized from read+splice+write inside it; hence the canonical op. Everything else
(`str_replace`→`edit_file`, `view`→`read_file`, `create`→`write_file`, `bash`→`run_command`) maps
onto ops that already exist.

---

## 7. Build order

1. **Generic** — smallest change (reword `tool_defs()`, pass-through normalize), widens Anvil's
   model support to *everything*, and is the most likely local-model win. Ship first.
2. **Codex** — rename of today's behavior into the `Dialect::Codex` arm. Zero functional change;
   keeps GPT native.
3. **Anthropic** — higher-ceiling build (native built-in tools + the `insert_lines` op + the
   transport touch). Worth doing second since Claude is a daily-driver coder; let the bench settle
   the native-vs-mimic fork first.

---

## 8. Hard parts / honest caveats

- **Every dialect is a surface to validate.** The bench (`ROADMAP_tool_dialect_bench.md`) is the
  eval; don't add a dialect you can't measure. The gate ledger is the real-world cross-check.
- **Anthropic native payload format (finding #6).** When `dialect.advertise()` emits a built-in
  type it must serialize as `{"type": "text_editor_20250728", "name": "str_replace_based_edit_tool"}`
  / `{"type": "bash_20250124", "name": "bash"}` with **no `input_schema`** — the API rejects a custom
  schema on a native type. Fork to settle via the bench: native built-in types (max RL fidelity,
  Anthropic-transport-only) vs ordinary custom tools shaped like str_replace (portable, slightly less
  native).
- **Provider envelope and dialect are separate axes.** Transport = wire envelope (which
  `*_turn_stream` runs); dialect = tool vocabulary. A Gemini coder on `openai_compat` can still pick
  `Generic`. Both resolve at the boundary; don't conflate them.
- **The toolbox does NOT fix the transport bugs.** Gemini's dropped `thought_signature` (#6) and the
  google-branch flatten (#5, `llm.rs:1166`) are *transport* problems — a Gemini coder still runs
  through `openai_compat`. The dialect split *contains* them ("transport for family X needs work")
  but they're fixed separately. Defer per prior decision.
- **Mixed surface for Anthropic.** Native text_editor/bash for edits+shell, Anvil tools for
  nav/search — make sure the prompt addendum describes the combined set coherently so the model
  isn't told to `apply_patch` in one breath and `str_replace` in the next.
- **Don't over-fork.** Three dialects is the v1 ceiling. Resist a per-model dialect explosion;
  Generic should absorb the long tail, with tuned dialects only where the data earns one.

---

**Status: PARKED — design, no go-ahead to build. Next step if pursued: implement `Dialect::Generic`
+ `Codex` (no exec changes), wire the Agent to carry a dialect, and A/B Generic vs Codex on a coder
via the bench. Add the Anthropic arm (+`insert_lines` + the `ToolDef`/transport touch) once the
native-vs-mimic fork is settled by data.**
