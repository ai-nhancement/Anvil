# Build plan: model-agnostic tool dialects + tool-fit benchmark

The execution plan for shipping the design in `docs/ROADMAP_model_dialects.md` and the harness in
`docs/ROADMAP_tool_dialect_bench.md`. Phased in Anvil's own gated style: each phase is independently
shippable and ends in a concrete verification, so work can stop after any phase and still bank
value. Smallest-first, lowest-risk-first.

Related: `docs/ROADMAP_local_coder_tuning.md` (Phase 4 decides whether it's even needed).

**Status: ready to build, awaiting go-ahead.** Recommended first increment: Phase 0 + 1 together.

---

## Phase 0 — Scaffolding at the transport boundary (pure plumbing, zero behavior change)

**Translate dialects at the LLM gateway (`llm.rs`), not in the agent loop** (review Recommendation
A). This keeps the confirmation gate, dedup/loop-breaker, history, and the ledger canonical — fixing
review findings #1 (confirmation bypass), #2 (dedup corruption), and #4 (cross-vendor ledger drift)
*by construction* rather than by careful ordering. Prove it changes nothing before adding any
dialect.

- Add `src/dialect.rs`: `Dialect` enum + `advertise()` / `format_call()` / `to_canonical()` /
  `prompt_addendum()`, with **only `Codex` as an exact pass-through** (advertise = today's schema;
  `to_canonical` / `format_call` = identity).
- Resolve the dialect in `src/ui.rs` where the logical binding is known (~`3036`) and thread the
  resolved `Dialect` to the transport — **not** derived inside `Agent::new` from the raw model id
  (finding #3).
- Thread `Dialect` into `src/llm.rs` at the gateway (`chat_turn_stream`): outbound `advertise()`
  (tool set) + `prompt_addendum()` spliced into the system prompt; inbound `to_canonical()` on the
  returned calls. All three dialect mutations live in one place, so the Agent stays
  dialect-agnostic. (`format_call()` — per-dialect history re-rendering — is only needed by the
  Anthropic native arm; deferred to Phase 3. Caveat: the addendum is spliced at the gateway, so the
  agent's `[prompt-log]` shows the base system prompt without it — fine while Codex's is empty;
  Phase 1 should weigh whether the Generic addendum needs to appear in the log.)

**Verify:** existing tests green; a real phase build under `Codex` behaves byte-identically to today,
**and** the ledger (`.anvil/session.json`) contains only canonical tool names.

**Touches:** `src/dialect.rs` (new), `src/llm.rs` (boundary), `src/ui.rs` (resolution + threading).
No change to `agent.rs` gates, `tools.rs` exec, or the ledger schema.

---

## Phase 1 — Generic dialect + selection (first real value)

The agnostic floor: after this, Anvil works with any function-calling model.

- `Dialect::Generic`: `advertise()` = canonical set **minus `apply_patch`**, with the
  "PREFER apply_patch" framing stripped from `edit_file`; identity `to_canonical()` (canonical
  already *is* the generic shape); supply Generic's `prompt_addendum` text — the splice site is
  already wired at the gateway (Phase 0).
- Selection: per-binding `dialect = "..."` override → family inference (Anthropic→Anthropic,
  OpenAI/Codex→Codex) → **`Generic` fallback**.

**Verify:** bind a coder to `generic`, run a scratch edit task end-to-end — `edit_file` / `write_file`
land the change without `apply_patch`, and the confirmation gate + dedup still fire (they see
canonical names).

**Touches:** `src/dialect.rs`, config plumbing (`src/config.rs`), binding resolution in `src/ui.rs`.

---

## Phase 2 — The benchmark harness

Turns `select_dialect()` from a guess into a measurement; also a regression net for later phases.

- `bench/fixtures/<id>/{before/, after/, task.toml}` format (edit-type tag, dialect-neutral
  instruction, success check).
- A runner that copies `before/` to a scratch dir, sweeps `model × dialect` over the real
  `tools::execute()` path, N runs per cell, scored on the tool-fit metrics
  (`ROADMAP_tool_dialect_bench.md` §3). Resumable per cell (rate limits).
- 6–8 fixtures covering the edit types where dialects diverge (single-line, multi-hunk, add/delete/
  rename, insert, tricky-whitespace, large-file targeted).

**Verify:** produces a Codex-vs-Generic heatmap for Claude and GPT (known-good), reported as
success rates ± spread.

**Touches:** `bench/` (new tree), a runner (bin or test harness). Reuses `dialect.rs` + `execute()`.

---

## Phase 3 — Anthropic native dialect

Higher ceiling; let Phase 2 settle the native-vs-mimic fork before building.

- Resolve the fork from data: real built-in tool types (`text_editor_20250728` +
  `str_replace_based_edit_tool`, `bash_20250124`) vs ordinary custom tools shaped like `str_replace`.
- Additive canonical op `insert_lines{path, after_line, text}` in `tools.rs::run()` (the one exec
  change; `to_canonical()` stays I/O-free so it can't synthesize `insert`).
- **`advertise()` for the native path emits `{"type": "text_editor_20250728", "name": "..."}` with
  NO `input_schema`** — the API rejects a custom schema on a native type (finding #6).
- **`to_canonical()` dispatches on `args["command"]`** of the single `str_replace_based_edit_tool`
  (`str_replace`/`view`/`create`/`insert`) — *not* on the tool name (finding #5) — and maps
  `bash`→`run_command`. Anvil `read_file`/`list_dir`/`grep`/`project_state`/`flag_risk`/`delegate`
  stay as ordinary tools.

**Verify:** a Claude coder edits via `str_replace` end-to-end (sandbox + confirmation gate intact);
bench shows Anthropic ≥ Codex for Claude.

**Touches:** `src/dialect.rs`, `src/tools.rs` (+`insert_lines`), possibly `src/llm.rs` + `ToolDef`.

---

## Phase 4 — The payoff test (local model)

- Run a local open-weight on `Generic` vs `Codex` through the bench.
- **Give each model a fair prompt baseline (finding #7):** keep the *task* dialect-neutral, but use
  the model-appropriate system scaffold (its dialect's `prompt_addendum`). A model that fails on
  *both* dialects is a "can't tool-use here" result to log — not evidence that dialect doesn't help.

**Verify / decide:** does Generic move a local model from "hopeless coder" to viable? This gate
decides whether `ROADMAP_local_coder_tuning.md` is needed at all, or whether the dialect alone was
the fix.

**Touches:** none (config + bench run only).

---

## Dependency order

```
Phase 0 ──▶ Phase 1 ──▶ Phase 2 ──▶ Phase 3
                            └──────▶ Phase 4
```

Phases 0–1 are the recommended first increment (Phase 0's "changes nothing" checkpoint de-risks all
that follow). Phase 2 unblocks the data-driven decisions in 3 and 4.

---

## Working agreements

- **Branch for code** (unlike the parked docs, which went straight to master). One branch for the
  dialect work; commit per phase at its verification checkpoint.
- **Run `cargo fmt` before every commit** — CI enforces fmt, release.yml doesn't, so a miss = green
  binaries but red master CI ([[run-fmt-before-release]]).
- **`cargo clean -p anvil` if a rebuild looks stale** ([[cargo-clean-required]]).
- Stop at each phase's verify gate for confirmation before starting the next.
