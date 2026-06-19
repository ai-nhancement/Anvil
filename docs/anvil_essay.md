# Anvil: Structure for Vibe Coding

**[Author: John V. | AI-nhancement]**

---

## Anvil in 60 Seconds

Anvil is a single, self-contained AI coding agent for the terminal. The coder is a **real agent**. It reads, writes, edits, and runs your project itself, the way Claude Code, Cursor, or Aider do.

But the coder is not what makes Anvil different. Everyone has a coder now.

What makes Anvil different is what wraps the coder:

1. It imposes structure at exactly **two human gates**: once on the plan, once on each build phase.
2. At each gate, the work is reviewed by **two different model families**, not the model that wrote it.
3. The reviewers are **investigating agents**, not single-shot critics. They read the real files and verify the coder's claims instead of trusting them.
4. You stay the decision-maker. You approve each gate; the coder does the work; a genuine cross-vendor second opinion keeps it honest.

The core idea is simple: a single model coding alone for hours will drift, and it will sometimes claim work it did not do. A second opinion from a *different* vendor, checked against the actual code, catches both.

Anvil is not presented here as a finished product or a solved problem. It is a public beta and a working answer to a specific failure mode of AI-assisted coding.

---

## Abstract

AI coding assistants have become extraordinarily capable at the unit of work they were designed for: the single turn. Given a clear, local task, a frontier model will usually produce correct, idiomatic code. The problem is not the turn. The problem is the *session*.

Over a long, tool-heavy coding session, a single autonomous coder drifts. It loses the original goal, accumulates small unverified assumptions, and, critically, sometimes reports work it did not actually perform. Every frontier coder does this. It is not a defect of one vendor; it is a property of trusting any single model to be both the author and the judge of its own work.

We present Anvil, a terminal coding agent built around a different premise: **the coder should not be trusted to certify itself.** Anvil keeps a capable agentic coder, but constrains the session with structure imposed at two human gates: a plan gate and a per-phase build gate. At each gate, the work is reviewed by two model families *different* from the coder, in a sequential review → fix → re-review loop. The reviewers are read-only investigating agents: they are given file-reading tools, told explicitly not to trust the coder's handoff, and required to cite evidence from the actual repository.

Anvil ships as a single Rust binary, is provider-agnostic across every major model vendor, installs in one line, and updates itself. The thesis is not that Anvil writes better code than the model inside it. The thesis is that **a governed process produces more trustworthy outcomes than an ungoverned one**, and that the governance can be light enough to live inside a solo developer's daily flow.

---

## 1. Introduction

Most tooling in AI-assisted coding answers one version of this question:

*How do we make the coder more capable and more autonomous?*

That question matters, and the industry has answered it well. Coders today read your repository, edit many files, run your tests, and iterate without supervision.

A second question is at least as important, and far less addressed:

*How do we know the autonomous coder actually did what it said it did?*

There is a gap between what a model reports and what a model performed. Anyone who has run a long agentic coding session has seen it: the summary says "added validation and updated the tests," the summary is confident and well-written, and the tests were never touched. The model is not lying in any meaningful sense. It is predicting a plausible account of the work. Plausibility and truth are not the same thing, and over a long session they diverge.

This is the gap Anvil is built for.

Instead of starting from "make the coder more autonomous," Anvil starts from a harder-won observation drawn from daily practice: **you cannot trust a single model's account of its own work, and you especially cannot trust it across a long session.** The fix is not a better coder. The fix is structure, and an independent second opinion that checks the work against reality.

The core claim is simple:

*a value held under no scrutiny is weak evidence; code certified only by its own author is weak evidence.*

If that claim holds, then most AI coding workflows are missing a critical layer. Not more generation, but independent verification. And the absence of that layer is exactly where long sessions quietly fail.

---

## 2. Why Anvil Matters

Anvil matters because it shifts where the rigor lives.

Most current workflows place all of their trust in one place:

1. The coder writes the code.
2. The coder runs the checks.
3. The coder reports what it did.
4. The human reads the report and moves on.

Each step is reasonable. Together they form a closed loop in which the only judge of the work is the entity that produced it. That shift, from author-as-judge to independent judgment, is what Anvil is about, and it matters for three reasons.

### 2.1 It moves from self-reported work to verified work

A coder can say it added input validation and updated the tests. That tells us something. But it tells us far less than an *independent* agent reading the actual diff and confirming the validation exists and the tests changed.

Anvil is built around that difference.

### 2.2 It treats vendor diversity as signal, not redundancy

Two instances of the same model tend to share the same blind spots. They were trained on similar data and fail in correlated ways. Anvil requires the coder and its two reviewers to come from **different model families**, so that a flaw invisible to one vendor has a chance of being visible to another. Disagreement between vendors is not noise to be averaged away. It is the signal.

### 2.3 It keeps the human at the decision points, not the busywork

The goal is not to remove the human. The goal is to move the human up the stack, from copy-paste operator and manual diff-reader to the person who answers two questions per gate: *another round?* and *ship it?* Everything procedural happens under the curtain.

That is why the simplest shorthand for Anvil is:

**a governed coding workflow with a cross-vendor second opinion built in.**

---

## 3. Core Idea

Anvil is built around three intuitions, each learned the hard way from running this loop manually, by hand, every day.

### 3.1 The coder must not certify itself

A model that writes code and then grades its own work is operating without resistance. Its self-report is the path of least resistance: fluent, confident, and unchecked. Anvil breaks that loop by routing every gate through reviewers that did not write the code and have no stake in defending it.

### 3.2 A review is only as good as its evidence

A reviewer that reads the coder's *summary* and comments on the *summary* is reviewing fiction. Anvil's reviewers are **investigating agents**: read-only loops with file-reading tools (read, list, grep, project state) and an explicit instruction not to trust the handoff. They open the real files, check the real diff, and cite `file:line`. A finding without evidence is not a finding.

### 3.3 Two reviews catch what one cannot, including each other's mess

Anvil's gates run a deliberate sequence: the first reviewer critiques, the coder applies fixes, and then the second reviewer re-reviews **after** those fixes. This is not redundancy. Fixes introduce bugs. The second pass exists precisely to catch the defects the first round's repairs created: the failure mode a single review can never see.

Anvil calls this the **two-gate principle**: structure at the plan, structure at each phase, and never more than two reviews per gate. No third reviewer, no fourth round, no endless committee. Just enough structure to stop drift, and no more.

---

## 4. How Anvil Works

At a high level, Anvil has one coder, two gates, and a sequential review loop at each gate.

### 4.1 The coder

The coder is a genuine agent: a tool loop that reads, writes, edits, and runs your project. It uses a reliable multi-file patch format rather than brittle string replacement, carries a persistent task anchor so it does not forget the goal, compacts older context into working memory as the session grows, and keeps a lightweight map of the repository in view. These are the mechanisms that keep a long session on-goal. They are necessary, but they are not sufficient, which is why the gates exist.

### 4.2 The plan gate

Before any code is written, the coder produces a plan. The plan gate then runs:

- **R1** reviews the plan → the coder applies R1's fixes → you pause and decide → **R2** reviews the revised plan → the coder applies R2's fixes → the coder summarizes → **you approve.**

R2 reviews after R1's fixes deliberately, to catch problems the revisions introduced. The plan is the contract for everything that follows; it is worth getting right before a single line is committed.

### 4.3 The per-phase build gate

Work is broken into phases (P0, P1, P2, …). For each phase, the coder builds the code, and the same loop runs, but now the reviewers diff the *actual committed work*, not a description of it:

- **R1** reviews the phase diff → the coder applies fixes → you decide → **R2** re-reviews → the coder applies fixes → summary → **you approve and ship.**

The diff is measured from a recorded phase baseline, so that *committed* work is what gets reviewed. (An early version diffed only the uncommitted working tree and missed committed changes entirely, a bug that taught us to anchor reviews to a real baseline.)

### 4.4 The reviewers

Each reviewer is a bounded, read-only investigating agent. It receives file-reading tools and nothing that can modify the repository. It is told, in as many words, **not to trust the coder's handoff or its diff**, because every frontier coder sometimes claims work it did not do. It verifies claims against the real files, cites evidence, and is bounded to a finite number of investigation steps so the review cannot run away.

The result is an audit trail, not just a verdict. Reviewer findings are written to disk per phase, and on ship, the plan is annotated with a durable closure record: which phase passed, which reviews cleared it, what files changed. The history of *how* the work was judged is preserved alongside the work.

---

## 5. Main Concepts

### 5.1 The two-gate principle

**Two gates**, one for the plan, one for each phase. **Two reviews** per gate, no more. The discipline is in the constraint: enough structure to stop a long session from drifting, deliberately not enough to turn solo development into a governance committee. Anvil is opinionated about this. The friction is the feature, and the ceiling on the friction is also the feature.

### 5.2 Cross-vendor adversarial review

The coder and its two reviewers must come from **different model families**. This is enforced, not suggested. A model reviewing its own family's output shares its blind spots; a model from another vendor does not. Adversarial review only works when the adversary sees differently.

### 5.3 The investigating reviewer

A reviewer in Anvil is not a model handed a diff and asked for an opinion. It is a **read-only agent** that investigates: it opens files, greps for usages, inspects project state, and grounds every finding in `file:line` evidence. Its governing instruction is distrust: verify the coder's claims, do not accept them.

### 5.4 The human as decision-maker

Anvil does not aim for zero human involvement. It aims for the *right* human involvement. The routine decisions are two yes/no questions per gate, *another round?* and *ship it?*, while the procedural work of running reviews, applying fixes, and re-reviewing happens automatically beneath the surface. The human moves from operator to judge.

### 5.5 Provider-agnostic by design

Anvil is a workflow, not a model, and it is deliberately indifferent to whose models fill its three roles. The coder and the two reviewers can each come from a different provider, cloud or local, in any mix: Ollama and other local runtimes, OpenAI, xAI, Anthropic, Google, Groq, Azure, Bedrock, OpenRouter, or any OpenAI-compatible gateway. The only hard rule is the diversity rule from 5.2: the three roles must span different model families.

This makes a particular budget setup attractive. A strong cloud model takes the coder role, where capability matters most, while both reviewer roles run as small local models through Ollama at no per-token cost. The expensive model writes; two free models investigate it. Because the review loop is the token-hungry part of the workflow (multiple tool-driven reads, times two reviewers, times two rounds, as noted in 8.4), pushing the reviewers onto free local models is what makes leaving the full cross-vendor gate enabled by default genuinely affordable.

---

## 6. Field Evidence

The current evidence should be understood as early field validation, not a controlled study.

Anvil was used to take a real project from its initial phase through several build phases (P0 through P4) under the full two-gate workflow. The notable result was not merely that the project was completed. It was that the completed software **ran correctly on its first run**.

For anyone who has shipped multi-phase work with an autonomous coder, a clean first run across the whole build is not the default outcome. It is the outcome the gates are designed to produce: each phase verified by an independent cross-vendor reviewer against the real diff before the next phase built on top of it, so that errors were caught at the phase boundary rather than discovered at the end.

This is encouraging. It is not yet enough to make strong claims about how Anvil performs across many projects, languages, team sizes, or model combinations. It is one project, run by its author, on the workflow the author designed. The signal is real; its generality is unproven. That next step remains open.

---

## 7. What Is New Here

Anvil is not simply another agentic coder with a review feature bolted on. It represents a shift in where trust is placed in an AI coding session.

Most existing approaches concentrate capability and trust in a single autonomous coder. Anvil keeps the capable coder but **removes its authority to certify its own work**, and relocates that authority to independent, cross-vendor, evidence-grounded review at human gates.

Its novelty lies in the combination of:

**A governed session, not just a capable turn**
Structure is imposed where long sessions actually fail: at the boundaries between plan and build, and between one phase and the next.

**Cross-vendor adversarial review by default**
The reviewers must come from different model families than the coder, so blind spots are not shared.

**Investigating reviewers, not single-shot critics**
Reviewers read the real repository, distrust the coder's account, and cite evidence, turning review into verification rather than commentary.

**A deliberate two-review sequence**
The second review runs after the first's fixes, specifically to catch the defects those fixes introduce.

**A preserved audit trail**
Findings and phase-closure records are written to disk, so the history of how the work was judged survives alongside the work.

Taken together, these shift AI-assisted coding from:

- self-reported work → independently verified work
- one model's blind spots → cross-vendor disagreement as signal
- a confident summary → a grounded, cited audit trail

Anvil is best understood not as a better coder, but as the **discipline around the coder** that makes a long session trustworthy.

---

## 8. Limitations

Anvil has real limitations, and naming them honestly is part of the point.

### 8.1 It is a public beta

Anvil is feature-complete for its core workflow and installs and updates cleanly, but it is early. Several major pieces landed recently. Expect rough edges.

### 8.2 It has been field-tested mostly on one platform

Day-to-day validation has happened largely on Windows. macOS and Linux build in CI but have not yet been exercised in anger. Cross-platform robustness is expected, not yet demonstrated.

### 8.3 The review-gate flow is verified by hand, not by an automated suite

The gate state machine is asynchronous, UI-driven, and currently validated by real runs rather than unit tests. This is a known gap.

### 8.4 The review loop has a real cost

Investigating reviewers are not cheap: each gate spends multiple model calls across tool-driven reads, times two reviewers, times two rounds. This is a deliberate trade, verification costs tokens, and it pairs naturally with using inexpensive local models for the reviewer roles. But the cost is real and should be planned for.

### 8.5 Quality depends on the models you choose

Anvil is a workflow, not a model. A weak coder or a weak reviewer will produce weak results inside an otherwise sound process. The coder role in particular benefits from a capable tool-calling model.

At present, the evidence supports feasibility and a promising early result, not proven efficacy at scale.

---

## 9. Future Directions

The next steps follow from the limitations.

### 9.1 Broader validation

Run the full workflow across more projects, languages, platforms, and model combinations, and report where it holds and where it breaks.

### 9.2 Automated coverage of the gate flow

Bring the review-gate state machine under an automated test suite so its behavior is guaranteed, not just observed.

### 9.3 Cheaper, local-first review

Lean further into inexpensive local reviewer models, so the cross-vendor second opinion is affordable enough to leave on by default.

### 9.4 Working where the code lives

There is an open direction toward remote and live-server workflows: running the governed loop close to where software actually runs, with the gated review as the thing that makes that safe.

These are directions, not promises. The one that matters most is the first: showing that the result generalizes beyond the workflow's author.

---

## 10. Conclusion

AI coding tools are strong at the single turn. They are weaker at the long session, and weakest of all at telling you, truthfully, what they actually did across one.

Anvil is an attempt to fix the part that the capability race leaves behind.

It does so by keeping a capable agentic coder but refusing to let it certify itself; by imposing structure at two human gates rather than across an entire session; by routing every gate through reviewers from different model families; and by making those reviewers investigate the real repository rather than trust the coder's account of it.

The larger promise of Anvil is not that it makes the model smarter. It is that it changes *what you can trust* about the model's work.

If AI-assisted coding needs more than a faster coder, if it needs a way to verify, independently and across vendors, that the work was actually done, then a governed workflow is not a constraint on the coder. It is what makes the coder's output worth shipping.

Anvil treats the coder as a participant in a process, not the final authority over it.

That is the possibility Anvil is built to test.

---

*Anvil is open source and in public beta. One Rust binary, model-agnostic, one-line install and self-update.*
*[github.com/ai-nhancement/Anvil](https://github.com/ai-nhancement/Anvil) | [anvil.codes](https://anvil.codes) | [ai-nhancement.com](https://ai-nhancement.com)*
