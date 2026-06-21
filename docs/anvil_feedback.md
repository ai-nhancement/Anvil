# Anvil v0.4.7 Feedback Notes

These notes capture the high-level product/codebase impressions after the `v0.4.7` jump, plus follow-up suggestions worth considering before broader public polish.

## Overall impression

Anvil now feels much more like a real product than an experimental local TUI agent. The product identity, public README, agent architecture, and update/install story are all becoming coherent around a clear thesis:

> A real coding agent, wrapped in a disciplined two-gate workflow with cross-vendor review so long AI coding sessions do not quietly drift off the rails.

The strongest shift is that Anvil no longer reads as “another coding agent.” The differentiator is now the workflow: human approval gates, sequential R1/R2 critique, and a deliberate second opinion from different model families.

## What feels strong

### Clearer identity

The “Forge the Workflow” positioning, forge-themed TUI, heat language, and visual polish give Anvil a recognizable voice. It feels distinct without needing to invent a new technical category.

### Sharper product thesis

The README makes the core argument well: the coder itself is not the unique part anymore; the governed workflow around the coder is. That is a stronger public story than simply competing on raw agent capability.

### More serious agent core

The tool set has the right shape for a practical coding agent:

- `apply_patch` as the preferred editing path.
- Offset/limit `read_file` for large files.
- `project_state` to ground the agent in live workflow/git reality.
- Bounded tool output to avoid context blowups.
- Command approval and auto-approval prefixes.
- Durable memory/context files.
- Delegation to scoped specialists.

Together, these make the agent feel intentionally engineered rather than demo-like.

### Specialists are a strong direction

The `researcher` and `repo-scout` specialists are scoped evidence gatherers, not alternate decision-makers. That fits Anvil’s philosophy well: specialists retrieve evidence; the governed coder remains responsible for synthesis and edits.

This is a promising extension point because it adds capability without making the main agent more chaotic.

### Self-update is a product milestone

The addition of `anvil update`, cached boot-time update checks, release target mapping, and in-place replacement makes Anvil feel installable and maintainable as a real CLI product.

### README is public-beta-ready

The README now explains:

- What Anvil is.
- What makes it different.
- How to install/update it.
- How the workflow works.
- What rough edges remain.

The public beta caveat is honest without underselling the project.

### Forge theme is landing

The forge visuals and wording give the app personality. The theme works best when public-facing role labels stay clear, such as `CODER`, while flavor appears around the edges through heat, cursor, update indicators, and workflow terms.

## Suggestions / watch areas

### 1. Watch metaphor density

The forge language is fun and memorable, but too many terms at once can become confusing for new users. Current examples include:

- forge
- smithing
- clinkering
- tempering
- quenching

Recommendation: keep the theme, but pair metaphor with plain meaning the first time each term appears. For example:

> `/accept-plan` — quench the plan, recording its hash and unlocking phase work.

That keeps the brand flavor while preserving clarity.

### 2. Consider splitting `src/ui.rs`

`src/ui.rs` is carrying a lot of responsibility. It is not necessarily urgent, but it is likely the biggest maintainability pressure point as the TUI grows.

Possible future split:

- `src/ui/render.rs` — layout and rendering orchestration.
- `src/ui/input.rs` — keyboard handling and input buffer behavior.
- `src/ui/wizard.rs` — configuration wizard state and rendering.
- `src/ui/commands.rs` — slash command parsing/dispatch.
- `src/ui/popups.rs` — palette, docs, confirmations, overlays.

Recommendation: do this only when touching related areas anyway, to avoid a broad risky refactor.

### 3. Clean up workflow wording consistency

The README describes the newer sequential review loop clearly:

1. R1 reviews.
2. Coder applies fixes.
3. User continues.
4. R2 reviews the revised artifact.
5. Coder applies fixes.
6. User approves.

Some older comments or legacy CLI wording may still sound like the older review flow. Before a larger public push, it would be worth doing a terminology pass across:

- `README.md`
- `src/main.rs` CLI help text/comments
- `src/phase.rs`
- `src/plan.rs`
- TUI slash command help text

Goal: make sure all public/help text describes the same workflow.

### 4. Add automated gate-flow coverage over time

The README honestly says the review-gate flow is mostly hand-verified. That is fine for beta, but broader adoption would benefit from smoke/integration tests around the workflow state machine.

Useful future tests:

- Plan file detection and hash acceptance.
- R1/R2 review artifact creation.
- Phase start/current phase state.
- Phase briefing/review/ship transitions.
- Re-running `/accept-phase` after new diffs.
- Handling missing or stale review artifacts.

Even a small suite here would make the most important Anvil-specific behavior feel safer.

### 5. Keep global config failure modes obvious

Global provider/model setup is a major usability win. The main risk is confusion when a repo appears to use an unexpected model or provider.

Recommendation: keep surfacing config provenance plainly in status/help/UI:

- Which config file supplied the active provider/model.
- Which roles are globally configured.
- Which roles are overridden locally, if any.
- What to run to change them.

This helps users understand “why is this repo using that model?” without digging through config files.

## Coder's-eye view: friction I actually hit while working

These are not product-strategy notes — they are the concrete papercuts and missing trust signals I run into *as the agent doing the work* each session. They are mostly cheap to fix and would make every session faster and more confident.

### A. Fix the formatting baseline (highest practical value)

`decisions.md` records that `cargo check` passes but `cargo fmt --check` reports pre-existing drift across unrelated files. This is a permanent low-grade tax: every commit warns, and I have to actively *avoid* running `cargo fmt` so I don't produce a huge noisy diff. A single one-time "format everything" baseline commit removes this friction forever and makes future formatting diffs meaningful again.

### B. Fill in `ARCHITECTURE.md`

It is listed as part of the coder's grounding, but it is currently just the empty template. For a ~17-file codebase with a very large `src/ui.rs`, a real map (what each module owns, where the gate/state machine lives, how plan/phase/review artifacts flow) would save re-deriving structure at the start of every session. Low effort, high recurring payoff.

### C. Clean stray `.log` files out of the repo root

The root is littered with ~50 `check-*.log` / `run-*.log` files. They bury the real files in `list_dir`, add noise to navigation, and risk being committed. Recommendation: `.gitignore` them and/or relocate under `.anvil/logs/`.

### D. A thin smoke net around the gate/state machinery

This overlaps with suggestion #4 above, but from the coder's seat the motivation is concrete: I edit `phase.rs` / `plan.rs` / state code with no automated signal that I broke a transition. Even a handful of tests over plan-hash acceptance and phase start→review→ship transitions would let me refactor that core confidently instead of hand-tracing.

### E. A "flag risk now" channel, not just at the gate

Today, mid-task uncertainty goes into `assumptions.md`, which the human may not look at until a gate. A lightweight way to surface "I'm proceeding but this decision deserves your eyes *now*" would catch wrong turns earlier, before they compound across a phase.

### F. Context-budget visibility (`/context`)

The window is sized per-model and compaction fires automatically, but neither the user nor I can easily see how full it is or when compaction is about to happen. A small readout (tokens used / budget / "compaction imminent") would make the memory behavior feel less like a black box and help decide when to commit/checkpoint.

### Suggested order of attack

These are all small and independently shippable. Rough priority by value-per-effort:

1. Real `ARCHITECTURE.md` (B).
2. `.gitignore` / relocate stray `.log` files (C).
3. One-time `cargo fmt` baseline (A).
4. Smoke tests for the gate machinery (D).
5. Risk-flag channel (E) and `/context` readout (F) as follow-ups.

## Bottom line

Anvil now feels coherent: a branded, installable, self-updating terminal coding agent with a defensible product angle.

The most valuable next polish is probably not adding more features, but tightening clarity:

1. Keep public wording clear while preserving the forge identity.
2. Reduce future maintenance pressure in the TUI.
3. Align all workflow terminology.
4. Add targeted tests around the gate system.

That combination would make Anvil feel less like a fast-moving beta and more like a durable tool people can trust with real projects.
