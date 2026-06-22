# Roadmap: benchmark which tool dialect each model performs best with

A study note for an empirical harness that measures **tool-use fidelity per model × tool
dialect** — so Anvil can hand each model the editing surface it actually works best with,
instead of one Codex-derived dialect for everyone. The data also de-risks the local-coder
direction and serves as the eval the tuning roadmap already requires.

**PARKED — ideation, no go-ahead to build.** Decision context: [[project-anvil-future-directions]]
(model-agnostic coder), [[user-model-setup-preference]] (no fixed model in any role; "make Anvil
model-agnostic-friendly"), [[reference-vertex-setup]] / [[reference-nvidia-nim]] (multiple
provider bindings already wired, cheap to sweep). Sibling studies: `docs/ROADMAP_local_coder_tuning.md`
(this benchmark *is* its §5 eval), and the model-dialect "toolbox" design (captured in conversation;
companion doc `docs/ROADMAP_model_dialects.md` TODO).

---

## 0. The motivating observation

Claude and GPT work best as the Anvil coder today — not by luck. `apply_patch` **is**
GPT/Codex's native edit format (Anvil borrowed it; `tools.rs:516` says "Codex-style"), and
Claude's native `str_replace` is one rename away from Anvil's `edit_file`. The two families that
work are the two whose native edit surface is closest to the single dialect Anvil hands out
(`tools::tool_defs()`). Models that struggle — Grok-build, Gemini 3.x, local open-weights — are
the ones furthest from it.

The principle: **don't fight the model, work with it.** Hand each model the tools it was trained
to use. But "which tools fit which model" is currently a hypothesis. Anvil benchmarks models for
everything else — it should benchmark this too, and let the data pick the dialect.

---

## 1. The one methodological rule: isolate tool-fit from intelligence

The trap: scoring "did the phase pass review" conflates *the model is smart* with *the dialect
fits the model*. GPT would win every cell because GPT is GPT — telling us nothing about dialects.

The benchmark must measure something narrower: **given an edit whose correct result is already
known, which surface does this model land cleanly, first try, with no flailing?** That means
deterministic, ground-truth edit tasks — not open-ended coding. The signal is always
**within-model** (model A on dialect X vs Y vs Z), never cross-model leaderboards.

---

## 2. The matrix

Hold each task fixed; vary `model × dialect`; score execution fidelity.

```
for task in corpus:            # known input file(s) + known expected output
  for model in bindings:       # whatever providers are configured
    for dialect in {apply_patch, str_replace, generic}:
      run N isolated agent turns (stochastic → repeat)
      capture tool call(s) → execute against a temp copy → diff vs expected
```

Each cell yields a **success rate**, not a pass/fail. The selector (`select_dialect(model)` in the
companion dialect design) then picks the argmax dialect per model from real data.

### Dialects under test (v1)

- **apply_patch** — today's Codex envelope (`*** Begin Patch`; `tools.rs:690`).
- **str_replace** — Anthropic-native: `str_replace`(old_str/new_str, unique), `view`(range),
  `create`, `insert`. (Native built-in tool types: `text_editor_20250728` +
  `str_replace_based_edit_tool`, `bash_20250124`.)
- **generic** — plain typed function calls: `edit_file(old_string, new_string)`, `write_file`,
  `read_file`, `grep`. The lowest-common-denominator surface every function-calling model handles;
  the agnostic floor and the most likely win for local models.

All three normalize down to Anvil's existing canonical `execute()` — the benchmark reuses the real
execution path, so a dialect that scores well is immediately shippable.

---

## 3. What to score (per model × dialect cell)

None of these is "is the code good." All are tool-fit:

| Metric | What it catches |
|---|---|
| **First-call validity** | well-formed call, valid args, no schema violation |
| **Edit landed** | the patch/str_replace actually located its target and applied |
| **Retries to success** | flailing = clearest wrong-fit signal |
| **Turns / tokens to done** | efficiency of the surface for that model |
| **Result correctness** | final file byte- (or AST-) matches the known-good output |

Report mean ± spread over N runs. A dialect that wins 11/12 task types but reliably botches one
(e.g. renames) is a finding to log, not noise to average away.

---

## 4. Task corpus — exercise exactly where dialects diverge

Small, deterministic, verifiable. Each fixture = `{input file(s), expected output, edit-type tag}`.
The flavors that separate dialects:

- single-line change
- multi-hunk change in one file
- new file (add)
- delete file
- rename file
- insert into the middle of a file
- **tricky-whitespace edit** — apply_patch's classic failure mode (Anvil's hunk matcher is exact,
  `tools.rs:655`)
- **targeted edit in a large file** — where `read_file` offset/limit + locate-by-context matters
  (`tools.rs:448`)

A dozen tasks × N models × 3 dialects is cheap to run and produces a real heatmap:
*this model, this edit type, this surface → success rate.*

### Suggested fixture format

```
bench/fixtures/<id>/
  task.toml        # edit-type tag, instruction text, success check (exact|ast|regex)
  before/          # input tree (copied to a temp dir per run)
  after/           # expected tree (the ground truth to diff against)
```

The runner copies `before/` to a scratch dir, gives the model the instruction under one dialect,
executes its tool calls against the scratch dir, then diffs against `after/`.

---

## 5. Why this is the keystone, not a side quest

1. **It's the eval `ROADMAP_local_coder_tuning.md` §5 already requires** — "a held-out task set
   scored on tool-use success (valid tool, valid args, phase completes), not vibes." Build it once;
   it serves dialect selection *and* the LoRA eval.
2. **It's the cheap pre-step before any tuning.** The bench tells you whether a local model on the
   *generic* dialect already closes most of the gap — maybe you don't tune at all, you just stop
   handing it Codex's patch DSL.
3. **It tests the local-model hunch directly.** Local open-weights are weak at *producing an exotic
   format from instructions alone* (the apply_patch envelope is a big ask for a 7–32B model) but
   often fine at a plain `edit_file(old, new)` call. If true, a model that looked hopeless as a
   coder was just fighting the wrong dialect — the highest-probability win in the project, provable
   for the price of an afternoon's runs. Lands exactly where the user is headed (local/self-host,
   $0 marginal, no rate limits).

---

## 6. Hard parts / honest caveats

- **Stochasticity.** Repeat each cell (N≥5) and report rates; a single run is noise.
- **Instruction phrasing leaks.** The instruction text must be dialect-neutral — don't say "apply a
  patch" in the prompt for one arm and "replace the string" in another, or you're benchmarking the
  prompt, not the surface. Keep the task description identical; only the advertised tools change.
- **Fair prompt baseline per model (local-model false negatives).** Local open-weights are sensitive
  to prompt styling; one may fail on *every* dialect for prompt-format reasons, not dialect ones.
  Give each model its appropriate scaffold (the dialect's `prompt_addendum`) so the comparison
  isolates the tool surface. A model that fails across all dialects is a "can't tool-use here"
  finding to log — not evidence that dialect choice doesn't matter.
- **Rate limits / cost.** Free-tier and local providers throttle; sweeps need backoff and the runs
  should be resumable per cell so a 429 doesn't trash the matrix (relates to v0.5.5 bug #4).
- **Native-tool-type plumbing.** The str_replace arm needs the Anthropic built-in tool types
  emitted in `anthropic_turn_stream` (`llm.rs:1402`), not as a normal `input_schema` tool — a real
  but contained change. The generic and apply_patch arms reuse the existing `openai_turn_stream`
  path.
- **Don't overfit the corpus.** A dozen synthetic edits won't cover every real failure; treat the
  bench as a selector signal, cross-check against the gate ledger (real trajectories), and log what
  the corpus doesn't exercise.
- **Scope of v1.** Edit-format fidelity only. Multi-tool agentic sequencing (read→edit→verify
  rhythm, loop discipline) is a richer, later benchmark; start with the single highest-leverage
  axis.

---

**Status: PARKED — ideation. Next step if pursued: a 6–8 fixture corpus + a runner that sweeps
`model × dialect` over the existing `execute()` path, scored on the §3 metrics. Validate against
Claude + GPT first (known-good), then a local model on the generic dialect to test the §5 hunch.**
