# Anvil — Project Charter

**Project name:** Anvil
**Date:** 2026-05-15
**Status:** **Approved + Amendment A1 Applied** (R1–R4 rotation closed by human-arbiter convergence on 2026-05-15, see `CHARTER_CONVERGENCE.md`; Amendment A1 R1–R2 rotation closed by human-arbiter convergence on 2026-05-19, see `AMENDMENT_A1_CONVERGENCE.md`; amendment content lives in the new *Amendment A1 — Applied* section below)
**Authoring model:** Claude (pinned interlocutor for the discussion stage)
**Provenance:** Synthesized from a multi-turn discussion between John Canady and Claude on 2026-05-15. The discussion explored porting the COGS (Coordination of Governed Specialists) frame from AiMe into a coding-focused workbench and converged on the commitments below.

**On the name.** *Anvil* was chosen over alternatives (*Quorum*, *Agora*, *Crucible*) to lead with the craft-and-iteration discipline at the heart of the system: a place where work is shaped through deliberate, repeated hammering against a solid surface. The workshop framing fits a system whose value proposition is *the workflow itself*, not the cleverness of any single specialist.

---

## Executive Summary

Anvil is a bounded-authority development system for coding work, built on the COGS frame and on a process discipline the user already practices manually. It is **not** a coding agent in the autonomous sense. It is a **human-gated workflow** with structured artifact production at every stage and cross-vendor adversarial review at every gate.

The pitch is not "smarter codegen." The pitch is: **the workflow is the product.** The system enforces a discipline — Discuss, Plan, Build in phases, review every artifact independently before it can become the input to the next stage, and rotate reviewer models so blind spots from any single training distribution cannot propagate unchecked. The COGS frame provides the substrate. The workflow provides the value.

---

## Why This Project Exists

Existing coding agents (Claude Code, Codex, Cursor and so on) operate as single-loop agentic systems. One model holds every role at once — reasoner, planner, writer, reviewer, verifier — and quality drifts as session context grows. The model re-decides things it already decided. Implicit goals carried in chat history are not durable. Reviews, when they exist, are advisory rather than gating, and are almost always conducted by the same model that wrote the code, which means the reviewer shares every blind spot of the writer.

The user has been working around these limitations manually: one model writes a phase plus a structured review briefing, a second-vendor model reviews against the briefing, findings are addressed, a third-vendor model performs a second review, the loop continues until convergence, and only then does the phase ship. In practice this produces noticeably higher-quality output than a single-model loop — measurable against the criteria in *Evaluation Criteria* below — but it is hand-driven and expensive in human attention. Anvil's job is to make the discipline cheap enough to be repeatable.

Anvil formalizes that practice as system infrastructure. It is the discipline made enforceable.

---

## Core Architectural Commitment

Anvil operates as a **pipeline of bounded specialists, human-gated at every transition, producing a graph of durable artifacts.**

Three commitments stack to form the load-bearing wall:

1. **A staged workflow** — Discuss → Plan → Build (per phase) → Ship. Every stage produces a durable artifact. Every artifact is reviewed before it can become the input to the next stage.
2. **A single writer** — one model writes everything in the project: code, briefings, dispositions, plan amendments. Voice consistency across all artifacts is treated as load-bearing.
3. **Cross-family adversarial review** — reviewers are drawn from different *model families* than the Coder (the operational independence floor; cross-vendor is the preferred default). They rotate deterministically per cycle. Their findings are advisory to the human, who is the actual authority.

None of these three commitments are negotiable without re-opening the Charter.

---

## What This Is

- A bounded-authority coding workbench, single-user, project-scoped
- A workflow enforcement layer built on a COGS specialist substrate
- An artifact-graph system where every doc has provenance and every change has a trail
- Model-agnostic at the config level — no hard dependency on any vendor
- Local-first — artifacts on disk in the user's file system, no required cloud component for state

## What This Is Not

- Not a Claude Code clone, not a Codex clone, not a Cursor clone
- Not a generic agent framework — the workflow is opinionated and enforced
- Not an autonomous multi-agent orchestrator — the human gates every transition
- Not a single-loop agentic system — every stage is bounded, every transition is explicit
- Not a multi-user team system in v1 — single coordinator, single *active* project at a time (the storage layer may hold multiple projects; exactly one is the focus of work in any given session)

---

## The Workflow

### Discuss

A single broader-scope **Interlocutor** specialist holds an exploratory conversation with the user. The goal is to lock down what the project is, what it is not, the boundaries, the non-goals, the core architectural commitments, and the named open items. The output is a Charter artifact — this document is the worked example of what a Charter looks like.

The Interlocutor is not COGS-narrow internally; it is a single broad-scope conversational thinking partner. By default, the Interlocutor is the same model as the Coder, because writing the Charter is itself a coder rendering job and voice consistency is preferred. This default is overridable per project.

The discussion stage ends when the user explicitly says "lock it down." The system does not auto-detect closure.

### Charter Review

The Charter is reviewed by two or more models drawn from the project's reviewer pool, which must satisfy the Adversarial Diversity invariant (floor: different model family from the Interlocutor / Coder; default: different vendor). Findings are surfaced to the user, who edits, accepts, or rejects each finding. The Coder then renders the converged Charter (with hardening notes folded into the body, see *Plan Consolidation*). The reviewed Charter is the input to the Plan stage.

### Plan

A **Planner** specialist consumes the approved Charter and produces a Phased Plan document. The Coder (single-writer invariant) renders the Plan document from the Planner's structured output.

The Charter-level **Planner Contract** is locked here (full specification is Plan-stage; the abstract contract below is invariant):

- *Inputs the Planner may rely on:* the approved Charter (consolidated version), the Charter's Required Project-Level Choices (filled by the user, see *Governance Taxonomy*), any prior Plans for this project (if iterating), and the deferred-decision registry (open items not yet resolved).
- *Outputs the Planner must produce:* a phased Plan document, a phase dependency graph (linear or DAG, explicitly declared), cross-cutting concerns mapped to phases, and acceptance criteria per phase.
- *Required per-phase fields:* phase ID + short name, goal (one sentence), action list, deliverable artifact(s), acceptance criteria (specific and testable), dependency list (which prior phases must be shipped first), hinge-test list (deferred decisions this phase encodes, with the conditions that would flip each one), evaluation-metric impact (which metrics from *Evaluation Criteria* this phase moves), estimated rounds-to-convergence (informs scheduling).
- *Phase size rule (constraint, not mechanism):* a phase must be small enough that one reviewer can read it in a single review session, *and* large enough to deliver standalone value (an integrated piece of system functionality, not arbitrary code chunks). One phase = one logical decision boundary. The Charter commits only to these constraints — the *implementation* of phase-size measurement (line count caps, reading-time estimates, complexity heuristics, learned scoring) is Plan-stage. The Charter must not be read as mandating a specific phase-size mechanism.
- *Cross-cutting concerns:* data models, invariants, evaluation metrics, and deferred decisions are declared at Plan level and referenced by phases. Each phase declares its impact on the relevant cross-cutting concerns.

Anything beyond this minimum (specific schemas, validation logic, heuristics versus learned planning) is Plan-stage.

### Plan Review

The Plan is reviewed by two or more models drawn from vendors different from the Planner and Coder. The same finding-surface / human-edit / coder-disposition cycle applies. The Plan is a **living document**: review findings get folded back into the Plan as appended "Hardening Notes (Rn — Consolidated)" sections. The Plan and the Reviews are not parallel streams; they merge.

### Build (per phase)

For each phase in the approved Plan:

The **Coder** specialist implements the phase. The Coder writes:

- The code itself
- A structured **Phase Review Briefing** document, which is a self-audit containing:
  - A *Files changed* table with action and purpose
  - An *Architecture Compliance* table mapping plan invariants to evidence
  - A *What to Review* section with numbered specific questions for the reviewer
  - A *Test Coverage Summary* naming what is tested and what is deferred
  - A *How to Activate for Testing* runbook
  - A short human-facing "what was done" summary at the top

The Coder is the **only writer** in the system. Reviewers do not write files. The Coder renders all artifacts from packets.

### Phase Review

The user reviews the briefing summary and approves the briefing to be sent to a reviewer. The reviewer is selected by deterministic rotation from the project's reviewer pool. The reviewer reads the briefing plus the code plus the relevant plan section and produces a **structured findings packet**:

- Each finding has: id, severity (P1 / P2 / P3), location (file:line where applicable), claim, evidence, recommendation

The findings packet is presented to the user, who can:

- Keep findings unchanged
- Drop findings the user judges to be wrong
- Edit findings (modify severity, recommendation, or evidence)
- Annotate findings with notes

The curated findings packet is then sent to the **Finding Verifier**, a specialist whose job is strictly *evidence validation*: given a finding with code anchors (file:line, symbol name, claim shape), does the cited evidence actually exist where the reviewer says it does? Findings emerge as *grounded*, *refuted*, or *cannot-be-verified* (semantic claims that cannot be reduced to grep / AST checks). The Verifier protects against confident-wrong reviewer anchors. **The Verifier does not decide whether a finding's recommendation is worth acting on** — that is the human's role at curation time. Verification is about facts; disposition is about judgment.

The verified findings packet is sent to the Coder. The Coder applies fixes to the code, then renders the **Phase R2 Disposition Document** containing:

- A *Verification of Rn claims* section grounding the reviewer's evidence
- A *Disposition of Rn findings* table with id, severity, finding, and disposition (fixed / deferred / refuted / needs-clarification)
- A *Files changed since Rn* table
- A *Corrections to the Rn narrative* section explicitly superseding any reviewer prose that was factually wrong
- A *Residual / deferred* section for findings deliberately not addressed in this phase
- A *Reproducibility* section with commands to verify the live state

The user reviews the disposition and approves either to ship the phase or to send back to a different reviewer. Rotation continues until the **termination condition** is met: by default, every reviewer in the project's pool has produced a clean pass on the *most recent state* (full-pool clean). The condition is configurable per project to *single clean pass* (any one reviewer's most recent pass is clean) when the lower independence guarantee is acceptable. The default is the stronger guarantee because the whole point of rotation is that every model in the pool has had a chance to see the final state.

**Convergence safeguards.** Rotation must converge. To prevent the *Two Generals' Problem* failure mode — reviewers alternating minor stylistic or pedantic findings forever — the termination condition is paired with two safeguards:

- **Severity-tiered convergence.** After a configurable round limit (default: 5 rounds for a single artifact), only **P1 findings** can block. P2 and P3 findings raised in rounds 6+ are *advisory* — they may inform the next phase or the next round of Plan refinement, but they do not block the current artifact from shipping. The round limit is part of *Required Project-Level Choices* (see *Governance Taxonomy*).
- **Human arbiter authority.** At any point in rotation, the Coordinator may declare the artifact *convergent* and force the ship state. Declaring convergence is itself a logged event in the audit store (record type `convergence-declaration`) and must include the Coordinator's reasoning. This is not a backdoor around the discipline; it is the explicit acknowledgement that the human is the final authority and that perfectionism is itself a workflow failure mode. The audit trail surfaces the count of convergence declarations per project; a project that relies on them frequently is a signal that reviewer configuration or briefing quality needs attention.

The two safeguards compose: rounds 1–5 are pure rotation; rounds 6+ tier by severity; at any round, the human may declare convergence.

### Ship

When all phases in the Plan are approved, the project (or sub-project, or feature) is shipped.

The Charter-level **Ship Abstraction** is: *Ship is the moment the human approves the current artifact set as the canonical output of the phase / project.* Transport actions — git commit, tag, deploy, hand-off to a downstream system — are **Plan-level implementations** of that approval, not Charter-level commitments. The Charter commits only to the abstract semantics: Ship is the threshold at which artifacts cross from "in-progress" to "canonical, referenced by downstream phases or external consumers." The specific transport (or transports — a project may use several) is a Plan decision parameterized per project.

Ship approval always sits with the human. Re-opening a shipped phase is governed by the *Rollback / Re-Open* invariant.

---

## Specialist Roster

| Specialist | Role | Writer? | Model assignment |
|---|---|---|---|
| Interlocutor | Discussion-stage conversational partner; produces Charter packet | No (rendered by Coder) | *Default:* same as Coder. *Override:* any model satisfying the Charter Review independence floor |
| Planner | Consumes Charter, produces Phased Plan packet (per Planner Contract) | No (rendered by Coder) | *Default:* same as Coder. *Override:* any model |
| Coder | Implements phases; **renders all human-facing project artifacts** | **Yes — the only writer of human-facing artifacts** | Pinned per project (model + version); upgrade requires Charter amendment |
| Reviewer Pool | Independent critique against the relevant upstream artifact | No (findings are packets, logged to audit store as machine-readable records) | Pool of ≥2 models. *Floor (enforced):* no reviewer shares model family with Coder. *Default (preferred):* cross-vendor and ≥2 distinct vendors. Rotated deterministically per cycle |
| Finding Verifier | Grounds reviewer findings (anchored evidence only) against actual code state before Coder acts | No (verified packet, logged to audit store) | May share family with Coder; grounding is mostly deterministic |
| Coordinator | Gates every transition; edits findings; approves artifacts | No | The human |

The Coder is pinned because writing wants consistency. The Reviewer pool rotates because review wants diversity. This asymmetry is the core principle of the system and is enforced by the config schema, not by convention.

---

## Governance Taxonomy

Every load-bearing rule in this Charter falls into one of three buckets. The bucket determines whether a project may parameterize the rule.

**Immutable Invariants (cannot be relaxed; violation requires re-opening the Charter):**

- Single-Writer (one model authors human-facing artifacts)
- Adversarial Diversity *floor* (no reviewer shares model family with Coder)
- Human Gates Every Transition (the six gates are non-negotiable; their *enumeration* is exhaustive)
- Findings Are Advisory (human curation step non-optional)
- Findings Are Grounded (Verifier-confirmed anchored evidence before Coder action)
- Provenance Graph (versioned references between artifacts)
- Cross-Reference Integrity (human-facing decision ↔ machine-readable record consistency)
- Artifact Encoding (UTF-8 for human-facing artifacts)
- Living Plan (Plan absorbs converged findings)
- Plan Consolidation (Hardening Notes periodically absorbed into main body)
- Deferred Decisions Are Tracked (queryable artifacts, not comments)
- Coder Model Pinning (Coder is pinned per project; upgrade requires Charter amendment)
- Rollback / Re-Open semantics (re-opening a shipped phase invalidates dependent phases)
- Audit-Store Minimum Schema (record types, cross-reference key, append-only, creation-before-Ship; see below)

**Default Policies (Charter states a default; projects may override at config time):**

- Adversarial Diversity *default*: cross-vendor reviewer pool, ≥2 vendors collectively (relaxable to cross-family only when supply requires)
- Termination Condition: *default* full-pool clean; *override* single-clean-pass
- Interlocutor model: *default* same as Coder; *override* any model
- Planner model: *default* same as Coder; *override* any model
- Plan Consolidation triggers: *default* phase boundary or note-count threshold; *override* time-based or manual

**Required Project-Level Choices (must be locked at project creation, before Plan stage begins):**

- Specific Coder model + version (subject to Pinning invariant once chosen)
- Specific reviewer pool members (≥2 models, family floor enforced)
- Termination Condition selection (full-pool clean or single-clean-pass)
- Convergence round limit (default: 5 rounds before severity-tiering takes effect; see *The Workflow / Phase Review / Convergence safeguards*)
- Interlocutor model (default or override)
- Planner model (default or override)
- Plan Consolidation triggers (default, time-based, manual, or hybrid)
- Per-metric numeric thresholds for *Project Success Targets* (see *Evaluation Criteria*)
- Ship transport actions (which of commit / tag / deploy / hand-off applies to this project, and in what order)
- Deferred-decision tracking mechanism for the project's language / stack (hinge tests preferred; alternatives listed in invariant)
- File system layout (project folder structure, naming conventions)

A project that has not locked all *Required Project-Level Choices* cannot enter the Plan stage. A project that violates any *Immutable Invariant* at runtime must halt and surface the violation to the human; recovery requires Charter amendment, not config tweak.

**Provisional Lock (escape hatch for choices that need exploratory work).** Some Required Project-Level Choices cannot be made with confidence at project creation — per-metric numeric thresholds, plan-consolidation thresholds, and ship transport sequencing often require domain exploration that itself happens in early Plan-stage work. To prevent the workflow from deadlocking on premature commitment, the Charter permits **Provisional Locks**:

- A Required Choice may be locked **provisionally**, with an explicit *hypothesis* (the current best guess) and a *revision trigger* (the Plan-stage event at which the hypothesis will be validated or revised).
- A provisionally-locked Choice satisfies the "must be locked before Plan stage begins" gate.
- When the revision trigger fires during the Plan stage, the Choice is re-evaluated: confirmed (provisional → final), revised (new hypothesis, new revision trigger), or escalated to Charter amendment if it has become a constitutional question.
- The audit store records the provisional value, the hypothesis, the revision trigger, and every revision event. A Charter cannot ship while any Provisional Lock is still outstanding.

This preserves the discipline (all Choices are tracked, surfaced, and reviewable) while preventing the failure mode where a project must guess a Layer-2 numeric threshold without any operational data on which to ground the guess.

---

## Invariants (Never Violate)

**Single-Writer.** Exactly one model — the Coder — authors **human-facing project artifacts** (code, briefings, dispositions, Plan amendments, Charter amendments). Other specialists may emit **machine-readable records** to the audit / provenance store (raw reviewer findings packets, Verifier results, rotation logs), but never to the human-facing artifact tree. The distinction is load-bearing: the artifact tree has one voice; the audit store has many. This is enforced by the runtime, not by trust.

**Coder Model Pinning.** The Coder is pinned to a specific model identity (vendor + family + version) at project creation. Pinning persists for the project's lifetime. The pinned identity cannot be changed by config: an upgrade is a **Charter amendment**, reviewable through the full Charter Review cycle. This protects voice consistency across the project's artifact stream — the property the Single-Writer invariant exists to deliver. (Reviewer pool members may rotate models more freely, constrained only by the Adversarial Diversity floor.)

**Adversarial Diversity.** Independence is defined operationally:

- *Floor (enforced):* no reviewer may share a model family with the Coder. If the Coder is Claude Opus 4.x, no Claude variant may serve as a reviewer.
- *Default (preferred):* reviewers are drawn from different vendors than the Coder, and from at least two different vendors collectively.
- *Pool size:* at least two distinct models.

The config schema enforces the floor at project creation and rejects any rotation that would violate it. Vendor preference is a project-level default that can be relaxed when supply requires; the family floor cannot.

**Human Gates Every Transition.** No artifact advances across a *transition* without explicit human approval. The set of transitions is **exhaustive**, not exemplary; the system shall not introduce new gates outside this list, nor skip any gate in it:

- *Stage boundaries (3):* Discuss → Plan, Plan → Build, Build → Ship.
- *Within-phase major beats (3):* Briefing → Reviewer (approval to send), Reviewer findings → curation/Coder (approval, with optional edits to findings), Disposition (R-N) → next reviewer or Ship.

Six gates total per phase loop. Internal mechanical operations — the Verifier grounding a claim, audit-store writes, rotation arithmetic, summary rendering — are *not* separate gates. Their output is reviewed by the human as part of the next major beat. The system runs internal mechanics freely; it never auto-advances a transition.

If a project type genuinely needs additional within-phase beats (e.g., a complex Build phase with sub-deliverables: "draft complete," "tests added," "ready for review"), those sub-beat definitions are a **Plan-stage decision** and must be enumerated in the Plan rather than emerging implicitly. The Charter fixes the framework; the Plan parameterizes it.

**Findings Are Advisory.** Reviewer findings flow to the human first, who curates them (keep / drop / edit / annotate) before they reach the Coder. The reviewer is never authoritative over the codebase. The human's curation step is non-optional.

**Findings Are Grounded.** No finding drives a Coder action until the Finding Verifier has confirmed its anchored evidence (file:line, symbol, claim shape) holds against the current code state. Findings whose evidence cannot be grounded — typically semantic claims without concrete anchors — are surfaced to the human as "cannot be verified" for explicit disposition rather than silently followed or silently discarded. Verifier scope is strictly evidentiary; recommendation worthiness remains the human's call.

**Living Plan.** The Plan absorbs converged review findings as appended Hardening Notes. Findings and decisions do not live in a separate stream that drifts from the Plan. The Plan is always the current truth.

**Plan Consolidation.** Hardening Notes accumulate by design but do not accumulate forever. At points designated in the Plan stage (phase boundaries, version cuts, or when notes exceed a configured threshold), the Plan undergoes *consolidation*: prior Hardening Notes are absorbed into the main body, the Plan version is bumped, and provenance is preserved via the artifact graph (the pre-consolidation version remains queryable but is no longer the canonical reference). The Coder performs consolidation as a rendering operation; consolidation itself is reviewable like any other Plan change.

**Deferred Decisions Are Tracked.** Decisions deliberately postponed during planning or review must be encoded into the artifact graph as first-class objects, not as TODO comments or chat history. The system tracks open deferred decisions, surfaces their count, and treats each resolution as a deliberate decision moment.

The **preferred mechanism** is *hinge tests* — assertions that pin the current state and require deliberate flipping to migrate, so deferred work is exercised on every test run rather than silently rotting. Where the language or stack does not support hinge tests cleanly (projects without a test harness, declarative configuration, prose artifacts), alternative mechanisms — flagged registry entries, dedicated deferral docs, calendar-bound reminders — may be substituted, *provided* the tracking invariant holds: the deferral is a queryable artifact, not a comment.

**Provenance Graph.** Every artifact is a node with versioned references to its inputs. The Charter references the discussion. The Plan references the Charter. Each phase's briefing references the Plan. Each disposition references the briefing and the prior reviewer findings. The graph is queryable.

**Cross-Reference Integrity.** Every human-facing decision recorded in a project artifact (a disposition entry, a Charter amendment, a Plan hardening note) must be backed by a corresponding machine-readable record in the audit / provenance store. The Coder's prose summarizes; the audit record proves. A decision present in prose but absent from the audit store is a Cross-Reference Integrity violation and is surfaced as such — the artifact is not considered shipped until the cross-reference holds in both directions. This invariant prevents the two stores from drifting apart, which is the failure mode the Single-Writer / Audit Trail pair would otherwise be exposed to.

**Audit-Store Minimum Schema.** The audit / provenance store must provide:

- **Required record types** (minimum set; Plan may extend): reviewer-finding-packet, verifier-result, rotation-log, charter-amendment, plan-amendment, phase-disposition, hinge-flip, gate-approval, convergence-declaration, provisional-lock, rollback-event.
- **Stable cross-reference key.** Every record carries a key linking it to the human-facing artifact it backs (Charter section ID, Plan phase ID, disposition entry ID, etc.). The key is stable across renderings of the human-facing artifact.
- **Append-only operational definition.** Existing records are not modified or deleted at the application layer; corrections are added as superseding records that reference the original by key. Bit-for-bit storage immutability is not required; logical append-only (no in-application mutation paths) is.
- **Creation timing.** A backing record must be written *before* the corresponding human-facing artifact section is considered shipped. Shipping a section without its backing record is a Cross-Reference Integrity violation.
- **Ship-block vs warn behavior.** Missing backing records BLOCK Ship for the affected artifact section. Cross-reference inconsistencies in non-shipped artifacts WARN (surfaced to the human at the next gate, not gated automatically).

Storage format, indexing, and the runtime mechanism that performs the bidirectional check are Plan-level. The Charter commits only to the abstract schema above. Without this minimum, both *Findings Are Grounded* and *Cross-Reference Integrity* are unenforceable.

**Reviewer Audit Trail.** Raw reviewer findings packets are logged to the audit store under the `reviewer-finding-packet` record type even when they are superseded by the Coder's disposition rendering. The Coder's prose is canonical for project use; the raw findings are canonical for audit. The audit store is a machine-readable record, not a human-facing artifact, and therefore does not violate Single-Writer.

**Rollback / Re-Open.** Any shipped phase may be re-opened. Re-opening is a Charter amendment (or, for Plan-level phases, a Plan amendment) — reviewable through the appropriate review cycle, not a unilateral coordinator action.

**Cascading Invalidation (blast radius).** Re-opening invalidates dependent phases through the **transitive closure** of the dependency graph — not just direct dependents. If phase A is re-opened, every phase whose declared dependency chain includes A (directly or via any path through other phases) is invalidated and must be re-shipped against the new state. Concretely: if A → B → C (B depends on A, C depends on B), re-opening A invalidates both B and C, not just B. The Coordinator is shown the full blast radius at re-open time and approves it before the re-opening commits.

The artifact graph supports this: the re-opened phase becomes a new version-bumped node, every dependent phase in the blast radius is marked invalidated and references the new version when re-shipped, and the prior graph state remains queryable for audit. Re-opening cannot violate any other invariant — e.g., it cannot retroactively rewrite the audit store; it adds new records that reference the prior state.

**Artifact Encoding.** All human-facing project artifacts are encoded in UTF-8. Non-ASCII characters (em-dashes, typographic quotes, mathematical symbols) are *permitted and intentional* where they aid readability; they are not corruption.

The Plan stage specifies two layered protections:

- A **lint check at the audit-store boundary** that flags any byte sequence that is not valid UTF-8, and surfaces any non-ASCII characters for explicit human acknowledgement (intentional typography vs. accidental paste from a non-UTF-8 source).
- A **Pre-Flight Environment Check** that runs before any tool (Coder, reviewer, verifier, viewer) interacts with project artifacts, validating that the tool's runtime is configured for UTF-8 (terminal locale, editor encoding, model-prompt-handling). The check protects against reviewer environments rendering valid UTF-8 as garbled output and then mistakenly flagging the artifact itself as corrupted. The pre-flight check is operationally distinct from the lint: lint defends the artifact; pre-flight defends the tool's view of the artifact.

Both protections are Plan-stage implementations of the Charter-level invariant.

---

## Scope Boundaries

- **Single-user.** One human coordinator. Team workflows are out of scope for v1.
- **Project-scoped, single-active.** Each project has its own pinned Coder, its own reviewer pool, its own Charter / Plan / artifact graph. The storage layer may hold multiple projects concurrently, but exactly one project is *active* at a time — no cross-project state sharing, no parallel project execution in v1. Project switching is an explicit user action.
- **Model-agnostic.** No hard dependency on any vendor. The config selects models per specialist slot. The diversity invariant is enforced regardless of which vendors are available.
- **Local-first.** Artifacts live on the user's disk. Project structure mirrors something like the AiMe `IP/` convention (plans, review_rounds, completed). No cloud state is required.
- **Coding-focused.** This is a coding workbench, not a general-purpose work agent. The specialist contracts assume code as the deliverable.

---

## Charter-Level Acceptance Criteria

The Charter is satisfied — and the project is ready to leave the Discuss stage — when:

1. The workflow shape (Discuss → Plan → Build → Ship) is locked.
2. The single-writer invariant is locked.
3. The adversarial-diversity invariant is locked.
4. The specialist roster is enumerated with role and writer status.
5. Human authority at every gate is explicit and non-optional.
6. The artifact graph and provenance commitment is stated.
7. The hinge-test pattern is recognized as first-class.
8. The Living Plan pattern is recognized as the merge mode for review findings.
9. Open items are named and deferred to the Plan stage rather than silently elided.
10. The Charter has been reviewed by at least two models from vendors different from the Interlocutor / Coder, and the converged version has been approved by the human.

Items 1–9 are satisfied by this draft (post R1 consolidation). Item 10 is in progress: R1 is complete; rotation continues until the termination condition is met.

---

## Evaluation Criteria

The core thesis — that bounded, adversarially-reviewed workflows produce higher-quality output than single-loop agentic systems — is a hypothesis to be measured, not assumed. The Evaluation system has three layers; each layer has a different role and a different owner. Conflating them — putting numeric thresholds in the Charter, or product-health framing in the Plan — would either make the Charter project-specific or make the Plan untestable.

### Layer 1 — Charter-level Product Health Metrics (project-agnostic)

These metrics define what "working as designed" means for the Anvil product itself. They apply to every Anvil project; the Charter commits to tracking them and to their qualitative success indicators. Their numeric thresholds live in each Plan (Layer 2).

- **Defect escape rate.** Issues discovered after Ship that should have been caught by review. *Direction: lower is better.* Qualitative: zero P1 escapes; P2/P3 escapes trend down across phases.
- **Review finding precision.** Of findings raised by reviewers, the fraction that were grounded (confirmed by the Verifier) and led to a Coder action. *Direction: higher is better.* Qualitative: ≥70% indicates healthy reviewers.
- **Human minutes per shipped phase.** End-to-end coordinator time from phase start to phase Ship. *Direction: lower is better, with a floor.* Qualitative: trends down as the system matures; a rising trend signals workflow friction.
- **Review rounds per phase.** Number of R-N rounds before a phase converges. *Direction: lower is better, with floor of 2.* Qualitative: most phases converge in 2–3 rounds; 5+ signals incomplete briefing or under-specified plan section.
- **Cross-reviewer agreement.** Fraction of finding classes where independent reviewers concur. *Direction: bimodal (extremes are diagnostic).* Qualitative: moderate agreement (~30–60%) is healthy; near-zero or near-100% indicates reviewer collapse or echo-chamber convergence.
- **Deferred-decision resolution rate.** Of deferrals tracked (hinge tests or alternative mechanisms), the fraction eventually resolved versus left dangling. *Direction: higher is better.* Qualitative: deferrals resolve within 2–3 phases of being set; deferrals open >5 phases get re-reviewed.

### Layer 2 — Project Success Targets (Plan-level, project-specific)

Each project sets *numeric thresholds* for the Layer-1 metrics, plus any project-specific success criteria not covered by the standard six. These are committed in the Plan and become **Ship criteria**: a phase that fails its targets without explicit justification is not eligible to ship. The Layer-2 targets are part of the *Required Project-Level Choices* in *Governance Taxonomy*.

Examples of project-specific targets (not exhaustive): per-component defect budget, latency / size / cost budgets for the Coder model, language-specific quality gates, domain-specific functional acceptance tests.

### Layer 3 — Runtime Alerts (auto-fired during execution)

Some metric states are not just diagnostic; they are *active signals* that the workflow itself needs adjustment. The Plan stage specifies which alert conditions fire, and what each fires *into* — a human-visible flag, a reviewer reconfiguration suggestion, or an automatic pause for human decision.

Examples of alert kinds (Charter names the *kinds*; Plan specifies the *thresholds* and the *response*):

- *Sustained low Review-finding-precision* → reviewer reconfiguration suggested (drift or prompt rot likely).
- *Rising Human-minutes-per-phase trend* → gate UX or summary-quality investigation.
- *Cross-reviewer agreement at an extreme* → pool review suggested.
- *Hinge / deferred decision open >5 phases* → re-evaluation of whether the deferral is still justified.

### How the three layers compose

Layer 1 says **what to measure** and **what counts as healthy**. Layer 2 says **what numeric targets this project commits to**. Layer 3 says **what to do when a state fires**. The three-layer split keeps each artifact narrowly scoped: the Charter is project-agnostic, the Plan is project-specific, the runtime is execution-time.

---

## Open Items (Deferred to Plan Stage)

These are the items the Charter intentionally does not specify. Each is implementation-level: the abstract contract is locked here; the concrete implementation is set by the Plan and parameterized per project. Items previously listed here that the Charter now locks (Planner Contract minimum, Ship Abstraction, Audit-Store Minimum Schema, Coder Model Pinning, Rollback semantics) have been promoted into the body and are no longer Open Items.

- **Planner implementation specifics.** The Planner Contract minimum is locked in *The Workflow / Plan* and *Governance Taxonomy*. Open: heuristic versus learned planning, the exact data schema for the structured Planner-output packet, how the Planner handles cross-cutting concerns vs. phase-local concerns operationally.
- **Audit-store storage and runtime mechanism.** The Audit-Store Minimum Schema invariant locks record types, cross-reference key, append-only semantics, creation timing, and Ship-block vs. warn behavior. Open: storage format (filesystem with naming conventions, SQLite, JSON-on-disk), indexing strategy, the runtime mechanism that performs the bidirectional Cross-Reference Integrity check.
- **Ship transport actions.** The Ship Abstraction is locked at Charter level. Open per project: which transport actions (git commit, tag, deploy, hand-off, manual archive) constitute Ship for this project, and in what order; how transport failures are surfaced and recovered.
- **Plan-consolidation thresholds.** The Plan Consolidation invariant locks the *behavior*; default policy is phase boundary or note-count threshold. Open: numeric threshold (how many notes before consolidation), whether to add time-based or manual triggers, how consolidation is reviewed.
- **Rollback / Re-Open detailed mechanics.** The Rollback invariant locks the abstract semantics. Open: the artifact-graph mechanism for version bumps, how dependent phases are notified, how the audit store represents a re-opened phase's history.
- **Deferred-decision tracking mechanisms beyond hinge tests.** The invariant locks the tracking requirement and names hinge tests as preferred. Open: implementation of the alternatives (flagged registry, deferral docs, calendar reminders) for stacks where hinge tests don't fit.
- **File system layout.** Project folder structure, naming conventions, where Charter / Plans / Reviews / Code live. Likely modeled on the AiMe `IP/` convention but not yet specified.
- **UI / UX surface.** CLI, web app, desktop app, or some combination. The workflow is the same regardless; the UX choice affects how findings are edited, how summaries are surfaced, how approval gates are clicked.
- **Interlocutor model override criteria.** Default (same as Coder) is locked. Open: when a project should explicitly override, and how the Charter Review step compensates for same-model Interlocutor / Coder bias.
- **Per-metric numeric thresholds for Project Success Targets.** Layer-1 metrics and qualitative success are locked; Layer-2 numeric targets are Plan-stage.
- **Runtime alert response policies.** Layer-3 alert kinds are named; the specific firing thresholds and the response (flag, suggest, pause) are Plan-stage.

The discussion-stage discipline was to name these and stop, not to resolve them. The Plan stage refines each in context. After R3 promoted the must-lock-before-plan items into the Charter body, the Open Items list above is genuinely implementation-level — the items here can be settled by the Plan without re-opening Charter-level questions.

---

## Risks and Failure Modes

**Risk: the Coder becomes an information bottleneck.** Because the Coder renders all artifacts including reviewer dispositions, a future reviewer sees the prior reviewer's findings through the Coder's prose, not raw. Mitigation: the Finding Verifier sits between reviewers and the Coder, and raw findings packets are logged to the audit store.

**Risk: the human becomes the bottleneck.** Every gate is human-gated. For a long project that is a lot of decisions. Mitigation: gate UX must be optimized for speed — clean summaries, structured findings, single-gesture keep/drop/edit on findings. Summary quality is treated as load-bearing.

**Risk: the workflow gets routed around.** If users can skip the discussion stage or the planning stage, they will when in a hurry, and the system becomes Claude Code with extra steps. Mitigation: workflow is enforced, not advisory. A triage stage at the very top may decide pipeline depth (full workflow vs. fast path) but the triage decision is itself bounded — it picks lanes, it does not decide what specialists conclude.

**Risk: reviewer fatigue / agreement bias.** Reviewers may converge too quickly because none of them want to be the one blocking. Mitigation: explicit instruction to reviewers that disagreement is information, not noise. Convergence is required for approval; rapid agreement without engagement is a red flag the system should surface.

**Risk: the Coder model's blind spots dominate.** The Coder is pinned, so its biases propagate across the whole project's code style and architecture. Mitigation: the adversarial reviewer pool is specifically designed to compensate. This is the whole point of the diversity invariant. Failure of this mitigation is detectable as repeated misses on the same finding class across phases.

**Risk: model availability / vendor outage.** If the pinned Coder is down, the project halts. If a reviewer vendor is down, rotation falls back to remaining pool members and may violate diversity. Mitigation: the Plan stage should specify behavior on partial pool availability.

**Risk: artifact sprawl / retrieval overload.** A workflow whose value comes from durable artifacts accumulates artifacts quickly: Charters, Plans, phase briefings, disposition documents per round per phase, hardening notes, audit-store records. Past a threshold, operators (and reviewers) can no longer see the forest for the trees, and the workflow's intended clarity is undermined by its own paper trail. Mitigation: *Plan Consolidation* invariant absorbs Hardening Notes into the canonical body periodically; the Plan stage must specify a parallel discipline for the artifact tree (archival of completed-phase materials, indexing for retrieval, navigation aids in the UI / UX surface). Sprawl is detected via the *Human minutes per shipped phase* metric trending up.

**Risk: planner-generated phase coupling.** When a Planner decomposes work into phases, it can produce phases that are *logically separable but operationally coupled* — phase B silently depends on a design choice made inside phase A, so B cannot be reviewed independently until A ships; phase A's review can't fully evaluate A without considering B's downstream needs. The result is a pseudo-waterfall: phases appear separate on paper but block on each other in practice. Mitigation: the Planner Contract requires each phase to declare its dependencies and its acceptance criteria explicitly; the Plan-stage review should specifically check for hidden coupling (a phase B finding that points back into phase A's territory is a strong signal); cross-cutting concerns are declared at Plan level, not absorbed into individual phases.

---

## Provenance and Review Process

This Charter is itself an artifact in the system it describes. The discussion stage that produced it has been documented in the conversation thread on 2026-05-15. The next step in the workflow described above is:

1. Send this Charter to at least two reviewer models from vendors different from the Interlocutor (which was Claude). Suitable candidates: GPT-class model from OpenAI, Gemini-class model from Google.
2. Each reviewer produces a structured findings packet (id, severity, claim, evidence, recommendation) against this document.
3. The user curates findings.
4. The Coder (also Claude in this case, by single-writer invariant) renders the converged Charter with appended "Hardening Notes (R1 — Consolidated)" or sends back for further iteration.
5. Once the Charter clears review, the Plan stage begins. The Planner consumes the approved Charter and produces a Phased Plan.

---

## Questions for Charter Reviewers

Numbered, specific, anchored — same format the Phase Review Briefings use, so reviewers know what to weigh in on.

1. **Workflow stages.** Are Discuss → Plan → Build → Ship the right stages? Anything missing — for instance, an explicit Triage stage before Discuss for projects that warrant a fast path? Anything redundant?
2. **Single-writer invariant.** Is the case for one model writing every artifact (code, briefing, disposition, plan amendment) overstated? Are there roles that should be allowed to write — for instance, the Finding Verifier writing its grounded packet directly?
3. **Adversarial-diversity invariant.** Is "different vendor from Coder" the right bar, or is "different model family from Coder" sufficient (which would permit, e.g., two Claude variants)? Industry trends are blurring vendor lines; does the invariant need tightening or loosening?
4. **Human gating at every transition.** Realistic for a single user across a multi-phase project, or will it cause workflow stalls? Is there a defensible subset of intermediate gates (e.g., individual reviewer iterations within a phase) that could auto-advance once human policy has been declared once?
5. **Finding Verifier scope.** Is the verifier doing too much (every finding grounded) or too little (only finding location-anchored claims, not semantic claims)? Where does deterministic verification end and LLM judgment begin?
6. **Living Plan pattern.** Is folding review findings into the Plan as Hardening Notes the right merge mode, or does it risk the Plan becoming bloated and hard to read over time? Should there be a Plan-condensation stage?
7. **Hinge tests as first-class.** Is the system tracking of hinge tests worth the implementation cost, or is a naming convention enough?
8. **Scope of v1.** Single-user, single-project-at-a-time, local-first — too narrow? Multi-project support is a config-level extension; team support is a much larger lift. Are these boundaries right for the first version?
9. **Open items list.** Are these the right items to defer, or should any of them be locked in here? Specifically: the fix-loop termination condition was leaned on but not pinned — is that defensible at Charter level or should it be settled now?
10. **Risk inventory.** Anything material missing from the risks section? Particular attention to model-availability and reviewer-fatigue risks, which are speculative and may need refinement.

---

## Bottom Line

Anvil is the user's existing manual workflow made enforceable. The discipline is designed to produce higher-quality output than a single-loop agent — measurable against *Evaluation Criteria* — because it preserves bounded authority across stages and leverages cross-family independence at every review gate. The Charter does not claim this without measurement; it claims it as a hypothesis the system is built to validate. The COGS frame is the substrate that makes the discipline implementable as software rather than as habit.

The Charter locks the discipline. The Plan refines the implementation. The Build executes phase by phase. The reviewers gate. The human approves. The artifacts persist. The system grows a graph instead of a chat history.

Charter rotation is in progress per the locked termination condition. Round-by-round provenance — including R1 / R2 / R3+ hardening notes — lives in `CHARTER_HARDENING_HISTORY.md`. The Charter body above is the canonical normative text; the history file is the audit trail.

---

## Amendment A1 — Applied 2026-05-19

Charter Amendment A1 (Open-Source Distribution + Defined Artifact Structures + Embeddable Workflow Infrastructure) converged on 2026-05-19 after two review rounds. The amendment's normative content is incorporated below; full text and provenance live in `CHARTER_AMENDMENT_A1.md` (Draft 3, the converged state) and `AMENDMENT_A1_CONVERGENCE.md`.

### New Invariants (added to the Charter's *Invariants (Never Violate)* canon)

**Open-Source Distribution.** Anvil is distributed as open-source software under the Apache 2.0 license. The source repository becomes public at the *Publication Milestone* (private during P0–P11 implementation; flips public at v1 ship, gated by the publication-safe git history scan). Governance: Contributor Covenant 2.1, DCO with AI-assistance and third-party-snippet provenance trailers, `SECURITY.md` (threat model + vulnerability triage + supported-version policy + 90-day window), semver discipline for every public contract in the *Contract Inventory*. Forks permitted under license; Trademark Posture A locked (no registration; naming-preference statement in README). Governance mechanics in `GOVERNANCE.md` (BDFL + maintainer admission/removal + conflict-of-interest + BDFL succession + BDFL-adversarial emergency-freeze provision).

**Artifact Structures Are Defined.** All canonical workflow artifacts (Plan, Phase Review Briefing, Disposition, Findings Packet) conform to templates and schemas specified in `ARTIFACT_SPECIFICATIONS.md`. Major spec changes require full Charter amendment, not narrow review.

**Embeddable by Design.** Anvil is designed as workflow infrastructure external programs may embed. The Vault library exposes a clean command/query API with no terminal-shaped, CLI-shaped, or App-shaped assumptions. The audit-store schema, structured CLI output (with `--describe-schema` discovery + per-command JSON Schemas + `schema_version` + stable error codes), and sidecar wire protocol are public contracts under semver discipline. Six non-negotiable embedding invariants apply when the v1.2 embedded surface ships: Vault remains the trust authority; no human-gate bypass; typed API only (no screen-scraping); audit records mandatory; per-embedder authentication; multi-tenant isolation. The specific transport form is a v1.2 design choice.

### New Required Project-Level Choices (added to *Governance Taxonomy*)

Fifteen new choices, all Final. Open-source-related: Open-source license = Apache 2.0; Contribution mechanism = DCO with AI/provenance extension; Governance model = BDFL with maintainers; Code of Conduct = Contributor Covenant 2.1; Repository host = GitHub primary (`https://github.com/ai-nhancement/Anvil`) + optional Codeberg mirror; Security disclosure policy per `SECURITY.md`; Trademark posture = Posture A (no registration; re-evaluation trigger documented); Publication milestone = private through P0–P11, public at v1 ship after publication-safe-history gate clears; Dependency review and SBOM (outcome locked; default tools — CycloneDX + cargo audit + govulncheck — Plan-stage); Release signing (offline-verifiable outcome locked; GPG default Plan-stage); Secret scanning (outcome locked; gitleaks default Plan-stage). Artifact-structures: Artifact specifications = `ARTIFACT_SPECIFICATIONS.md`. Embeddable: Embeddable scope = v1 ships "keep the door open" pieces; v1.2 ships full embedded surface.

### New Charter Sections (full text in the amendment document; key commitments here)

- **Contract Inventory** — six public contracts enumerated with owner / versioning policy / migration policy: Vault library API, audit-store record schemas, artifact specification document, sidecar wire protocol, structured CLI output, machine-readable error codes. Major bumps to any of these require Charter amendment.
- **Public vs Private Audit Records** — all audit records private by default. `recommended_visibility` metadata is non-binding guidance. Public publication requires per-record explicit Coordinator approval through `anvil audit export --public` with secret scan + license scan + sensitivity labels + manual Coordinator review + cryptographic seal.
- **Publication-Safe Git History Gate** — runs before the repo flips public: full-history secret scan + full-history license scan + Coordinator commit-message review + exceptional history-rewrite allowance.
- **Embedding Invariants** — six non-negotiable rules for the v1.2 embedded surface (Vault authority, no gate bypass, typed API, audit records mandatory, per-embedder auth, multi-tenant isolation).
- **Structured CLI Output Stability** — per-command JSON Schemas in `schemas/cli/`, `schema_version` in every output, stable error codes, `--describe-schema` flag mandatory, compatibility test suite in CI.
- **Repo-Readiness Acceptance Gates** — twelve concrete deliverables (LICENSE, NOTICE, README, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, GOVERNANCE, optional TRADEMARK, CI policy checks, release signing workflow, SBOM generation, public-safe audit bundle self-validation) the publication milestone cannot fire without.
- **Per-Item Disposition Mechanism + Cross-Document Convergence** — review-process rules for future composed amendments and cross-document concurrent reviews.

### Audit-store record types updated

The Charter's *Audit-Store Minimum Schema* invariant lists eleven required types and explicitly permits Plan extensions. With Amendment A1 applied, v1's audit store now includes sixteen record types in total: the original 11 + 5 Plan extensions (`ArbiterFindingResolution`, `SidecarReload`, `PublicVisibilityPolicy`, `PublicExportApproval`, `EmergencyFreezeDeclaration`). The constitutional hinge `test_audit_store_required_types_present` is unchanged (subset check on the required 11). Plan Draft 7 reconciles per-implementation counts and hinges accordingly.

### Downstream

Plan Draft 7 is the required next workstream: reconcile record-type counts from 13 to 16; produce the impact matrix promised in the amendment; integrate the publication-safe-history gate into P11 acceptance; add the public-export bundle to P2; add `--describe-schema` to P5–P10a; update P0 to include repo-readiness gate work. `ARTIFACT_SPECIFICATIONS.md` continues its own R1 review as a separate workstream.

---

## Provenance and Linked Artifacts

- **Hardening history:** `CHARTER_HARDENING_HISTORY.md`
- **Round disposition documents:** `REVIEW_CHARTER_R<N>.md` (one per Charter review round)
- **Reviewer raw findings packets:** logged to the audit store under record type `reviewer-finding-packet` (audit store schema is locked at Charter level; storage layer is Plan-stage)

The Charter body and the hardening history are paired: amendments visible in the Charter body must trace to a hardening-note entry in the history file (and to backing audit-store records, per *Cross-Reference Integrity*). The Charter is the constitution; the history is the legislative record; the audit store is the receipts.

