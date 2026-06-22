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

## 2. The `Dialect` abstraction (`src/dialect.rs`)

A dialect owns three things; execution owns none of them.

```rust
pub enum Dialect { Codex, Anthropic, Generic }   // open to extension

impl Dialect {
    /// Tools advertised to the model, in this family's idiom.
    fn tool_defs(&self) -> Vec<ToolDef>;

    /// Map a model-emitted call into a CANONICAL call that tools::run() already understands.
    /// Pure name/arg rewriting — no I/O.
    fn normalize(&self, call: ToolCall) -> ToolCall;

    /// Family-specific addendum spliced into the coder system prompt (loop discipline is
    /// family-tuned too — e.g. the Anthropic dialect must NOT say "prefer apply_patch").
    fn prompt_addendum(&self) -> &str;
}
```

The dialect is a thin adapter *over* the canonical core — never a fork of it. Adding support for a
new model is writing one small adapter, not touching execution, sandboxing, or the gates. That
adapter-over-invariant shape is what makes "any model" real instead of N parallel codepaths.

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

`normalize()` maps the native commands back to canonical:

| Native (Claude) | Canonical (Anvil `execute()`) |
|---|---|
| `str_replace{path, old_str, new_str}` | `edit_file{path, old_string, new_string}` |
| `view{path, view_range}` | `read_file{path, offset, limit}` |
| `create{path, file_text}` | `write_file{path, content}` |
| `insert{path, insert_line, insert_text}` | `insert_lines{...}` — **the one additive exec op needed** (see §6) |
| `bash{command}` | `run_command{command}` (keeps the confirmation gate, `tools.rs:188`) |

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

## 5. Wiring — what changes, what doesn't

The Agent owns a `Dialect` (set at coder construction, `src/ui.rs:3036`). In the agent loop
(`src/agent.rs`):

1. Build the turn's tools from `dialect.tool_defs()` instead of `tools::tool_defs()`.
2. Splice `dialect.prompt_addendum()` into the coder system prompt.
3. On each returned `ToolCall`, run `dialect.normalize()` **before** `tools::execute()` and before
   the transcript summary (`summarize_args`/`result_summary` are name-keyed on canonical names,
   `tools.rs:319/352` — normalize first or display degrades).

**Unchanged:**
- `tools::execute()` / `run()` — the canonical core (one additive op, §6).
- `llm.rs` transport interface — it still serializes whatever `ToolDef`s it's handed into each
  provider's envelope (`openai_turn_stream` `parameters`, `llm.rs:1234`; `anthropic_turn_stream`
  `input_schema`, `llm.rs:1437`). The Anthropic built-in-type arm is the one transport touch (§8).
- Reviewers / specialists — keep the canonical read-only subset (`read_only_tool_defs`,
  `tools.rs:179`); they're not fighting harness training the way the coder is. A reviewer dialect is
  a possible later refinement, not v1.
- Gates, plan, audit, sandbox — entirely above/below the dialect line.

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
- **`ToolDef` needs an optional native marker for the Anthropic built-in arm.** Today `ToolDef` is
  `{name, description, input_schema}` (`llm.rs:84`). Built-in types are schema-less and serialize as
  `{"type": "text_editor_20250728", "name": "..."}`. Either add an optional `native_type` field that
  `anthropic_turn_stream` honors, or take the mimic path (custom tools shaped like str_replace) and
  avoid the transport change entirely. Decide via the bench.
- **The toolbox does NOT fix the transport bugs.** Gemini's dropped `thought_signature` (#6) and the
  google-branch flatten (#5, `llm.rs:1166`) are *transport* problems — a Gemini coder still runs
  through `openai_compat`. The dialect split *contains* them ("transport for family X needs work")
  but they're fixed separately. Defer per prior decision.
- **Mixed surface for Anthropic.** Native text_editor/bash for edits+shell, Anvil tools for
  nav/search — make sure the prompt addendum describes the combined set coherently so the model
  isn't told to `apply_patch` in one breath and `str_replace` in the next.
- **Display correctness.** Normalize before `summarize_args`/`result_summary` or the transcript
  shows raw native names the helpers don't recognize.
- **Don't over-fork.** Three dialects is the v1 ceiling. Resist a per-model dialect explosion;
  Generic should absorb the long tail, with tuned dialects only where the data earns one.

---

**Status: PARKED — design, no go-ahead to build. Next step if pursued: implement `Dialect::Generic`
+ `Codex` (no exec changes), wire the Agent to carry a dialect, and A/B Generic vs Codex on a coder
via the bench. Add the Anthropic arm (+`insert_lines` + the `ToolDef`/transport touch) once the
native-vs-mimic fork is settled by data.**
