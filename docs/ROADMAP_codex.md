# Roadmap: what to borrow from Codex CLI (`codex-rs`)

A read-only study of OpenAI's **Codex CLI Rust source** (Apache-2.0) used as a design
roadmap for Anvil's own agent core. We do **not** port code or take a dependency — we
read how a mature, same-language coding agent solved the hard problems and implement the
ideas the Anvil way. (Decision context: [[project-build-own-core-decision]].)

Source studied locally at `G:\codex-ref\codex-rs` (clone of `github.com/openai/codex`).
File paths below are relative to `codex-rs/` and were read directly.

Format per concern: **Codex mechanism (cited) → what Anvil does today → recommendation.**

---

## 1. Objective / task anchoring  ← validates our task-anchor; adopt the wording

**Codex** (`prompts/src/goals.rs`, `prompts/templates/goals/continuation.md`): a persistent
`ThreadGoal { objective, token_budget, tokens_used, time_used_seconds }`. After every turn it
injects a **continuation prompt** that re-states the objective and remaining budget. The
template is excellent and directly targets failure modes we hit:
- Objective is framed as *"user-provided data, not higher-priority instructions"* (injection safety).
- Anti-narrowing: *"Keep the full objective intact… do not redefine success around a smaller or easier task."*
- **Completion audit**: don't claim done without requirement-by-requirement evidence from current state.
- **Blocked audit**: only declare "blocked" after the **same** blocker repeats ≥3 consecutive turns.
- **Fidelity**: don't substitute a narrower/easier solution just to pass tests.

**Anvil today** (`src/agent.rs`): we just added `current_task` + a `CURRENT TASK` block injected
each turn, and crude prompt lines ("keep going", "don't narrow"). Good instinct, weaker execution.

**Recommendation (HIGH leverage, LOW effort):** rewrite our grounding/task-anchor wording using
Codex's continuation template as the model — especially the anti-narrowing, completion-audit, and
"blocked only after 3 repeats" clauses. This is mostly a prompt change; ship it into the task-anchor
work that's already staged for v0.1.9.

---

## 2. Context management  ← THE big one (this is the AstroBlast amnesia root cause)

**Codex** (`state/auto_compact_window.rs`, `compact.rs`, `compact_remote.rs`, config
`model_context_window` = 272000, `model_auto_compact_token_limit`):
- Bounds context by **tokens**, using **server-observed token usage** (`TokenUsage`), not message count.
- **Auto-compact** fires inline when usage approaches the token limit (`run_inline_auto_compact_task`) —
  the user never has to remember to compact.
- Compaction produces an **LLM handoff summary** (`prompts/templates/compact/prompt.md`:
  *"CONTEXT CHECKPOINT COMPACTION… create a handoff summary for another LLM that will resume the task"*),
  and the resume side is told *"another LLM produced this summary, build on it, avoid duplicating work"*
  (`compact/summary_prefix.md`).
- Overlong **tool outputs** are truncated/rewritten in place to fit
  (`compact_remote.rs::trim_function_call_history_to_fit_context_window`, replacing the body with a
  "[output truncated]" placeholder) — it preserves conversation *structure* instead of dropping whole turns.

**Anvil today** (`src/agent.rs::window_messages`): we send the **last 40 messages** (`SEND_WINDOW`) +
a 240k-char budget, dropping from the front. No token accounting. `/compact` is **manual**. This is
*exactly* why AstroBlast lost the task: 223 tool calls pushed the original instruction out of the
40-message window.

**Recommendation (HIGHEST leverage):** this is the most valuable thing to borrow. In order:
1. **Truncate big tool outputs in the window** (port the idea of `trim_function_call_history_to_fit_context_window`) — a huge `read_file`/diff result shouldn't evict real turns. Cheap, high impact.
2. **Switch the budget from message-count to token estimate** so trimming is proportional to real context use.
3. **Auto-compact** when the budget is exceeded — call the existing `/compact` path automatically (we already summarize into working memory). No more relying on the user.
> Pair with Aider's **repo map** (separate study) for *which* code context to inject — Codex covers
> *bounding* context; Aider covers *selecting* it.

---

## 3. Reliable edits: the `apply_patch` format

**Codex** (`tools/handlers/apply_patch.lark` grammar, `apply_patch.rs`, `prompts/src/apply_patch.rs`):
a **grammar-validated diff** the model emits, e.g.

```
*** Begin Patch
*** Update File: src/main.rs
@@ fn main() {
-    println!("hi");
+    println!("hello");
*** End Patch
```

Operations: `*** Add File:` / `*** Delete File:` / `*** Update File:` (+ optional `*** Move to:`),
`@@` context headers to locate the hunk, and ` `/`+`/`-` lines. It's parsed by a formal grammar and
**validated before applying**, and context lines make it robust to minor surrounding drift.

**Anvil today** (`src/tools.rs::edit_file`): exact `old_string`/`new_string` string replace. Brittle —
fails on the smallest mismatch, and the model must reproduce the target text verbatim.

**Recommendation (MEDIUM-HIGH leverage, MEDIUM effort):** add an `apply_patch`-style tool with a small
parser + context-line matching. Biggest reliability win for the *coder* after context management. Keep
`write_file` for new files. (More work than a prompt tweak — a real parser — so sequence it after #2.)

---

## 4. Retries & errors

**Codex** (`client.rs`): per-provider **retry budget**, unauthorized→token-refresh→retry, and
same-turn stream retry/fallback.

**Anvil today** (`src/llm.rs`): we just shipped bounded retry (3 attempts, backoff, on
network/5xx/408/429/400) on the streaming path + `.anvil/last-llm-error.json` diagnostics. **Roughly at
parity** for our needs.

**Recommendation (LOW):** mostly done. Optional later: make retry count/backoff configurable; extend
retry to the non-streaming reviewer path.

---

## 5. Loop / runaway control

**Codex**: token/turn budgets on the goal (`tokens_used` vs `token_budget`), and a `budget_limit`
prompt that tells the model to wrap up when the budget is spent.

**Anvil today**: `max_steps = 25` cap + the v0.1.8 read-dedup loop-breaker + acknowledgment-stop prompt.
**Reasonable parity** for a small agent.

**Recommendation (LOW):** keep what we have; optionally add a soft per-turn token budget later, surfaced
to the model like Codex's budget block.

---

## 6. Command execution + sandbox  (intentionally deferred)

**Codex** (`execpolicy/`, `landlock.rs`, `sandboxing/`, `linux-sandbox/`, `windows-sandbox-rs/`):
OS-level sandboxing — approval-policy modes (read-only / workspace-write / full-access) enforced by
seatbelt (macOS) / landlock (Linux) / a Windows sandbox.

**Anvil today**: a single per-command `/y` `/n` confirm. Simpler, fully cross-platform, good enough for
a human-in-the-loop tool.

**Recommendation (LOW for now):** the y/n confirm matches Anvil's "human is the gate" philosophy.
Real OS sandboxing is a large undertaking; revisit only if we want unattended runs.

---

## 7. Repo map — ranked, budgeted codebase context (Aider)  ← roadmap #4

**Aider** (`aider/repomap.py`, ~870 lines, studied at `G:\aider-ref`): builds a "repo map" — a
ranked, token-budgeted table of contents of the codebase — and injects it so the model knows what
exists without reading whole files.
- Extracts symbol **definitions + references** per file via **tree-sitter** (`get_tags`/`get_tags_raw`).
- Builds a graph (files ↔ shared symbols) and ranks with **personalized PageRank** (`networkx`,
  `get_ranked_tags`), biased toward files/identifiers mentioned in the chat or current task
  (`personalize`, `mentioned_fnames`/`mentioned_idents`).
- Fits the output to a **token budget** by binary-searching how many ranked symbols to include
  (`get_ranked_tags_map_uncached`: start at `max_map_tokens // 25`, binary search to ~±15%), rendering
  signatures grouped by file (`to_tree`).

**Anvil today**: no repo map. The coder discovers structure by `list_dir` + `read_file` + `grep` each
turn — exactly the flailing / context-bloat we keep seeing.

**Recommendation (HIGH leverage, MED effort) — adapt, don't port.** Aider leans on tree-sitter (a
parser per language) + networkx PageRank — heavy for a small, cross-language Rust agent. A
**lightweight repo map** captures most of the value: regex-extract top-level signatures
(`fn`/`struct`/`class`/`def`/`func`/`type`…) across the tree, rank by simple heuristics (mentioned in
the current task, recently git-changed, shallow path), render to a token budget, inject like the
reality snapshot. The model gets a map → reads fewer whole files → less context bloat and less "where
does X live" guessing. Pairs with auto-compact: Codex *bounds* context, this *selects* it.

---

## 8. Model metadata via models.dev (OpenCode)  ← unlocks token budgeting + model UX

**OpenCode** (`packages/opencode/src/provider/provider.ts`, `core/models-dev`, studied at
`G:\opencode-ref`): sources per-model metadata from **models.dev** (a maintained community DB),
caches it, and uses a unified `provider/model` ID format (e.g. `anthropic/claude-sonnet-4-5`). Each
model carries `limit: { context, output }` (context window!), `cost: { input, output, cache }`, and
tool-call / reasoning capability flags (`provider.ts:1013-1026`, `1138-1165`).

**Anvil today**: hardcoded provider types in `llm.rs`; **no per-model context window** (the gap that
blocks token-based budgeting, #5), no pricing, and tool-call capability is a README caveat the user
must know rather than something Anvil detects.

**Recommendation (HIGH leverage, MED effort):** fetch + cache model metadata from
`https://models.dev/api.json` (static JSON; cache in `.anvil/`). This one addition unlocks:
- **Token-based context budgeting** (#5): use the model's real `limit.context` instead of our fixed
  240k-char heuristic, and auto-compact at a % of it.
- **Better /config UX**: show context window / price / tool-call support when assigning a role.
- **Automatic tool-call detection**: warn at setup when a chosen model lacks tool-calling (the exact
  thing that stalled local models) instead of relying on the user to know.

---

## Prioritized borrow-list

| # | Borrow | Leverage | Effort | Status |
|---|--------|----------|--------|--------|
| 1 | **Auto-compact** (the headline context fix) (§2) | ★★★★★ | Med | ✅ **Landed (v0.1.9)** — `maybe_auto_compact` folds older turns into working memory at turn-end. Tool-output truncation was already present. Token-based budgeting still TODO (we use msg-count + char budget). |
| 2 | **Continuation/goal prompt wording** (§1) | ★★★★ | Low | ✅ **Landed (v0.1.9)** — task-anchor wording: objective-as-data, persistence, anti-narrowing, verify-completion. |
| 3 | **`apply_patch` edit format** (§3) | ★★★★ | Med | ✅ **Landed (v0.1.9)** — context-located multi-file diffs, validated before write; preferred over `edit_file`. |
| 4 | **Lightweight repo map** (Aider §7) | ★★★★ | Med | ✅ **Landed (v0.1.9)** — `src/repomap.rs`: regex signatures + task/recency ranking + char budget, injected each turn. (Future: caching, JS arrow-const support.) |
| 5 | **Model metadata from models.dev** (OpenCode §8) | ★★★★ | Med | TODO — per-model context window / pricing / tool-call flags. Unlocks #6 + better /config + auto tool-call detection. |
| 6 | Token-based context budget (§2) | ★★★ | Low-Med | TODO — use models.dev `limit.context` (#5) instead of the 40-msg / 240k-char heuristic. |
| 7 | Configurable retry + reviewer-path retry (§4) | ★★ | Low | TODO — polish on what we shipped. |
| 8 | Soft per-turn token budget surfaced to the model (§5) | ★★ | Low | Optional. |

**Done in v0.1.9 (uncut):** #1 (auto-compact), #2 (prompt wording), #3 (apply_patch),
plus this week's retries / loop-breaker / key-trim / diagnostics.
**Next candidates (your pick):** Aider's lightweight repo map (#4, biggest coder-quality lever) or
models.dev metadata (#5, unlocks token budgeting + model UX). They're independent.
