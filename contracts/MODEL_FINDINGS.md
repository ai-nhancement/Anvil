# Local-model findings — what works, and in which role

What each local model we tried can actually do in Anvil, **measured on the bench**, not
guessed. For each model: the contract that works best, how it scores as a **coder** and as a
**reviewer**, what it *can't* do, and the **role** we'd actually put it in. Bringing a new
model? The recipe at the bottom tells you how to slot it in.

> Numbers from `anvil bench` (coder: 10 fixtures incl. multi-step ones, n=10 → /100, scored by a
> real check passing) and `anvil review-bench` (6 planted-bug cases → catch /30, + 1 decoy →
> clean /5, scored by claude-sonnet-4-6 as judge). Run on 2026-06-25.

## Summary

| model | size / family | coder contract | coder | reviewer (catch / clean) | best role |
|---|---|---|---|---|---|
| gemma4:e2b | 2B / gemma | **full** (`coder_local_base.md`) | 83/100 | 28/30 · 3/5 | light coder (simple edits) |
| gemma4:e4b | 4B / gemma | **minimal** (`coder_local_base_v4.md`) | **94/100** | 30/30 · **1/5** | **coder** (not reviewer — noisy) |
| qwen2.5-coder:7b | 7B / qwen2.5 | — (cannot drive tools) | **0/100** | 14/30 · **5/5** | **reviewer only** (high-precision) |
| qwen3-coder:30b | 30B / qwen3 | **minimal** (`coder_local_base_v4.md`) | 93/100 | **30/30 · 5/5** | **coder + reviewer** (all-rounder) |

*catch = planted bugs flagged (recall); clean = decoys left alone (precision / no false alarms).*

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
- **Reviewer 30/30 catch but only 1/5 clean.** Catches every planted bug — and also "finds" bugs
  in correct code 4 times out of 5. **Too noisy to trust as a reviewer.**
- **Role: coder.** It's the strongest small coder here; keep it off the review gate.

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

## How this maps to Anvil's two-gate review

Anvil wants a strong **coder** plus a **different-family reviewer**. The data says: code with
**e4b** or **qwen3-coder:30b**; review with **qwen3-coder:30b** (best) or **qwen2.5-coder:7b** (a
free precision reviewer that can't code but reads well). **Don't** review with **e4b** — it's a
great coder but a noisy reviewer. Coding skill ≠ reviewing skill; pick per role, not per model.

## Bringing a new model

1. Pull it, run the coder bench against both tiers:
   `anvil bench --model <provider>/<model> --runs 10 --dialects generic --contract contracts/coder_local_base_v4.md`
   (then again with `coder_local_base.md`). Ship the tier that clears **~90%** — including the
   multi-step fixtures, not just the easy ones.
2. If it scores ~0 with valid-looking prose, it's emitting tool calls as text (like
   qwen2.5-coder:7b) — it needs a tool-dialect, not a contract. Consider it for reviewing instead.
3. Rate it as a reviewer:
   `anvil review-bench --model <provider>/<model> --judge reviewer-a` — watch BOTH catch (recall)
   and clean (precision); a high catch with low clean is a *noisy* reviewer.
4. Record the result here and the role it earned.

## What the bench does NOT yet replicate (caveats)

- **The Anvil workflow gates** — writing a plan/brief, committing per phase, following a plan —
  are not deterministically benched. A high coder score means "lands real multi-step edits," not
  "runs the full gated workflow." Treat workflow-readiness as unverified.
- **Reviewer discrimination on large diffs** — the review cases are small; they separate recall
  and false-positive rate (which already proved discriminating), but not subtle-bug-in-large-diff
  skill. Deepen with larger cases later.
