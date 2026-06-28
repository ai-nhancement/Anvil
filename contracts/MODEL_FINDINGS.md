# Local-model findings — what works, and in which role

What each local model we tried can actually do in Anvil, **measured on the bench**, not
guessed. For each model: the contract that works best, how it scores as a **coder** and as a
**reviewer**, what it *can't* do, and the **role** we'd actually put it in. Bringing a new
model? The recipe at the bottom tells you how to slot it in.

> Numbers from `anvil bench` (coder: 10 fixtures incl. multi-step ones, n=10 → /100, scored by a
> real check passing) and `anvil review-bench` (6 planted-bug cases → catch /30, + 1 decoy →
> clean /5, judge = claude-sonnet-4-6). Run on 2026-06-25. Reviewer numbers in the table are under
> the *built-in* reviewer prompt; "Reviewer contracts" below shows the gains from a real reviewer
> contract. Validate any judge first with `anvil judge-check` (see "Choosing a judge").

## Summary

| model | size / family | coder contract | coder | reviewer (catch / clean) | best role |
|---|---|---|---|---|---|
| gemma4:e2b | 2B / gemma | **full** (`coder_local_base.md`) | 83/100 | 28/30 · 3/5 | light coder (simple edits) |
| gemma4:e4b | 4B / gemma | **minimal** (`coder_local_base_v4.md`) | **94/100** | 30/30 · 1/5 → **4/5**ᴾ | **coder** (+ reviewer ᴾ) |
| qwen2.5-coder:7b | 7B / qwen2.5 | — (cannot drive tools) | **0/100** | 14/30 · **5/5** | **reviewer only** (high-precision) |
| qwen3-coder:30b | 30B / qwen3 | **minimal** (`coder_local_base_v4.md`) | 93/100 | **30/30 · 5/5** | **coder + reviewer** (all-rounder) |
| gemma4:31b | 31B / gemma | **minimal** (`coder_local_base_v4.md`) | **Passed screening** | 30/30 · 0/5 → **5/5**ᴾ | **coder + reviewer** (strong all-rounder ᴾ) |
| qwen3.6:35b | 35B / qwen | **minimal** (`coder_local_base_v4.md`) | **Passed screening** (ultra-efficient) | **30/30 · 5/5** | **coder + reviewer** (top-tier all-rounder) |
| llama4:latest | 70B / llama | — (cannot drive tools) | **0/100** | — | **Not compatible** (emits tool calls as text JSON blocks) |
| codestral | 22B / mistral | — (cannot drive tools) | **0/100** | 20/30 · 0/5 | **reviewer only** (moderate recall, noisy precision) |
| command-r | 35B / cohere | **minimal** (`coder_local_base_v4.md`) | 0/100 (failed simple edit) | 15/30 · 0/5 | **not recommended** (struggles with tool execution/precision) |
| phi4 | 14B / reasoning | — (cannot drive tools) | **0/100** | **30/30** · 0/5 | **reviewer only** (flawless recall, noisy precision) |

*catch = planted bugs flagged (recall); clean = decoys left alone (precision / no false alarms).*
*ᴾ = under the precision reviewer contract (`reviewer_local_precision.md`); built-in-prompt clean is 1/5. See "Reviewer contracts".*

## What discriminates (and what doesn't)

Simple single-edit fixtures saturate — every model that can tool-call scores ~10/10, so they
don't rank coders. The **multi-step** fixtures do the separating:

| fixture | e2b | e4b | qwen3-30b | what it tests |
|---|---|---|---|---|
| multi-file-feature | **1** | 6 | **9** | edit 2 files + read a 3rd |
| fix-failing-test | **2** | 10 | 8 | read→fix→verify→iterate |
| large-file-targeted | 10 | 10 | **7** | precise edit in a big file |

That `multi-file-feature` column *is* the coder capability gradient. The old single-edit bench
would have called e2b and qwen3-30b both "~100%"; the faithful bench shows 1 vs 9.

## Per-model

### gemma4:e2b — 2B, gemma → light coder
- **Coder 83/100 (full contract).** Near-perfect on single-edit tasks, but the multi-step ones
  expose its ceiling: `multi-file-feature` **1/10**, `fix-failing-test` **2/10** — and that last
  one is *unstable* (a prior run hit 10/10; this run 2/10), so treat e2b's multi-step coding as
  high-variance, not dependable. Needs the FULL contract (without ACT it emits tool calls as text;
  without VERIFY it ships unverified).
- **Reviewer 28/30 catch, 3/5 clean.** Good recall, but false-positives the clean decoy ~2/5 — a
  bit noisy.
- **Role: light coder** for simple, single-file edits; usable as a *backup* reviewer if you can
  tolerate some false alarms. Don't hand it multi-file work.

### gemma4:e4b — 4B, gemma → coder
- **Coder 94/100 (minimal v4) — best coder tested.** Handles the multi-step work far better than
  e2b (`multi-file-feature` 6/10, `fix-failing-test` 10/10). Adding clauses *hurts* it.
- **Reviewer: 30/30 catch, but 1/5 clean under the *built-in* prompt** — it "finds" bugs in correct
  code 4 of 5 times. The fix is a contract, not a different model: under `reviewer_local_precision.md`
  (minimal base + a NO-FALSE-ALARM clause) clean jumps to **4/5** with catch unchanged at 30/30.
- **Role: coder — and a viable reviewer *with the precision contract*.** On the built-in prompt,
  keep it off the gate; with the precision contract it's usable. (See "Reviewer contracts".)

### qwen2.5-coder:7b — 7B, qwen2.5 → reviewer only
- **Coder 0/100 — cannot drive Anvil's loop.** It emits tool calls as fenced-JSON *text* instead
  of native `tool_calls`, so the agent loop sees no action. A popular local coder that, as shipped
  in Ollama, will not code in Anvil without a tool-dialect that parses text tool-calls.
- **Reviewer 14/30 catch, 5/5 clean.** Misses subtler bugs (`wrong-operator` 0/5,
  `missing-empty-check` 0/5, `transposed-rates` 1/5) but **never false-alarms**, and does catch
  semantic gotchas (`mutable-default` 5/5). A cautious, **high-precision / moderate-recall**
  reviewer.
- **Role: reviewer only.** Good as a precision second opinion (won't waste your time), but pair it
  with a higher-recall reviewer to cover the subtle bugs it misses.

### qwen3-coder:30b — 30B, qwen3 → coder + reviewer
- **Coder 93/100 (minimal v4).** Strong across the board; only soft spot is `large-file-targeted`
  7/10 (better than its prior 3/10). Agentic-tuned, so minimal contract wins.
- **Reviewer a perfect 30/30 catch, 5/5 clean.** Catches every planted bug AND never false-alarms.
- **Role: the all-rounder.** Excellent coder and the best reviewer tested — the natural pick when
  you want one capable local model, or a strong cross-family R1/R2 reviewer.

### gemma4:31b — 31B, gemma → coder + reviewer (high-precision)
- **Coder (minimal v4).** Passed robust multi-step screenings including `single-line-change` and `fix-failing-test` on the first try. Highly capable agentic coder under minimal scaffolding.
- **Reviewer 30/30 catch, 0/5 clean under built-in prompt → 5/5 clean under `reviewer_local_precision.md`.** Like its smaller 4B sibling, it over-flags on correct code under the built-in prompt. However, under the precision contract, it achieves a perfect 30/30 catch and 5/5 clean-rate.
- **Role: coder + reviewer (all-rounder).** Highly capable. Highly recommended to run under the precision contract if used as a reviewer.

### qwen3.6:35b — 35B, qwen → top-tier coder + reviewer
- **Coder (minimal v4) — fastest / most efficient local coder.** Passed robust screenings on multi-step fixtures like `fix-failing-test` and `multi-file-feature` with extreme efficiency (solving complex debugging loops in as few as 2-3 average steps). 
- **Reviewer a perfect 30/30 catch, 5/5 clean.** Catches every defect flawlessly and never false-alarms under the built-in prompt.
- **Role: top-tier all-rounder.** The strongest, most efficient local model tested to date for both coder and reviewer roles.

### llama4:latest — 70B, llama → not compatible
- **Coder 0/100 — cannot drive tools.** Incompatible under the current Ollama driver because it emits tool calls as text/JSON blocks in message content instead of standard native `tool_calls`. Even under the `full` contract, it fails to drive the agent loop.
- **Role: Not compatible.** Avoid in both coder and reviewer roles until tool-calling formatting/driver support is updated.

### codestral — 22B, mistral → reviewer only
- **Coder 0/100 — cannot drive tools.** Ollama reports that `codestral:latest` does not natively support tool-calling via Ollama's API layer (returns 400 Bad Request: "registry.ollama.ai/library/codestral:latest does not support tools"). Cannot drive the coder loop.
- **Reviewer 20/30 catch, 0/5 clean.** Under both the built-in prompt and precision contract, it scored a moderate 4/6 catch rate and continued to raise false alarms on the clean decoy code (0/1 clean-rate).
- **Role: reviewer only (moderate).** Moderate recall, but noisy. Use as a secondary or backup reviewer.

### command-r — 35B, cohere → not recommended
- **Coder 0/100 — struggles with exact edits.** Ollama supports tool calling with `command-r:latest`, but it failed the simple single-line-change fixture due to struggling to accurately reproduce exact strings inside `edit_file` (failed exact match).
- **Reviewer 15/30 catch, 0/5 clean.** Scored 3-4 of 6 catch-rate with 0/1 clean-rate, struggling to detect critical defects or respect style-precision boundaries.
- **Role: not recommended.** Struggles to maintain the level of tool-precision and logical discrimination required for Anvil workflows.

### phi4 — 14B, reasoning → reviewer only (flawless recall, noisy precision)
- **Coder 0/100 — cannot drive tools.** Like Codestral, Ollama reports that `phi4:latest` does not support native tool-calling via Ollama's API layer.
- **Reviewer flawless 30/30 catch, 0/5 clean.** Across both the built-in and precision contracts, Phi4 scored a perfect **6/6 (100% catch-rate)** on all planted defects, demonstrating incredibly strong analytical reasoning. However, it remains highly opinionated and raised style/nit alarms on clean decoy code (0/1 clean-rate) under both setups.
- **Role: reviewer only (high-recall/noisy).** An exceptional detector of bugs and logical defects. Best paired with a high-precision reviewer (like Qwen) that can act as a stabilizing second opinion.

## How this maps to Anvil's two-gate review

Anvil wants a strong **coder** plus a **different-family reviewer**. The data says: code with
**e4b** or **qwen3-coder:30b**; review with **qwen3-coder:30b** (best), **qwen2.5-coder:7b** (a
free precision reviewer that can't code but reads well), or **e4b under the precision reviewer
contract**. The headline: coding skill ≠ reviewing skill, *and* a model's reviewer behavior is
itself contract-shaped — e4b goes from "too noisy to gate" to "usable" on one clause. Pick per
role, and give each role its contract.

## Reviewer contracts

The reviewer is a role, so it gets a contract too — same method as the coder. Two tiers so far in
`contracts/`:
- `reviewer_local_base.md` — minimal: role + output, nothing else.
- `reviewer_local_precision.md` — base + a **NO-FALSE-ALARM clause** ("report a defect only when you
  can name the line and say why; if the diff is clean, pass it").

A/B on e4b (n=5, claude judge) — catch held at 30/30 throughout; the lever is precision:

| reviewer prompt | catch | clean |
|---|---|---|
| built-in generic | 30/30 | 1/5 |
| base contract | 30/30 | 2/5 |
| **+ NO-FALSE-ALARM clause** | 30/30 | **4/5** |

Structure alone helps a little (1→2); the one clause is the real lever (2→4), at zero cost to
recall — the same lesson as the coder side. Bench reviewer contracts with
`anvil review-bench --model X --judge Y --contract <file>`.

**In the live gate**, a reviewer binding can set `contract` too (same field as the coder). The
`"reviewer"` alias = `reviewer_local_live.md`, which keeps Anvil's investigate-with-read-only-tools
framing AND adds the no-false-alarm clause — so a reviewer that over-flags (e.g. e4b) can be calmed
without losing the verify-against-files behavior.

(Caveats: clean-rate rests on a single decoy case — directional, not precise; more decoys would
firm it up. And the no-false-alarm *clause* is bench-validated **pure-diff** — a conservative proxy,
since the live reviewer also has tools to verify with, which can only reduce false positives
further. Validating the full live contract under an *agentic* bench, and re-validating every model
under the precision contract, are both pending.)

## Choosing a judge (the bench is only as good as it)

`review-bench` scores a reviewer with a **judge** model — so a lenient judge would silently inflate
every reviewer number. Validate any candidate against the gold answer key first:

`anvil judge-check --judge <provider>/<model>` scores it on 12 fixed (review, correct-verdict)
cases; **≥90% means it's trustworthy.** The gold set's MISSED / FALSE_POSITIVE cases are the real
test — a rubber-stamp judge fails them.

Calibration results (n=2):

| judge | access | score |
|---|---|---|
| claude-sonnet-4-6 | paid | 24/24 |
| **qwen3-coder:30b** | **free, local** | **24/24** |
| nim-qwen3.5-397b | free (slow) | 22/22 |
| nim-glm-5.1 | free (slow) | 23/23 |

**Recommended: `qwen3-coder:30b` (local)** — perfect calibration, no API key, no rate limits. Use
claude (or any strong paid model) for speed. All four cleared the bar, so the set validates
competence and catches lenient judges but doesn't *rank* capable ones — add borderline cases to
discriminate further.

## Bringing a new model

1. Pull it, run the coder bench against both tiers:
   `anvil bench --model <provider>/<model> --runs 10 --dialects generic --contract contracts/coder_local_base_v4.md`
   (then again with `coder_local_base.md`). Ship the tier that clears **~90%** — including the
   multi-step fixtures, not just the easy ones.
2. If it scores ~0 with valid-looking prose, it's emitting tool calls as text (like
   qwen2.5-coder:7b) — it needs a tool-dialect, not a contract. Consider it for reviewing instead.
3. Rate it as a reviewer — first confirm your judge is trustworthy with
   `anvil judge-check --judge <your-judge>` (≥90%), then
   `anvil review-bench --model <provider>/<model> --judge <judge>`. Watch BOTH catch (recall) and
   clean (precision); a high catch with low clean is *noisy* — try
   `--contract contracts/reviewer_local_precision.md` to fix it.
4. Record the result here and the role it earned.

## Configuring a model's contract

Set `contract` on the coder model's binding in `anvil.toml` — a tier alias (`"full"` or
`"minimal"`) or a path to a contract file. The live coder then runs under that contract
instead of the built-in prompt:

```toml
[model_bindings.my-local-coder]
provider = "local-ollama"
model    = "gemma4:e4b"
contract = "minimal"   # "full" for ~2B, "minimal" for >=4B, or a path to a .md
```

Leave `contract` unset for frontier/cloud models — they keep Anvil's built-in coder prompt.
A name that doesn't resolve warns and falls back to the built-in prompt, so a typo can't
leave the coder prompt-less.

The same field works on a **reviewer** binding — use the `"reviewer"` alias for the live reviewer
contract (tool-verify + no-false-alarm). Unset keeps the built-in reviewer prompt:

```toml
[model_bindings.my-local-reviewer]
provider = "local-ollama"
model    = "gemma4:e4b"
contract = "reviewer"
```

## What the bench does NOT yet replicate (caveats)

- **The Anvil workflow gates** — writing a plan/brief, committing per phase, following a plan —
  are not deterministically benched. A high coder score means "lands real multi-step edits," not
  "runs the full gated workflow." Treat workflow-readiness as unverified.
- **Reviewer discrimination on large diffs** — the review cases are small; they separate recall
  and false-positive rate (which already proved discriminating), but not subtle-bug-in-large-diff
  skill. Deepen with larger cases later.
