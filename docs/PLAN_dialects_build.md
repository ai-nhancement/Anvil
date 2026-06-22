# Build plan: model-agnostic tool dialects + tool-fit benchmark

The execution plan for shipping the design in `docs/ROADMAP_model_dialects.md` and the harness in
`docs/ROADMAP_tool_dialect_bench.md`. Phased in Anvil's own gated style: each phase is independently
shippable and ends in a concrete verification, so work can stop after any phase and still bank
value. Smallest-first, lowest-risk-first.

Related: `docs/ROADMAP_local_coder_tuning.md` (Phase 4 decides whether it's even needed).

**Status: ready to build, awaiting go-ahead.** Recommended first increment: Phase 0 + 1 together.

---

## Phase 0 — Scaffolding (pure plumbing, zero behavior change)

The riskiest-to-get-subtly-wrong wiring, so prove it changes nothing before adding any dialect.

- Add `src/dialect.rs`: `Dialect` enum + `tool_defs()` / `normalize()` / `prompt_addendum()`, with
  **only `Codex` implemented as an exact pass-through** of today's `tools::tool_defs()`.
- Coder `Agent` carries a `Dialect` (construction seam `src/ui.rs:3036`).
- In the loop (`src/agent.rs`): source tools from `dialect.tool_defs()`, splice
  `dialect.prompt_addendum()` into the coder system prompt, run `dialect.normalize()` on each call
  **before** `tools::execute()` and before the transcript summary
  (`summarize_args`/`result_summary`, `tools.rs:319/352`).

**Verify:** existing tests green; a real phase build under `Codex` behaves byte-identically to
today.

**Touches:** `src/dialect.rs` (new), `src/agent.rs`, `src/ui.rs`. No change to `tools.rs` exec or
`llm.rs` transport.

---

## Phase 1 — Generic dialect + selection (first real value)

The agnostic floor: after this, Anvil works with any function-calling model.

- `Dialect::Generic`: `tool_defs()` = current set **minus `apply_patch`**, with the
  "PREFER apply_patch" framing stripped from `edit_file`'s description; pass-through `normalize()`;
  a short neutral prompt addendum.
- Selection: per-binding `dialect = "..."` override → family inference (Anthropic→Anthropic,
  OpenAI/Codex→Codex) → **`Generic` fallback**.

**Verify:** bind a coder to `generic`, run a scratch edit task end-to-end — `edit_file` / `write_file`
land the change without `apply_patch`.

**Touches:** `src/dialect.rs`, config plumbing (`src/config.rs`), wherever the coder binding is
resolved.

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
  change; `normalize()` stays I/O-free so it can't synthesize `insert`).
- If native path: optional `native_type` marker on `ToolDef` (`llm.rs:84`) honored by
  `anthropic_turn_stream` (`llm.rs:1402/1437`).
- `normalize()` map: `str_replace`→`edit_file`, `view`→`read_file`, `create`→`write_file`,
  `insert`→`insert_lines`, `bash`→`run_command`; keep Anvil `read_file`/`list_dir`/`grep`/
  `project_state`/`flag_risk`/`delegate` as ordinary tools.

**Verify:** a Claude coder edits via `str_replace` end-to-end (sandbox + confirmation gate intact);
bench shows Anthropic ≥ Codex for Claude.

**Touches:** `src/dialect.rs`, `src/tools.rs` (+`insert_lines`), possibly `src/llm.rs` + `ToolDef`.

---

## Phase 4 — The payoff test (local model)

- Run a local open-weight on `Generic` vs `Codex` through the bench.

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
