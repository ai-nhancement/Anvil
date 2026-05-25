# Anvil — Implementation Plan v1

**Date:** 2026-05-15
**Status:** **Approved** — converged on 2026-05-19 via human-arbiter declaration on the post-R5 state. R1–R5 rounds closed; full hardening trail in `PLAN_HARDENING_HISTORY.md`; convergence record in `PLAN_CONVERGENCE.md`. P0 (Bootstrap) is unblocked.

**Amendment A1 Reconciliation (2026-05-19, post-Plan-convergence).** Charter Amendment A1 (Open-Source Distribution + Defined Artifact Structures + Embeddable Workflow Infrastructure) converged after this Plan was approved. Rather than producing a Plan Draft 7 reconciliation, the Coordinator's decision (2026-05-19) is that **the Coder reads both this Plan AND the Charter's *Amendment A1 — Applied* section when building each phase, absorbing the amendment's downstream impact during the actual build rather than as a pre-build reconciliation pass**. See the *Amendment A1 — Build-Time Reconciliation* section immediately below for the per-phase impact summary. Phase-specific hinges and acceptance refinements introduced by the amendment are added during build, not pre-specified in this Plan. The Plan's pre-R5-converged content remains canonical for everything *not* affected by the amendment; the amendment-affected items in the Charter take precedence where they apply.

---

## Amendment A1 — Build-Time Reconciliation

This Plan converged at R5 on 2026-05-19 (morning). Charter Amendment A1 converged on 2026-05-19 (evening) — after this Plan. The Coordinator chose at convergence to not produce a Plan Draft 7 rewrite; instead, the Coder reads the Charter's amendment-applied content during build and integrates it phase-by-phase. This section is the navigation aid for that integration.

### Impact by phase

- **P0 (Bootstrap).** Add repo-readiness gate deliverables: `LICENSE` (Apache 2.0), `NOTICE`, `CONTRIBUTING.md` (with DCO + AI-assistance + third-party-snippet provenance trailers per the Charter's *DCO Extension*), `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1), `SECURITY.md` (per the Charter's *Security Posture* commitment — threat model, vulnerability triage roles, supported-version policy, coordinated-disclosure workflow), `GOVERNANCE.md` (per the Charter's *Governance Mechanics* — BDFL, maintainer admission/removal/conflict-of-interest, BDFL succession + adversarial emergency-freeze), CI configuration with DCO check, secret-scanning hook (`gitleaks` default per Plan-stage tooling), dependency-vulnerability scanning (`cargo audit` Rust + `govulncheck` Go), SBOM generation step (CycloneDX default), release-signing workflow (GPG-signed `SHA256SUMS.txt.asc`). The repo is created **private** at P0 start and stays private through P0–P11 implementation per the Charter's *Publication Milestone*.
- **P1 (Config and Charter Loader).** No direct amendment additions; existing P1 work covers what's needed. New Required Choices from the amendment (license, contribution mechanism, governance model, Code of Conduct, repo host, security disclosure policy, trademark posture, publication milestone, dependency review + SBOM, release signing, secret scanning) are now part of the Required-Choices schema; update the schema to include them.
- **P2 (Audit Store + Provenance Graph).** Add three new audit-record types beyond Plan's pre-amendment 13: `PublicVisibilityPolicy`, `PublicExportApproval`, `EmergencyFreezeDeclaration`. Total v1 record types: **16**. Constitutional subset hinge `test_audit_store_required_types_present` is unchanged. Add the **`anvil audit export --public`** command implementing the public-safe bundle process (default-deny per record, secret scan, license scan, sensitivity-label respect, Coordinator manual review gate, cryptographic seal). The local-private vs public-project record distinction is core P2 work.
- **P3a (Contract Definition).** No direct additions beyond what was already in place (config-epoch from R4, provider_connection_id from prior provider-flexibility work). Note that the embedding-invariants from the Charter (no-bypass, Vault-authority, etc.) are v1.2 commitments; v1 P3a does not change.
- **P3b (Rust Sidecar Client) / P3c (Go Sidecar).** No direct additions beyond the prior R4 work.
- **P4 (Interactive CLI Setup Wizard).** Add wizard prompts for the new Required Choices not already covered (governance model selection, trademark posture confirmation — defaults to Posture A per Coordinator lock, security disclosure contact details). Wizard's existing 7-step structure stands; new prompts integrated into existing steps or as sub-steps where natural.
- **P5 (Charter Stage Pipeline).** No direct additions. The disposition document template the Coder renders is per the Artifact Specifications spec (still in R1 review); minor template adjustments may land when that spec converges.
- **P6 (Multi-Reviewer Rotation + Convergence Safeguards).** No direct additions. The `EmergencyFreezeDeclaration` audit record type is referenced from `GOVERNANCE.md` content; P6's machinery doesn't need to know about it specifically (it's a governance event, not a workflow gate).
- **P7 (Plan Stage Pipeline).** No direct additions. Plan template per the Artifact Specifications spec.
- **P8 (Build Stage Pipeline).** Add `--describe-schema` flag support to the CLI's structured-output mode (per the Charter's *Structured CLI Output Stability*). Every command emitting `--format json` must support `--describe-schema` returning the embedded JSON Schema for that command. Schemas live in `schemas/cli/*.json` and are embedded into the binary at build time. Compatibility test suite in CI checks against prior major version's schemas.
- **P9 (Ship + Rollback).** No direct additions.
- **P10a (Evaluation Criteria Infrastructure).** No direct additions; metric instrumentation framework already covers what's needed.
- **P10b (Hinge-Test Framework).** No direct additions; bi-language registry and consensus check are unchanged.
- **P11 (Dogfooding and Documentation).** Add the **Publication-Safe Git History Gate** as a P11 acceptance criterion: before the repo flips public (per Charter's *Publication Milestone*), full-history secret scan must pass (zero unresolved hits OR every hit has a Coordinator-acknowledged audit record), full-history license scan must pass (zero incompatible-license files OR every flag has a resolution audit record), Coordinator commit-message review pass must be complete, OR an exceptional pre-publication history rewrite must have been performed (with the rewrite itself an audit record). Add scripted smoke tests for the publication-safe gate to the existing P11 smoke-test list. The pilot rubric's provider-diversity requirement is unchanged.

### Cross-cutting

- **DCO Extension** (`AI-Assisted-By:` and `Derived-From:` trailers): CI check lands as part of P0's CI configuration. The "substantive code paths" definition (≥20 lines new code, or new file, or code in `crates/anvil-core/` `crates/anvil-audit/` `crates/anvil-graph/` `crates/anvil-sidecar-client/` or `sidecar/internal/`) and "persistently" definition (warning on first PR, blocking on subsequent updates) are operational thresholds; tunable in Plan-stage refinement.
- **Cross-Document Convergence rules** govern future amendments and concurrent multi-document reviews. Build phases do not implement these rules directly; they apply when the Plan itself or the Artifact Specifications spec is amended.

### Plan documents needed during build

The Coder consulting this Plan during a Build phase must also have on hand:
- `new_project_charter.md` (with the *Amendment A1 — Applied* section).
- `CHARTER_AMENDMENT_A1.md` (for full text of supporting sections — Contract Inventory, Public/Private Audit Records, Embedding Invariants, etc. — that are referenced from the Charter but not duplicated in full).
- `ARTIFACT_SPECIFICATIONS.md` (still in R1 review; its R1 convergence may produce small template adjustments).
- This Plan.

If the Coder encounters a conflict between this Plan and the Charter's amendment-applied content, the Charter wins (constitutional precedence). The conflict itself is logged as a finding for the next Plan amendment cycle.

---
**Charter version consumed:** `new_project_charter.md` post-R4 (declared convergent on 2026-05-15; see `CHARTER_CONVERGENCE.md`)
**Authoring model:** Claude (Planner specialist; Coder rendering)
**Planner-Contract compliance:** This Plan satisfies the Charter's *Planner Contract* invariant. See *Planner Contract Compliance* for the field-by-field mapping.

---

## Executive Summary

Anvil v1 is a **Rust CLI** with a Go sidecar for model-provider integration, communicating over **gRPC with versioned protobuf schemas**. The CLI is the v1 deliverable; a desktop App (Tauri + React + TypeScript) is scoped as v1.1 and will be informed by usage feedback from v1.

**Why CLI-first.** Anvil's value proposition is *structure for vibe coding* — review gates, provenance, adversarial cross-vendor diversity, and explicit workflow discipline replacing the unstructured agent-loop approach that produces unreliable output. The CLI surface is the right v1 form because Anvil's gate-heavy workflow — six human-approval gates per phase, structured briefings, finding-by-finding curation, disposition rendering — maps naturally to a terminal interface where each gate is a prompt and each artifact is a file. The audience most likely to surface real issues with that discipline is experienced developers using CLI surfaces in real workflows. The App in v1.1 will be designed against this evidence rather than against guesses.

**Architectural commitment that survives v1 → v1.1.** Even though v1 ships only the CLI, the Vault is designed as a clean Rust library (`anvil-core`) with a command/query API that an App can consume directly in v1.1 without rework. File-system locking is in place from v1 so the App can coexist with the CLI from day one of v1.1. Several Plan-stage decisions in v1 are *Provisionally Locked* with `revision trigger = v1.1 App design begins`, so the App design can refine them based on real CLI usage data. The v1 decisions made explicitly for App compatibility are named in *App-Compatibility Design Decisions*.

**The architectural shape:**

```
┌──────────────────────────────────────┐
│  CLI (anvil)                         │
│  - Subcommand dispatch               │
│  - Interactive setup wizard          │
│  - Workflow gates surface as prompts │
└──────────────┬───────────────────────┘
               │ links
               ▼
┌──────────────────────────────────────┐
│  Vault (anvil-core, Rust library)    │
│  - State machine                     │
│  - Audit store                       │
│  - Provenance graph                  │
│  - Policy enforcement                │
│  - Designed for v1.1 App consumption │
└──────────────┬───────────────────────┘
               │ gRPC (versioned)
               ▼
┌──────────────────────────────────────┐
│  Sidecar (anvil-sidecar, Go)         │
│  - Vendor adapters (HTTP)            │
│  - Error classification              │
│  - Stateless across invocations      │
│  - Workspace-scoped daemon           │
└──────────────────────────────────────┘
              │
              ▼
   ┌──────────────────────┐
   │  Vendor APIs         │
   │  Anthropic / OpenAI  │
   │  / Google            │
   └──────────────────────┘
```

The Plan decomposes the v1 build into **fifteen phases** (P0, P1, P2, P3a, P3b, P3c, P4, P5, P6, P7, P8, P9, P10a, P10b, P11). Critical path is P0 → P1 → P2 → P3a → (P3b ∥ P3c) → P4 → P5 → P6 → P7 → P8 → P11. P9 (Ship + Rollback), P10a (Evaluation Infrastructure), and P10b (Hinge-Test Framework) are parallel branches after P8; P11 (Dogfooding) requires all three.

The deliverable acceptance test has two components: (1) **dogfooding** — Anvil v1 can manage the Anvil v1.1 design (Charter through Plan) without manual orchestration; and (2) **external pilot** — at least one full Charter→Plan→Build→Ship cycle on a small, non-self-referential project. Both are required for Plan-level acceptance. See *Plan-Level Acceptance Criteria*.

---

## Project Context

This Plan is itself an artifact in the system it describes. Phases 0–10 build the Anvil v1 CLI; the dogfooding test in P11 uses the built v1 CLI to manage planning for Anvil v1.1.

Three trust-boundary rules already govern the v1 architecture and are implemented in the Build phases. They are locked at Plan level now — not merely aspirational — because the phase implementations depend on them as settled constraints. Charter amendments to promote them to the constitutional layer are scheduled after Plan convergence. The locked definitions are in *Plan-Level Trust-Boundary Invariants*; the promotion list is in *Post-Convergence Charter Amendments*.

---

## Plan-Level Trust-Boundary Invariants

These three rules are hard constraints on every phase that interacts with the sidecar or the App surface. They are not advisory. Charter amendment to promote each to the constitutional layer is scheduled after Plan convergence.

**1. No commit on partial or invalid sidecar output.** The Vault never commits a phase artifact, advances a gate, or records a disposition based on partial or invalid sidecar output. The boundary is precise:

- *Ephemeral display:* `InvokeStreaming` may emit `Token` events that are shown live in the terminal (or, in v1.1+, the App) as the model produces them. This is purely a UX affordance.
- *Authoritative commit:* only the `FinalResult` event carries a result that the Vault may commit (audit-store record, phase artifact, disposition entry). The Vault's commit path consumes `FinalResult` exclusively; it ignores `Token` events for commit purposes.
- *Mid-stream error:* on any `Error` event during a streaming invocation, the Vault discards all accumulated tokens from the commit path, does not write a result record, and surfaces only the typed error to the caller. Tokens already displayed in the terminal remain visible as artifacts of the failed call (the terminal cannot un-print); audit-store and artifact-tree state are unaffected.

There is no best-effort commit. Enforced in `anvil-sidecar-client` (P3b) and documented as a contract invariant in the proto schema (P3a). Guarded by hinge test `test_partial_output_discarded_on_streaming_error`.

**2. Sidecar must remain stateless across invocations.** The sidecar holds no persistent state between RPC calls. All session context, conversation history, and stateful reasoning live in the Vault. API keys and other secret credentials are passed per-call via the `Credentials` field of `InvokeRequest`, consumed by the request handler, and discarded — never cached, never logged, never persisted on the sidecar side. (Non-secret provider-connection metadata such as endpoint URL and region *is* loaded at sidecar startup from `--provider-config`; this is routing data, not secret material.) The workspace-scoped daemon (see *Sidecar lifecycle* in the Locked Choices table) may live across multiple CLI invocations but holds no application-layer state between calls.

**3. App frontend is not on the trust boundary; Vault enforces all invariants regardless of frontend input.** When the v1.1 App is added, it is a UI surface consuming the Vault API, not a trust-bearing layer. The Vault validates all inputs regardless of whether they originate from the CLI, the App, or any future surface. Locked now so v1.1 does not inadvertently re-architect around it.

---

## Product Positioning

This section is positioning context that informs implementation decisions throughout the Plan. It is not marketing copy; it is the product thesis distilled into a form the Coder can reference when making design trade-offs.

### The wedge

Anvil's core positioning is *structure for vibe coding*, but the wedge is sharper than that one phrase. The criticism the developer community levels at vibe coding — the dismissal of "AI slop," the complaints about fragile codebases, the "looks good on the surface but fractured underneath" pushback — is not anti-AI. It is anti-fragility. The same critique applies, in slightly different language, to many human-written codebases where review discipline has eroded: PRs rubber-stamped, design docs drifting from implementation, post-mortems skipped, reviews conducted by the same team whose blind spots produced the code.

Vibe coding is a particularly visible instance of an older problem. Anvil targets the older problem. The pitch is not "the AI coding tool that hides its failure modes better"; it is "the workflow that prevents them."

### The competitive axis

Anvil is not competing with Codex, Claude Code, Cursor, or any other AI coding agent on the axis of "better AI coding agent." That is a losing race against the model providers who will keep iterating their own surfaces.

Anvil competes on a different axis: *AI coding done correctly*. The competitive question reframes from "which AI tool is best?" to "which workflow produces shippable code?" On that axis, Anvil's competitors are not other AI tools — they are the absence of process.

### The defensible quality claim

A defensible claim, worth being precise about: Anvil-using developers can end up *more structurally disciplined than the median professional codebase*, not just more disciplined than vibe coding. The structural enforcement mechanisms — convergent review as a ship gate, cross-family independence as a config invariant, audit-store provenance as a runtime requirement, the Plan as canonical truth — exceed the discipline of many real-world shops where review is nominally required but practically rubber-stamped.

The honest caveat: Anvil will not out-structure the best-disciplined shops. The competitive opportunity is against the median, not against the best. That is where the audience is.

### The audience reach

The pitch — *structure for vibe coding* — is the wedge, but the audience is broader. The product reaches:

- Vibe coders who want their output to be reliable.
- AI-assisted developers in shops where review discipline has eroded.
- Teams who feel the speed-versus-rigor trade-off and want to collapse it.
- Individual developers who recognize the failure modes Anvil prevents without wanting to identify as vibe coders.

Most developers do not want to identify as vibe coders; they want to identify as people who ship reliable software. Anvil's positioning should let them be both, without forcing the label.

### Implementation implications

- **Review surface is core, not optional.** The CLI's review-related commands (`anvil charter review`, `anvil phase review`, `anvil charter findings`, etc.) are the primary value-delivery surface for v1. They get the most polish, the clearest output formatting, the best error messages. Other commands can be terse; review commands must be excellent.
- **Provenance is visible, not hidden.** `anvil audit list` and `anvil audit show` are first-class commands, not debug tools. The user should be able to see why every decision was made; that visibility is part of the product.
- **Documentation framing leads with workflow, not AI.** The `runbook.md` and `onboarding.md` documents (P11) should frame Anvil as a workflow tool that happens to integrate AI specialists, not as an AI tool that happens to have a workflow.
- **The "tired of fragile code" framing reaches further than "for vibe coders."** Onboarding language should lead with the failure modes Anvil prevents, not with the audience Anvil names.

### What this positioning is *not*

It is not a claim that Anvil eliminates the need for skilled developers. The workflow makes review discipline cheap to honor; it does not invent the skill required to write good code or judge architectural trade-offs. The Coordinator (the human) remains the load-bearing actor in every workflow stage.

It is not a claim that all AI-coding fragility is solved. Vendor-API quirks, model-update churn, prompt drift, and the underlying probabilistic nature of LLM output are real and persistent. Anvil mitigates these through cross-family independence and audit-trail provenance — not through magic.

It is not a marketing document. It is a positioning anchor. Marketing prose will be authored separately, against this anchor, when v1 approaches Ship.

---

## Locked Required Project-Level Choices

| Choice | Lock Type | Value | Provisional revision trigger |
|---|---|---|---|
| Coder model + version | Final | Claude (current production version) | n/a |
| Reviewer pool | Final | At minimum two distinct model families different from Coder (Claude). v1 minimum: Codex-class (e.g., GPT-5) + Gemini-class. **Each reviewer model is accessed via a configurable *provider connection*** — direct vendor API (Anthropic, OpenAI, Google, xAI), cloud-hosted equivalent (Azure OpenAI, AWS Bedrock, Google Vertex AI), or other gateway (DigitalOcean Gradient, OpenRouter, etc.). The family-floor invariant operates on **model identity**, not the access path: Claude via Anthropic-direct and Claude via AWS Bedrock are both *Claude family* and cannot both serve as Coder + reviewer. | n/a |
| Termination Condition | Final | Full-pool clean (default) | n/a |
| Convergence round limit | Final | 5 rounds before severity-tiering activates | n/a |
| Interlocutor model | Final | Claude (Charter default — same as Coder) | n/a |
| Planner model | Final | Claude (Charter default — same as Coder) | n/a |
| v1 deliverable form | Final | **CLI (primary v1 surface).** App is scoped to v1.1; v1 architecture is designed to make the v1.1 App addition non-disruptive (Vault as library, file-system locking in v1, IPC-ready command API). | n/a |
| Implementation language | Final | **Rust core (Vault) + Go sidecar.** Rust ≥1.80, Go ≥1.22. v1.1 will add Tauri + React + TypeScript for the App; v1 does not include any frontend tooling. | n/a |
| **Sidecar lifecycle** | **Final** | **Workspace-scoped daemon, CLI-managed.** The `anvil` CLI spawns `anvil-sidecar` as a background process on first invocation that requires model access. The sidecar writes its PID and bound port to `.anvil/run/sidecar.pid` and `.anvil/run/sidecar.port`. Subsequent CLI invocations read those files, probe the `Health` RPC, and restart the daemon if it is not responding. The sidecar auto-exits after a configurable idle timeout (default: 30 minutes; `sidecar.idle_timeout_secs` in `anvil.toml`). Logs go to `.anvil/logs/sidecar.log`; `--verbose` mode passes sidecar stderr to the terminal. Port is a random available loopback port. Binary location: `$PATH` or `sidecar.binary_path` in `anvil.toml`. `anvil sidecar status` and `anvil sidecar stop` provide explicit management. The spawn logic lives in `anvil-core` (not in CLI-specific code) so the v1.1 App can reuse it without rework. | n/a |
| Plan Consolidation triggers | Provisional | Phase boundary trigger | End of P7 (first Build-stage phase) |
| Per-metric numeric thresholds | Provisional | See *Evaluation Metric Targets* | First three Build phases ship and produce baseline |
| File system layout | Provisional | See *File System Layout* sub-section below | P0 scaffolds the actual tree; revisit if layout proves awkward |
| Deferred-decision tracking mechanism | Provisional | Hinge tests via `cargo test` (Rust) and `go test` (Go); P10 unifies collection | P10 stands up the registry |
| Ship transport actions | Provisional | For Anvil's own dev: `git commit`. For user projects: configurable. | P9 (Ship + Rollback) |
| Runtime alert response policies | Provisional | Alerts surface to CLI as warnings in v1 | P10 (Evaluation infrastructure) |
| CLI Setup Wizard step ordering and prompts | **Provisional (v1.1 prep)** | Seven-step interactive wizard, invoked via `anvil setup` (distinct from `anvil init`; see P4 and *App-Compatibility Design Decisions*). | v1.1 App design begins; validate against v1 usage feedback before App wizard is implemented |
| CLI command structure | **Provisional (v1.1 prep)** | Verb-resource pattern (`anvil <resource> <verb>`); commands enumerated in P5–P9 | v1.1 App design begins; validate that command structure maps cleanly to App view structure |

**Provisional Locks outstanding at Plan-Review time: 7.** Two carry "v1.1 App design begins" as their revision trigger.

### File System Layout (provisional)

**Anvil source repo layout (`C:\Anvil\`):**

```
C:\Anvil\
├── new_project_charter.md            # The Charter (constitutional)
├── CHARTER_HARDENING_HISTORY.md
├── CHARTER_CONVERGENCE.md
├── REVIEW_CHARTER_R<N>.md
├── ANVIL_PLAN.md                     # This file
├── PLAN_HARDENING_HISTORY.md         # Created when Plan Review begins
├── REVIEW_PLAN_R<N>.md
├── Cargo.toml                        # Rust workspace manifest
├── Cargo.lock
├── rust-toolchain.toml
├── justfile                          # Cross-language build orchestration
├── README.md
├── crates/                           # Rust workspace members
│   ├── anvil-cli/                    # CLI binary
│   ├── anvil-core/                   # Vault library — designed to be App-consumable in v1.1
│   ├── anvil-audit/                  # Filesystem audit store
│   ├── anvil-graph/                  # Provenance + dependency graph
│   ├── anvil-sidecar-client/         # gRPC client to Go sidecar
│   ├── anvil-eval/                   # Metrics + alerts (P10)
│   ├── anvil-hinge/                  # Hinge-test framework (P10)
│   └── anvil-ship/                   # Transport + rollback (P9)
├── proto/                            # Versioned protobuf contracts
│   ├── anvil/v1/sidecar.proto
│   └── README.md                     # Schema-versioning policy
├── sidecar/                          # Go module: model-adapter sidecar
│   ├── go.mod
│   ├── go.sum
│   ├── cmd/anvil-sidecar/main.go
│   └── internal/
│       ├── adapters/                 # anthropic/, openai/, google/
│       ├── contract/                 # Generated protobuf code
│       ├── errors/
│       └── server/
├── tests/
│   ├── hinge/                        # Top-level Rust hinge tests
│   └── integration/                  # End-to-end across CLI + sidecar
└── docs/
    ├── runbook.md
    ├── onboarding.md
    └── contract.md                   # Sidecar contract documentation
```

No `app/`, no `crates/anvil-app/` in v1. Those land in v1.1.

**Per-project layout (when a user runs `anvil init` from CLI):**

```
<project-root>/
├── anvil.toml                        # Project config (TOML)
├── charter.md                        # Project Charter
├── plan.md                           # Project Plan
├── CHARTER_HARDENING_HISTORY.md
├── PLAN_HARDENING_HISTORY.md
├── REVIEW_CHARTER_R<N>.md
├── REVIEW_PLAN_R<N>.md
├── phases/
│   └── <phase-id>/
│       ├── briefing.md
│       └── REVIEW_PHASE_<id>_R<N>.md
├── audit-store/
│   ├── _index.json
│   ├── reviewer-finding-packet/
│   ├── verifier-result/
│   ├── rotation-log/
│   ├── charter-amendment/
│   ├── plan-amendment/
│   ├── phase-disposition/
│   ├── hinge-flip/
│   ├── gate-approval/
│   ├── convergence-declaration/
│   ├── provisional-lock/
│   └── rollback-event/
└── .anvil/
    ├── run/
    │   ├── sidecar.pid
    │   └── sidecar.port
    └── logs/
        └── sidecar.log
```

---

## App-Compatibility Design Decisions

Several v1 decisions are made with explicit awareness that the v1.1 App will consume the same infrastructure. These decisions are not purely CLI-derived choices; they are informed by the constraint that the App must be addable in v1.1 without rework. Naming them explicitly prevents them from being mistaken for arbitrary CLI-shaped decisions when v1.1 design begins.

| Decision | v1 Rationale | App Compatibility Reason |
|---|---|---|
| Vault as library (`anvil-core`) with command/query API | Clean separation of business logic from CLI dispatch | App will be a second consumer of the same Vault API; no translation layer needed |
| No CLI-shaped assumptions in Vault API | Vault is not a subprocess of the CLI; CLI is one consumer | App is the second consumer; an API with terminal-specific assumptions would need wrapping |
| File-system locking from v1 | Prevents corruption from concurrent CLI processes | App + CLI can coexist without rework once App launches |
| Sidecar spawn logic in `anvil-core`, not in CLI-specific code | Reduces duplication within v1 | App reuses the same spawn logic; no second implementation needed |
| `.anvil/run/` directory for sidecar PID + port files | Works for CLI-managed daemon | App can locate the running daemon by reading the same files |
| TOML config at project root (`anvil.toml`) | Human-readable, version-controllable config | App reads the same config; no translation or migration layer |
| Per-project directory layout finalized in v1 | Required for audit store | App uses the same layout; no project migration needed at v1.1 |
| Two Provisional Locks carry `revision trigger = v1.1 App design begins` | Forces deliberate re-evaluation | Ensures the App design can reshape CLI patterns that don't map cleanly to a GUI |

---

## Planner Contract Compliance

The Charter's *Planner Contract* specifies required per-phase fields and required top-level outputs. This section maps each requirement to where it is satisfied in this Plan.

**Required top-level outputs:**

| Output | Where satisfied |
|---|---|
| Phased Plan document | This document |
| Phase dependency graph (explicit DAG) | *Phase Dependency Graph* section |
| Cross-cutting concerns mapped to phases | *Cross-Cutting Concerns* section |
| Acceptance criteria per phase | Per-phase *Acceptance criteria* fields (all inlined in Draft 5; see resolution of Finding 4 in `REVIEW_PLAN_R1.md`) |

**Required per-phase fields:**

| Field | Present in all phases? | Notes |
|---|---|---|
| Phase ID + short name | Yes | e.g., `P0 — Bootstrap` |
| Goal (one sentence) | Yes | First bullet under each phase |
| Action list | Yes | Per-phase *Action list* |
| Deliverable artifact(s) | Yes | Per-phase *Deliverable* |
| Acceptance criteria (specific and testable) | Yes (after Draft 5) | Draft 4 had three phases with "As Draft 2" placeholders; all inlined in Draft 5 |
| Dependency list | Yes | Per-phase *Dependencies* |
| Hinge-test list | Yes | Per-phase *Hinge-test list*; full registry in *Deferred-Decision Registry* |
| Evaluation-metric impact | Yes (added in Draft 5) | Per-phase *Evaluation-metric impact* field; absent in Draft 4 |
| Estimated rounds-to-convergence | Yes | Per-phase *Estimated rounds-to-convergence* |

---

## Phase Decomposition

Fifteen phases. P0–P3c are foundations (Vault library, audit store, contract, sidecar). P4 is the CLI Setup Wizard. P5–P9 are workflow stages delivered through the CLI. P10a (Evaluation Infrastructure) and P10b (Hinge-Test Framework) split in Draft 6 from a single P10; both ship independently. P11 is dogfooding + documentation.

(For phase-count audit: P0, P1, P2, P3a, P3b, P3c, P4, P5, P6, P7, P8, P9, P10a, P10b, P11 = 15 phases. The two sub-phase groupings — P3a/b/c and P10a/b — are each counted as distinct phases because each has its own acceptance criteria, deliverable, and review round. They are not consolidated as parent phases for counting purposes.)

---

### **P0 — Bootstrap**

- **Goal.** Stand up Rust workspace + Go module + protobuf code generation + build orchestration.
- **Action list.**
  - Initialize Cargo workspace with stub member crates listed in the file-system layout.
  - `anvil-cli` ships stub `main.rs` printing version on `anvil --version`.
  - `rust-toolchain.toml` pins Rust ≥1.80 stable.
  - Initialize Go module under `sidecar/`. Sidecar `main.go` stub prints version on `anvil-sidecar --version`.
  - Add `proto/anvil/v1/sidecar.proto` placeholder with `package anvil.v1` and a `Ping` RPC.
  - Add `justfile` orchestrating: `just build` (Rust + Go), `just test`, `just gen` (protobuf), `just lint`, `just fmt`, `just dev-sidecar` (launch sidecar in dev mode).
  - Configure `rustfmt.toml`, `clippy` (deny warnings), `golangci-lint`.
  - Hinge-test convention stubs in both languages. **Specifically:** from P0 onward, hinge tests are ordinary unit tests (`cargo test` / `go test`) whose source-level annotations follow a structured-comment convention (`// hinge_test: pins=<value>, intended=<value>, phase=<P-id>` in Rust above the `#[test]` attribute; equivalent comment in Go above the `func Test...`). The P10b *Hinge-Test Framework* phase implements the auto-discovery, parsing, and registry persistence; until then, hinge tests behave as ordinary tests with structured comments that later phases import. This means hinge tests can be *declared* with full metadata from P0, even though the framework that auto-tracks them ships at P10b.
- **Deliverable.** `just build` produces two binaries (`anvil`, `anvil-sidecar`). `just test` runs both test suites. Lint passes cleanly.
- **Acceptance criteria.**
  1. Fresh-clone build works given Rust ≥1.80 and Go ≥1.22.
  2. `anvil --version` and `anvil-sidecar --version` print correctly.
  3. `cargo test` and `go test ./...` both succeed.
  4. `clippy` and `golangci-lint` report zero issues.
  5. `just gen` regenerates Rust + Go bindings without warnings.
  6. README references Charter and Plan files.
- **Dependencies.** None.
- **Hinge-test list.**
  - `test_rust_toolchain_version_floor` (Rust)
  - `test_go_toolchain_version_floor` (Go)
  - `test_cli_entry_point_exists` (Rust)
  - `test_sidecar_entry_point_exists` (Go)
- **Evaluation-metric impact.** Deferred-decision resolution rate: introduces first 4 hinge tests; rate starts at 0/4 (0%). No workflow data yet.
- **Estimated rounds-to-convergence.** 2.

---

### **P1 — Config Schema and Charter Loader**

- **Goal.** Implement Required-Choices schema, Charter loader, CLI surface for project initialization.
- **Action list.**
  - Required-Choices schema in `anvil-core` using `serde` + Rust enums for lock state (Final / Provisional / Unlocked).
  - **Provider-connection + model-binding schema.** Provider connections are configurable, named contexts (e.g., `my-anthropic-direct`, `my-bedrock-us-east-1`, `my-azure-openai-east`); each carries provider type + credentials reference + endpoint metadata. Model bindings tuple `(model_identity, provider_connection)` so the same model identity can have multiple bindings if accessed via multiple providers. Role assignments reference model bindings. The family-floor invariant runs on the *model identities* behind the assignments, not on the connections — Claude via Anthropic-direct and Claude via Bedrock are both Claude family.
  - Charter loader: read `charter.md`, extract metadata, compute content hash.
  - Provisional Lock support with `hypothesis` and `revision_trigger` fields.
  - Implement `anvil init <project-path>` in `anvil-cli`. `anvil init` is the **idempotent scaffold command**: it creates the per-project directory layout and a default `anvil.toml` without running the interactive wizard. Re-running `anvil init` on an already-initialized project is a no-op with a status report. It does not prompt for API keys or model assignments; that is `anvil setup` (P4).
  - Implement `anvil config show` and `anvil config set <key> <value>`.
  - Pre-Plan-stage gate check.
  - Config storage: TOML (`anvil.toml` at project root).
- **Deliverable.** A user can initialize a project, set/view configuration, and the gate check blocks Plan stage if Choices are unlocked.
- **Acceptance criteria.**
  1. `anvil init my-project` creates the per-project directory layout.
  2. Required-Choices schema covers all 16 Choices from the Charter + this Plan (including the two "v1.1 prep" provisional locks and the now-locked sidecar lifecycle).
  3. `anvil config show` displays lock status.
  4. Provisional Locks require non-empty hypothesis and revision_trigger fields.
  5. Pre-Plan-stage gate check exits non-zero with clear listing of unlocked Choices.
  6. Malformed TOML produces typed errors, not parse-panics.
  7. `anvil init` on an already-initialized project prints current status and exits zero without modifying state.
- **Dependencies.** P0.
- **Hinge-test list.**
  - `test_required_choices_count` — pins 16 (updated from 15 to include sidecar lifecycle).
  - `test_project_layout_directories` — pins per-project directory names.
- **Evaluation-metric impact.** Deferred-decision resolution rate: adds 2 hinge tests; Provisional Lock mechanism enables Choice tracking. Human minutes per shipped phase: config inspection commands reduce friction in later phases.
- **Estimated rounds-to-convergence.** 2.

---

### **P2 — Audit Store and Provenance Graph**

- **Goal.** Filesystem-backed audit store with 11 required record types, cross-reference keys, append-only enforcement, integrity check. Rust's type system used to make append-only hard to violate.
- **Action list.**
  - Audit-store storage layout: one subdir per record type under `audit-store/`; one JSON file per record, named by stable record ID.
  - Record-type Rust structs for the 11 Charter-required types plus two Plan-extensions (Charter says "minimum set; Plan may extend"): `ReviewerFindingPacket`, `VerifierResult`, `RotationLog`, `CharterAmendment`, `PlanAmendment`, `PhaseDisposition`, `HingeFlip`, `GateApproval`, `ConvergenceDeclaration`, `ProvisionalLock`, `RollbackEvent`, plus `ArbiterFindingResolution` (R4 addition — per-finding arbiter override; see P6), plus `SidecarReload` (R4 addition — config-epoch reload events; see P3b). Total: 13 record types in v1.
  - Cross-reference key generation: `<artifact-path>:<section-id>:<version>`.
  - Append-only enforcement: store API exposes `append(record)` only; no `update` / `delete`. Filesystem-level: `open` with `O_CREATE|O_EXCL`.
  - Index file (`audit-store/_index.json`) updated atomically on every `append(record)` call; each entry records the record ID, type, and expected file path.
  - Audit store completeness check: the integrity check compares the index against physically present files; a record whose ID appears in the index but whose file is missing from disk is reported as a `BlockShip` violation. Protects against out-of-band deletion defeating the append-only guarantee.
  - Provenance Graph as a queryable view in `anvil-graph`.
  - `anvil audit list <type>` and `anvil audit show <id>` CLI commands.
  - Cross-Reference Integrity check (`Pass` / `Warn` / `BlockShip`).
  - UTF-8 lint at audit-store boundary.
- **Deliverable.** Audit store is operational; records can be created, queried, and cross-referenced; integrity check runs against the Anvil Charter file.
- **Acceptance criteria.**
  1. All 11 record-type schemas defined in `anvil-audit` as Rust types.
  2. Cross-reference keys stable across re-renderings (hinge-tested).
  3. Append-only enforced at both API and filesystem levels.
  4. Provenance Graph resolves "what records back this artifact section?" correctly.
  5. Cross-Reference Integrity check produces `BlockShip` for sections lacking backing records.
  6. UTF-8 lint flags invalid byte sequences.
  7. Audit store completeness check detects records present in `_index.json` but physically missing from disk and reports `BlockShip`. **Threat model:** this is *local tamper detection*, not adversarial tamper-proofing. It catches accidental deletion (a contributor `rm`s a file), partial restores from backup, and filesystem corruption. It does not defend against an adversary who can modify both the file and the index entry; cryptographic tamper-proofing (chained record hashes, signed manifests, periodic snapshots) is a v1.x consideration, surfaced in *Open Items*.

  8. Layer-1 metric counters are wired up at the audit-store write path (each `append(record)` call increments the appropriate counter); collection infrastructure (P10a) reads from these counters. P2 ships the *instrumentation hooks*, not the dashboards.
- **Dependencies.** P0, P1.
- **Hinge-test list.**
  - `test_audit_store_required_types_present` — *constitutional, subset check*: asserts that the 11 Charter-required record types (`ReviewerFindingPacket`, `VerifierResult`, `RotationLog`, `CharterAmendment`, `PlanAmendment`, `PhaseDisposition`, `HingeFlip`, `GateApproval`, `ConvergenceDeclaration`, `ProvisionalLock`, `RollbackEvent`) are all present in the implementation's record-type set. Does not assert exact total count; Plan-level extensions (currently `ArbiterFindingResolution`, `SidecarReload`) are permitted growth under the Charter's "minimum set; Plan may extend" wording. Per R3's pin convention, this is now correctly a subset/minimum check rather than an exact-equality count, which fixes the brittleness of the prior pinned-11 formulation under legitimate Plan-level additions.
  - `test_append_only_api_has_no_update_or_delete`
  - `test_append_only_filesystem_o_excl`
  - `test_cross_reference_key_stability`
  - `test_audit_store_detects_deleted_records`
- **Evaluation-metric impact.** All six Layer-1 metrics become instrumentable after this phase — the audit store is the data source for every metric. No project workflow data flows yet; this phase provides the infrastructure that later phases write into.
- **Estimated rounds-to-convergence.** 3.

---

### **P3a — Contract Definition (protobuf)**

- **Goal.** Define wire contract between Vault and sidecar.
- **Action list.**
  - Author `proto/anvil/v1/sidecar.proto` with the `Sidecar` service: `Handshake`, `Invoke`, `InvokeStreaming`, `Cancel`, `Health`, `ReloadConfig`. The `ReloadConfig` RPC is added in Draft 6 post-R4 to close the split-brain state-drift gap (see *Configuration Epoch* below): the Vault calls it when it detects a `provider-config` mismatch between the running sidecar and the current `anvil.toml`.
  - Command envelope: `InvokeRequest { string idempotency_key, string model_id, string provider_connection_id, Credentials credentials, oneof payload { ChatRequest chat, EmbedRequest embed, ... }, optional Timeout timeout }`. The `model_id` carries model identity (used by the Vault's family-floor check); the `provider_connection_id` names which configured connection the sidecar should route the call through; `credentials` carries the per-call secret material (see *Credentials Schema* below).
  - Result envelope: `InvokeResponse { string idempotency_key, oneof result { ChatResponse chat, ... }, optional AnvilError error }`.
  - Streaming event schema: `InvokeStreamEvent { string idempotency_key, oneof event { Token token, FinalResult result, Error error, Heartbeat heartbeat } }`.
  - Error message: `AnvilError { ErrorClass class = 1; string vendor_code = 2; string message = 3; map<string, string> details = 4; }` where `ErrorClass` enum: `TRANSPORT`, `PROVIDER_REFUSAL`, `SCHEMA_VIOLATION`, `ADAPTER_BUG`, `TIMEOUT`, `CANCELLED`.
  - Version handshake schema: `HandshakeRequest { string core_protocol_version, repeated string supported_versions, string vault_config_epoch }` / `HandshakeResponse { string negotiated_version, string sidecar_version, string sidecar_build_info, string sidecar_config_epoch }`. The `*_config_epoch` fields are SHA-256 hashes of the active `provider-config` content (sidecar's loaded state on its side; Vault's current `anvil.toml`-derived state on its side). The Vault compares the two on every handshake (which happens on every CLI invocation that talks to the sidecar daemon) and, on mismatch, either calls `ReloadConfig` (clean path) or force-restarts the sidecar (recovery path).
  - **Configuration Epoch.** The sidecar daemon persists across CLI invocations, but the Vault can revise `anvil.toml` between invocations. Without epoch checking, the daemon could happily serve requests against a stale `provider-config` — including a `provider_connection_id` the Vault has redefined or removed. The epoch hash gives the Vault a fast staleness signal; the `ReloadConfig` RPC gives it a graceful recovery path without process restart. Force-restart is the fallback when `ReloadConfig` fails or returns an error.
  - `ReloadConfig { string new_config_epoch, bytes new_provider_config }` request; `ReloadConfigResponse { bool success, optional AnvilError error, string active_config_epoch }` response. The sidecar atomically swaps its in-memory `provider-config` state on success; on failure, it retains the prior state and surfaces the error class (`AdapterBug` if the new config is malformed, `Transport` if a connectivity check against any required endpoint fails during the swap).
  - Document schema-versioning policy in `proto/README.md`.
  - Document the **"no commit on partial output" rule** as a contract invariant (referencing the Plan-Level Trust-Boundary Invariants section).
  - Set up `prost` + `tonic-build` (Rust); `protoc-gen-go` + `protoc-gen-go-grpc` (Go). `just gen` regenerates both.
- **Deliverable.** `proto/anvil/v1/sidecar.proto` compiles; generated Rust and Go bindings work.
- **Acceptance criteria.**
  1. `just gen` regenerates both sides without warnings.
  2. `Sidecar` service exposes all five RPCs.
  3. `ErrorClass` enum lists all six classes.
  4. Streaming event schema separates `Token`, `FinalResult`, `Error`, `Heartbeat`.
  5. Version-handshake schema requires `core_protocol_version` + `supported_versions` on connect.
  6. `proto/README.md` documents schema-versioning policy.
  7. "No commit on partial output" rule documented with reference to relevant `ErrorClass` values.
- **Dependencies.** P0.
- **Hinge-test list.**
  - `test_proto_package_version` (Build)
  - `test_error_class_count` — pins 6.
  - `test_handshake_required_fields`
- **Evaluation-metric impact.** No direct metric impact. Foundational: defines the contract that P3b and P3c implement, enabling model invocations in P5+.
- **Estimated rounds-to-convergence.** 2.

---

### **P3b — Rust Sidecar Client (Vault side)**

- **Goal.** Vault-side gRPC client with contract enforcement.
- **Action list.**
  - `anvil-sidecar-client` crate with `SidecarClient` struct owning a `tonic` gRPC client.
  - Connection handshake; refuse to operate if no protocol-version overlap.
  - Typed `invoke(request) -> Result<Response, AnvilError>` and `invoke_streaming(request) -> impl Stream<Item = InvokeStreamEvent>`.
  - Contract-enforcement layer: every response schema-validated; deserialization failures become `ErrorClass::SchemaViolation`.
  - Retry/backoff for `Transport` failures (exponential + jitter, configurable max).
  - Idempotency-key generation (UUIDv7).
  - Timeout / cancellation: client-side enforcement; `Cancel` RPC on Ctrl-C.
  - Streaming partial-output rule: `Token` events may pass through to the caller's display sink (terminal stdout) as they arrive — this is ephemeral UX, not commit. The Vault's commit path consumes only `FinalResult`; `Token` events do not produce audit-store writes or artifact-tree changes. On `Error` mid-stream, the Vault stops forwarding tokens to the display sink, discards all accumulated stream state from the commit path, and returns only the typed error to the caller's commit path. (Enforces Plan-Level Trust-Boundary Invariant #1.)
  - **Configuration-epoch validation.** On every Handshake (every CLI invocation that connects to the daemon), the client computes the SHA-256 of the active `anvil.toml`-derived `provider-config` and sends it as `vault_config_epoch`. The sidecar returns its loaded `sidecar_config_epoch`. On mismatch, the client first attempts `ReloadConfig` with the new bytes; if `ReloadConfig` returns success, the client proceeds with the original request. If `ReloadConfig` fails (any error class), the client kills the daemon (SIGTERM with 5-second grace period, then SIGKILL) and respawns it; the new daemon picks up the current `--provider-config` at startup. Either path is logged to the audit store as a `SidecarReload` record (added to the record-type list — see P2 update below).
  - Telemetry: emit `RotationLog` audit record per invocation.
  - `MockSidecar` for unit testing.
- **Deliverable.** Vault can talk to sidecar, enforce contract, classify errors, retry transport failures.
- **Acceptance criteria.**
  1. Handshake refuses no-version-overlap connections.
  2. Schema-validation produces `SchemaViolation` on drift.
  3. Transport failures retry; non-Transport errors surface immediately.
  4. Idempotency keys round-trip intact.
  5. Cancellation propagates; partial output discarded.
  6. Every invocation produces `RotationLog` record.
  7. `MockSidecar` usable for downstream unit tests.
- **Dependencies.** P0, P2 (audit), P3a (contract).
- **Hinge-test list.**
  - `test_handshake_refuses_no_version_overlap`
  - `test_schema_violation_classification`
  - `test_transport_failure_retries`
  - `test_partial_output_discarded_on_streaming_error`
- **Evaluation-metric impact.** No direct metric impact. Foundational: enables the sidecar invocations that P5+ workflow phases depend on.
- **Estimated rounds-to-convergence.** 3.

---

### **P3c — Go Sidecar (vendor adapters)**

- **Goal.** Go sidecar with gRPC server, vendor adapters, and daemon lifecycle support.
- **Action list.**
  - gRPC server in `sidecar/cmd/anvil-sidecar/main.go`.
  - **Provider adapters** in `sidecar/internal/adapters/{provider}/`. Each adapter handles one provider's API. **Direct-vendor APIs (Anthropic, OpenAI, Google, xAI) and cloud-hosted gateways (Azure OpenAI, AWS Bedrock, Google Vertex AI, DigitalOcean Gradient, OpenRouter, etc.) are equal citizens** — each is one adapter implementing the same `ProviderAdapter` interface. Adapters know how to: authenticate (API key, AWS SigV4, Azure AD, GCP service account, etc.); translate the Anvil contract envelope to the provider's API surface; map provider-specific errors to the contract's `ErrorClass` values; handle streaming where supported. Raw HTTP, no vendor SDKs.
  - **v1 ships adapters for: Anthropic direct, OpenAI direct, Google AI Studio direct** (the minimum set required to satisfy the family-floor invariant with Claude as Coder). The architecture is designed so additional adapters — cloud-hosted variants (Bedrock, Vertex, Azure OpenAI), other direct APIs (xAI), and gateways (Gradient, OpenRouter) — can be added in later phases or v1.1+ **without changes to the Vault, the contract, or the existing adapters**. Each new adapter is additive.
  - **Model identity vs. provider access.** The sidecar distinguishes the *model* being invoked (vendor of the model + family + version — what determines diversity-floor membership) from the *provider connection* used to invoke it (provider type + endpoint + region). The contract's `model_id` field carries the model identity. The contract's `provider_connection_id` field names which connection to route through — this field is *not* opaque to the Vault; the Vault populates it from the active model binding. What *is* opaque to the Vault is the connection's internal configuration (endpoint URL, region, provider-specific auth metadata): the sidecar holds that configuration via `--provider-config` and resolves it from the `provider_connection_id` per-call. Secret material is passed per-call via the `Credentials` field; the sidecar does not cache credentials between calls (per *Plan-Level Trust-Boundary Invariant #2*).
  - `Handshake` RPC announces sidecar version and supported protocol versions.
  - Structured logging (JSON to `.anvil/logs/sidecar.log`, with stderr pass-through in verbose mode) with correlation IDs matching idempotency keys.
  - Health-check RPC.
  - **Daemon lifecycle:** on startup, bind to a random loopback port, write PID to `.anvil/run/sidecar.pid` and port to `.anvil/run/sidecar.port`. **Also register globally** in `~/.anvil/global-registry.json` — a user-home JSON file mapping each active sidecar's `(workspace_path, pid, port, started_at, last_seen_at)`. The daemon updates `last_seen_at` periodically (every 60 seconds) and removes its own entry on clean exit. Auto-exit after idle timeout (default 30 min, configurable). Graceful shutdown on SIGTERM: drain in-flight calls, update global registry, then exit.
  - **Global-aware sidecar management.** Every `anvil` CLI invocation that touches the sidecar layer performs a quick sweep of `~/.anvil/global-registry.json`: any entry whose `last_seen_at` is older than 2× idle-timeout is treated as a *stale* daemon. Stale entries are surfaced as warnings on every invocation until cleaned. `anvil sidecar status --all` lists all active and stale daemons across workspaces; `anvil sidecar kill --stale` removes stale daemons (sends SIGTERM, then SIGKILL after grace period, then removes registry entry); `anvil sidecar kill --workspace <path>` targets a specific workspace's daemon. Stale-daemon detection is also a smoke-test scenario in P11.
  - Configuration: provider-connection metadata (endpoint URL, region, provider type) loaded from `--provider-config` at sidecar startup; this is non-secret routing data. **Secret material (API keys, SigV4 credentials, OAuth tokens) is never loaded from env vars or files at sidecar startup;** it flows per-call in the `Credentials` field of `InvokeRequest` and is consumed within the request handler then discarded. Vendor endpoints configurable for testing via `--provider-config`. The env-var path for API keys exists only at the *CLI / wizard* layer (P4 Step 2) for headless / CI workflows, where the Vault reads the key from the env at CLI invocation time and injects it into the per-call `Credentials`; the sidecar process itself never reads the env var.
  - Integration tests against vendor APIs (gated by env-var API keys); contract-conformance tests.
- **Deliverable.** Sidecar binary starts as a daemon, accepts gRPC connections, proxies invocations correctly, cleans up on exit.
- **Acceptance criteria.**
  1. Sidecar starts, writes PID and port files, and accepts connections.
  2. `Handshake` advertises supported versions.
  3. Each adapter correctly maps vendor errors to `ErrorClass` (hinge-tested against recorded fixtures).
  4. `Invoke` proxies to all three vendors with consistent envelope shape.
  5. `InvokeStreaming` emits `Token` events as tokens become available from the upstream provider (ephemeral, for display); emits exactly one `FinalResult` event on success, or exactly one `Error` event on failure. On `Error`, no `FinalResult` is emitted. The sidecar does not buffer-then-replay tokens: streaming is live, but commit is gated on `FinalResult`.
  6. Graceful shutdown drains in-flight calls before exiting.
  7. Idle-timeout auto-exit fires within ±10% of configured value.
  8. PID and port files are removed on clean shutdown.
  9. One log line per significant event with idempotency key as correlation ID.
- **Dependencies.** P0, P3a. Parallelizable with P3b.
- **Hinge-test list.**
  - `test_v1_minimum_provider_adapters` (Go) — pins the v1 minimum set: Anthropic direct, OpenAI direct, Google AI Studio direct. Flippable as new adapters are added in later phases.
  - `test_provider_adapter_interface_extensibility` (Go) — pins that the `ProviderAdapter` interface allows new adapters without modifying Vault, contract, or existing adapters. Stable invariant.
  - `test_error_class_mapping_anthropic`
  - `test_error_class_mapping_openai`
  - `test_error_class_mapping_google`
  - `test_streaming_aborts_on_error_no_continuation`
- **Evaluation-metric impact.** No direct metric impact. Foundational: vendor adapters operational, enabling model-backed workflow steps in P5+.
- **Estimated rounds-to-convergence.** 3.

---

### **P4 — Interactive CLI Setup Wizard**

- **Goal.** First-run setup walks a fresh user through seven steps with no opaque side effects. Each step is visible, validated, and audited.
- **Command decision (locked in Draft 5).** `anvil init <path>` (P1) creates the scaffold; `anvil setup` runs the interactive wizard. They are distinct commands. `anvil setup` can be run after `anvil init` (or it runs `anvil init` implicitly if the project is not yet initialized). Re-running `anvil setup` on an already-configured project prompts for which steps to revisit; it does not wipe existing state. This distinction separates idempotent scaffolding from interactive configuration and is visible in both the CLI help text and the onboarding docs.
- **Action list.** Implement `anvil setup` that runs an interactive wizard:
  - **Step 1: Workspace root selection.** Either argument-supplied (`anvil setup <path>`) or interactive prompt. Validates folder is writable and not already an Anvil project (or prompts to load existing). Runs `anvil init` implicitly if needed.
  - **Step 2: Provider connections.** Interactive prompts to configure one or more provider connections. Each connection has a user-supplied name (e.g., `my-anthropic`, `my-bedrock-us-east-1`) and a provider-type-specific credential set. v1 supports: Anthropic direct (API key), OpenAI direct (API key), Google AI Studio direct (API key). Each connection's credentials are validated via a minimal test call through the sidecar; failure surfaces a clear error inline. A user may configure multiple connections of the same provider type (e.g., multiple Anthropic API keys for different accounts) or skip vendors they don't intend to use.

    **Credential storage (R4 hardening).** v1 supports two paths for persistent credential storage, and *only* two:
    - *OS keychain (preferred, default for interactive setup):* `keyring` crate → Windows Credential Manager / macOS Keychain / Linux Secret Service. The wizard probes for keychain availability at Step 2 entry.
    - *Environment variables (floor):* if the keychain is unavailable on the target system (notably some Linux configurations without a running Secret Service, or CI environments), the wizard refuses persistent storage and emits a **clear security warning** explaining that the user must supply credentials per-session via `ANVIL_API_KEY_*` environment variables. The wizard records the choice as a `ProvisionalLock` entry naming the unavailable keychain and the env-var-only mode.

    **File-based encryption with user passphrase is explicitly NOT in v1.** Prior draft language suggested it as a fallback; R4 review correctly identified custom-encryption-at-rest as an unnecessary security surface area. Implementing a sound passphrase-based encryption scheme (key derivation, salt management, ciphertext format versioning, passphrase rotation) is significantly more work than v1 should take on, and the failure modes of a weak implementation are severe. The env-var floor is the conservative alternative: it does not store credentials at rest at all, which is strictly safer than a homegrown encryption scheme. Reconsidering this for v1.x is possible if user feedback indicates the env-var floor is too friction-heavy on no-keychain systems; until then, no file-based encryption.
  - **Step 3: Model bindings and role assignment.** Interactive prompts to bind specific models to roles. For each role (Coder, Interlocutor, Planner, Reviewer-1, Reviewer-2), the user selects: which model identity (e.g., `claude-opus-4.6`, `gpt-5`, `gemini-2.5-pro`) and which provider connection to access it through. The same model identity can be used multiple times in the project with different connections (e.g., two Claude reviewers via different accounts — though this would violate the family-floor invariant in Step 4).
  - **Step 4: Adversarial Diversity policy validation.** Computes the family-floor check; refuses to proceed if violated; shows the specific conflict.
  - **Step 5: Adapter connectivity test.** Spawns the sidecar daemon (or probes the existing daemon), pings each configured vendor through it; reports pass/fail per vendor. Failures are explicit: `anvil setup` does not continue past Step 5 with a failing adapter.
  - **Step 6: Initial local store creation.** Creates the per-project directory layout. Reports each created path.
  - **Step 7: Confirmation and summary.** Prints the locked Required Choices, audit-store layout, next-step suggestions.
  - Every step persists state changes through the Vault: `ProvisionalLock` records for each Choice locked during setup, `GateApproval` records for each step's confirmation. Cancelling mid-wizard leaves no partial state (transaction-style: changes commit only when the wizard completes; cancellation rolls back).
- **Deliverable.** A user can run `anvil setup` on a fresh project and reach a state where `anvil plan` is unblocked, with every setup decision visible and audited.
- **Acceptance criteria.**
  1. All seven wizard steps complete for a fresh project.
  2. Cancellation at any step leaves no partial state in the workspace.
  3. API key validation actually invokes the provider through the sidecar; wrong key detected inline.
  4. Diversity validation rejects same-family pools with specific conflict message.
  5. Adapter connectivity test passes for valid configs, fails clearly for invalid.
  6. All 11 audit-store record-type subdirectories created in Step 6.
  7. Encrypted-at-rest API keys not visible in plaintext anywhere on disk.
  8. Every wizard step produces audit records (`ProvisionalLock`, `GateApproval`, etc.).
  9. `anvil setup` on an already-configured project offers step-level re-run, not full wipe.
  10. `anvil sidecar status` reflects a running daemon after Step 5 completes successfully.
  11. API keys can be supplied via environment variables (`ANVIL_API_KEY_ANTHROPIC`, `ANVIL_API_KEY_OPENAI`, `ANVIL_API_KEY_GOOGLE`) without triggering keyring interaction or interactive passphrase prompts. The env-var path is the sole key-supply mechanism in headless and CI environments; the wizard detects a non-interactive terminal and skips the interactive step for any vendor whose key is already in the environment.
  12. **Clean-Windows-machine first-time-user walkthrough (R5 addition).** A non-author reviewer (someone other than the Coder) walks the wizard end-to-end on a clean Windows machine with: no prior Anvil install, no prior sidecar daemon running, no existing `~/.anvil/global-registry.json`, and no existing keychain entries for `ANVIL_*` credentials. The walkthrough produces no unexpected errors, no missing prompts, no behavior surprises. The reviewer records the walkthrough as a structured document in `docs/p4-walkthrough.md` (timestamps per step, terminal output excerpts, the keychain prompts encountered, any deviations from the runbook). A clean walkthrough is a P4 ship gate — the phase does not ship until at least one reviewer-walkthrough document exists for the primary platform.
- **Dependencies.** P0, P1, P2, P3a, P3b, P3c (Step 5 needs working sidecar).
- **Hinge-test list.**
  - `test_wizard_step_count` — pins 7.
  - `test_diversity_policy_validation_rejects_same_family`
  - `test_workspace_lock_enforced` — pins that two `anvil` processes can't write the same workspace.
  - `test_api_keys_encrypted_at_rest`
  - `test_wizard_cancellation_leaves_no_partial_state`
  - `test_api_keys_env_var_bypass_works_headless`
- **Evaluation-metric impact.** Human minutes per shipped phase: setup time baseline established (setup is outside the per-phase workflow loop but informs overall friction). Deferred-decision resolution rate: adds 5 hinge tests.
- **Estimated rounds-to-convergence.** 3.

---

### **P5 — Charter Stage Pipeline (Single Reviewer)**

- **Goal.** First end-to-end workflow stage via CLI: Interlocutor discussion → Charter render → single reviewer → verifier → curation → disposition.
- **Action list.**
  - `anvil discuss` — interactive Interlocutor session, streaming via sidecar. Conversation logged; Charter packet is structured output.
  - Coder renders Charter from packet → `charter.md`.
  - `anvil charter review` — invokes next reviewer in rotation (single reviewer in this phase).
  - Findings-packet schema as Rust type.
  - Finding Verifier — grounds findings against artifact; emits Grounded / Refuted / CannotBeVerified.
  - `anvil charter findings` — CLI flow showing verified findings; per-finding Accept / Drop / Edit / Annotate actions.
  - Disposition rendering (`REVIEW_<artifact>_R<N>.md`).
  - Hardening-history append (`HARDENING_<artifact>.md`).
- **Deliverable.** End-to-end Charter cycle via CLI commands.
- **Acceptance criteria.**
  1. Charter packet meets required fields.
  2. Charter rendering produces valid `charter.md`.
  3. Reviewer invocation produces conforming findings packet.
  4. Verifier produces verified results with evidence pointers.
  5. Curation gestures persist as audit records, round-trip correctly.
  6. Disposition rendering matches required format (Verification, Disposition table, Files Changed, Corrections, Residual, Reproducibility sections).
  7. Hardening-history append works; Charter body not contaminated.
- **Dependencies.** P0–P4.
- **Hinge-test list.**
  - `test_findings_packet_schema`
  - `test_disposition_doc_required_sections`
  - `test_curation_audit_record_required`
- **Evaluation-metric impact.** First workflow cycle data: Review finding precision (first grounded/refuted breakdown), Review rounds per phase (first round count), and Cross-reviewer agreement (single-reviewer baseline — agreement is trivially 100% with one reviewer; meaningful data begins in P6).
- **Estimated rounds-to-convergence.** 3.

---

### **P6 — Multi-Reviewer Rotation + Convergence Safeguards**

- **Goal.** Multi-reviewer pool, deterministic rotation, severity-tiered convergence, human arbiter authority.
- **Action list.**
  - Rotation arithmetic in `anvil-core`.
  - Per-artifact round counting.
  - **Severity-tiered convergence (post-round-5 behavior, locked here):** After round 5, P2 and P3 findings are marked **advisory** in the findings packet. Each advisory finding must receive an explicit human disposition — one of `Accept-Advisory` (acknowledged; no action; finding recorded in the disposition), `Drop-Advisory` (finding refuted or non-applicable; reason required), or `Defer-Advisory` (deferred to a named future phase; target phase required). No advisory finding may pass silently without an explicit disposition. The gate check at the `next-reviewer-or-ship` transition verifies that all advisory findings in the current round have been disposed. Advisory findings are stored in the `reviewer-finding-packet` audit record with `advisory: true`. The `convergence-declaration` audit record notes the count of outstanding advisory findings at declaration time.
  - `anvil arbiter declare-convergence <artifact>` — creates `ConvergenceDeclaration` audit record with required non-empty reasoning field. Exits non-zero if reasoning is empty.
  - **Per-finding arbiter resolution (R4 addition).** Beyond artifact-level convergence declaration, the Coordinator may explicitly resolve an individual finding as **Arbiter-Decided** via `anvil arbiter resolve-finding <finding-id> --reason "<text>"`. This is the escape hatch for *reviewer contradictions* — the failure mode where Reviewer A insists on direction X, Coder implements X, then Reviewer B raises a finding that contradicts X's rationale. Without per-finding resolution, the full-pool-clean termination becomes structurally impossible: every fix toward A's direction surfaces a new B finding, and vice versa. With per-finding arbiter resolution, the Coordinator names the contradiction explicitly: "Finding F is Arbiter-Decided; the chosen direction is X; B's countervailing finding is acknowledged as a legitimate alternative but is not the chosen path." The resolution is logged to a new `ArbiterFindingResolution` audit record (12th record type) with: finding ID, arbiter identifier, reasoning, chosen-direction summary, contradiction context (which other findings or rounds the contradiction relates to). The full-pool-clean termination check **ignores findings whose latest resolution is Arbiter-Decided** — they are explicitly settled and do not block ship. Arbiter-Decided is distinct from Disposition labels (Fixed / Refuted / Deferred) in that it is a *meta-resolution* applied alongside the disposition, not in place of it: the finding still has a disposition for the Coder's record-keeping, but the arbiter has overridden the convergence-blocking effect. Reviewers see Arbiter-Decided findings in their input briefing flagged as such; they are advised that re-raising the same direction-of-finding is welcome but does not change ship-gate status.
  - `anvil status` shows rotation position (current reviewer and next in queue), round count per artifact, convergence declaration count, count of open advisory findings, and count of Arbiter-Decided findings on the active artifact.
  - Full-pool clean termination check, with Arbiter-Decided findings excluded from the blocking set.
- **Deliverable.** Multi-reviewer rotation works; convergence safeguards activate; arbiter command works; advisory-finding disposition enforced at gate.
- **Acceptance criteria.**
  1. Rotation selects reviewers deterministically in round-robin order; rotation path verifiable via `rotation-log` audit records.
  2. Per-artifact round counter increments correctly; reaching round 5 triggers severity-tiering flag (hinge-tested).
  3. P2/P3 findings in rounds 6+ are marked advisory; gate check rejects advancement if any advisory finding lacks explicit disposition.
  4. Each advisory disposition type (`Accept-Advisory`, `Drop-Advisory`, `Defer-Advisory`) persists correctly as an audit record.
  5. `anvil arbiter declare-convergence <artifact>` creates `ConvergenceDeclaration` record with reasoning; empty reasoning exits non-zero.
  6. `anvil status` shows rotation position, round count, declaration count, and open advisory finding count.
  7. Full-pool clean termination requires all pool members to have produced a clean pass on the current artifact state; partial-pool clean does not satisfy the default condition.
  8. Single-clean-pass override is configurable per project; override is visible in `anvil status` and in the config.
  9. `anvil arbiter resolve-finding <finding-id> --reason "<text>"` creates an `ArbiterFindingResolution` audit record with non-empty reasoning; empty reasoning exits non-zero. The targeted finding is excluded from the full-pool-clean blocking set on subsequent termination checks.
  10. Reviewers receive Arbiter-Decided findings in their input briefing with explicit flag; re-raising the same direction-of-finding does not change the ship-gate status (verified by integration test: a sequence where R1 raises F, Coordinator resolves F as Arbiter-Decided, R2 re-raises F → Coordinator's `anvil status` shows F as Arbiter-Decided across rounds; full-pool-clean ignores it).
- **Dependencies.** P3a/b/c, P5.
- **Hinge-test list.**
  - `test_round_limit_default_is_5`
  - `test_severity_tiering_at_round_6`
- **Evaluation-metric impact.** Cross-reviewer agreement: multi-reviewer data meaningful with rotation. Review rounds per phase: convergence safeguards limit runaway iteration and produce the per-artifact round count that feeds this metric.
- **Estimated rounds-to-convergence.** 2.

---

### **P7 — Plan Stage Pipeline**

- **Goal.** Plan stage delivered via CLI; Planner Contract validation; Plan Consolidation; dependency-graph artifact.
- **Action list.**
  - `anvil plan` invokes Planner via sidecar with approved Charter + Required Choices.
  - Planner Contract validation; per-phase field enforcement (all nine required fields, including evaluation-metric impact).
  - Plan rendering.
  - Plan Review reuses Charter machinery.
  - Plan Consolidation logic.
  - Dependency graph queryable via the `anvil-graph` Rust crate (library); the CLI surface for graph operations is the `anvil graph <verb>` subcommand family (e.g., `anvil graph show`, `anvil graph blast-radius <phase-id>`), conforming to the verb-resource pattern. The crate name and the CLI command differ by intent: the crate is the library consumers (CLI, App, embedders) link against; the CLI subcommand is the user-facing surface.
- **Deliverable.** A user can produce a Plan that passes Planner-Contract validation; Plan can be reviewed, hardened, consolidated.
- **Acceptance criteria.**
  1. `anvil plan` invokes the Planner with the approved Charter and locked Required Choices; exits non-zero if Charter is not in approved state.
  2. Planner Contract validation enforces all nine required per-phase fields; missing fields produce typed errors naming the field and phase.
  3. Plan rendering produces a self-contained Plan document with all required sections.
  4. Plan Review reuses the Charter review machinery (same finding/curation/verifier/disposition cycle).
  5. Plan Consolidation absorbs Hardening Notes into the Plan body at configured triggers; bumps Plan version; prior version remains queryable.
  6. Dependency graph is queryable: `anvil-graph` resolves transitive dependencies, transitive dependents, and blast radius for a given phase.
- **Dependencies.** P5, P6.
- **Hinge-test list.**
  - `test_planner_contract_required_fields`
  - `test_plan_consolidation_preserves_provenance`
- **Evaluation-metric impact.** Review finding precision, Review rounds per phase: Plan review cycle adds data to both metrics. Deferred-decision resolution rate: hinge tests from Plan validation feed the registry.
- **Estimated rounds-to-convergence.** 3.

---

### **P8 — Build Stage Pipeline (Per-Phase Loop)**

- **Goal.** Per-phase Build loop: Coder implementation, Phase Review Briefing, phase review, gate-approval records.
- **Action list.**
  - `anvil phase build <id>` invokes Coder per phase definition.
  - Phase Review Briefing renderer: Files Changed table, Architecture Compliance table, What to Review section, Test Coverage Summary, How to Activate for Testing runbook, human-facing summary at the top.
  - `anvil phase review <id>` reuses review machinery; sends briefing + code + Plan section to the next reviewer in rotation.
  - `anvil phase ship <id>` requires termination condition; exits non-zero with clear message if not met.
  - Gate-approval audit records for all six gate types: briefing sent, findings received, findings curated, disposition rendered, next-reviewer-or-ship decision, phase ship.
  - Finding Verifier runs on phase findings before Coder receives them; each finding tagged grounded / refuted / cannot-be-verified.
- **Deliverable.** Phase build → review → ship works end-to-end via CLI.
- **Acceptance criteria.**
  1. `anvil phase build <id>` invokes the Coder for the specified phase and renders the phase implementation artifact.
  2. Phase Review Briefing contains all six required sections; missing sections block the briefing-sent gate.
  3. `anvil phase review <id>` submits the briefing to the next reviewer in rotation; returns a structured findings packet.
  4. Finding Verifier tags each finding grounded / refuted / cannot-be-verified before Coder receives the packet.
  5. `anvil phase ship <id>` exits non-zero with a named list of unmet conditions if the termination condition is not satisfied.
  6. All six gate-approval audit records created per phase loop; gate check verifies completeness before ship.
  7. Coder renders disposition document with all six required sections (Verification, Disposition table, Files Changed, Corrections, Residual, Reproducibility).
- **Dependencies.** P5, P6, P7.
- **Hinge-test list.**
  - `test_phase_briefing_required_sections`
  - `test_phase_cannot_ship_without_termination`
- **Evaluation-metric impact.** All six metrics now receive Build-phase data. Human minutes per shipped phase is the primary metric driven by this phase. Defect escape rate: Build-phase reviews are the primary defect-catching mechanism; issues not caught here become escapes.
- **Estimated rounds-to-convergence.** 3.

---

### **P9 — Ship + Rollback (Cascading Invalidation)**

- **Goal.** Project ship + rollback with cascading invalidation through dependency graph.
- **Action list.**
  - `anvil ship` requires all phases shipped, performs configured transport.
  - `anvil phase reopen <id>` — Charter / Plan amendment workflow.
  - Cascading invalidation: transitive closure via dependency graph.
  - Blast-radius confirmation: `anvil phase reopen` shows full invalidation set; user explicitly approves before commit.
  - `RollbackEvent` audit records reference re-opened phase and all invalidated dependents.
  - **Rotation reset on rollback (R4 addition).** Re-opening a phase resets the reviewer rotation for that phase to position 0 (first reviewer in the pool). All invalidated dependent phases also reset their rotation. This ensures that a late-stage fix is reviewed by the *full pool's diversity*, not just whichever reviewer happened to be next in the prior rotation. Without this reset, a phase rolled back at rotation position 3 of a 4-reviewer pool would re-ship after one clean pass from reviewer 4 — which trivially satisfies the rotation but misses three-quarters of the diversity the pool exists to provide. The `RollbackEvent` record includes `rotation_reset_phases: string[]` listing the affected phase IDs; the hinge `test_rollback_resets_rotation_on_dependents` enforces the reset semantics.
- **Deliverable.** Project can ship; phases can be re-opened with full transitive invalidation and audit trail.
- **Acceptance criteria.**
  1. `anvil ship` succeeds only when all phases are in shipped state; exits non-zero with a named list of unshipped phases otherwise.
  2. `anvil ship` executes configured transport actions in declared order; transport failure surfaces as a typed error, not a silent no-op.
  3. `anvil phase reopen <id>` computes the transitive closure of dependent phases via `anvil-graph` and displays the full blast radius before committing.
  4. User must explicitly confirm the blast radius; re-open does not commit without confirmation.
  5. Re-opening creates `RollbackEvent` records referencing the re-opened phase and all invalidated dependents; one record per invalidated phase.
  6. `anvil ship` is blocked if any `RollbackEvent` lacks a corresponding re-shipped resolution for the affected phase.
  7. Audit store records remain immutable through rollback; no existing records modified; only new records added.
  8. Charter/Plan amendment workflow triggered by re-open; amendment must converge before re-shipped phases are reviewed.
- **Dependencies.** P8.
- **Hinge-test list.**
  - `test_rollback_transitive_invalidation`
  - `test_audit_store_immutable_through_rollback`
  - `test_rollback_resets_rotation_on_dependents`
- **Evaluation-metric impact.** Defect escape rate: rollback events triggered after Ship are escape events and are tracked as such. Human minutes per shipped phase: ship transport time included.
- **Estimated rounds-to-convergence.** 2.

---

### **P10a — Evaluation Criteria Infrastructure**

- **Goal.** Three-layer Evaluation Criteria system: metric collectors, Layer-2 project-target evaluation, Layer-3 alert engine, and CLI surfaces.
- **Action list.**
  - Metric collectors for the six Layer-1 metrics. Data sources are audit-store records: `reviewer-finding-packet` (precision, agreement), `gate-approval` (human minutes), `phase-disposition` (round count), `hinge-flip` (deferred-decision resolution rate), `rollback-event` (defect escape rate).
  - Layer-2 target evaluation: compare current metric values against the project's numeric thresholds in `anvil.toml`; produce pass/warn/fail per metric.
  - Layer-3 alert engine: rule-based for v1; fires on the four alert kinds named in the Charter (low precision, rising human-minutes trend, extreme cross-reviewer agreement, deferral open >5 phases).
  - `anvil metrics show` — current values with qualitative direction indicators.
  - `anvil metrics history` — per-metric time series across shipped phases.
  - Alerts wired to CLI warnings surfaced at the next gate.
- **Deliverable.** All six Layer-1 metrics computed automatically from audit-store data. Layer-2 targets evaluated per project. Layer-3 alerts fire correctly on the four alert kinds.
- **Acceptance criteria.**
  1. All six Layer-1 metrics computed from audit-store data (no manual entry).
  2. Layer-2 evaluation correctly compares metrics against project thresholds; `anvil metrics show` flags metrics outside their target range.
  3. Layer-3 alerts fire on the four alert kinds named in the Charter.
  4. `anvil metrics show` displays current values with qualitative direction indicators (↑ / ↓ / → ) and target-range status.
  5. `anvil metrics history` shows per-metric values across all shipped phases.
  6. Deferred-Decision Resolution Rate metric reads from `HingeFlip` records produced by P10b; displays correctly even if P10b is not yet complete (reads from audit store, not from P10b runtime).
- **Dependencies.** P2 (audit), P8 (Build phase data). Parallelizable with P9 and P10b.
- **Hinge-test list.**
  - `test_layer_1_metric_count` — pins 6.
  - `test_alert_kinds_count` — pins 4.
- **Evaluation-metric impact.** All six Layer-1 metrics: automated collection infrastructure complete. Qualitative direction indicators operational. Alert engine produces first actionable signals.
- **Estimated rounds-to-convergence.** 2.

---

### **P10b — Hinge-Test Framework**

- **Goal.** Bi-language hinge-test framework, unified registry persisted to audit store, and `anvil hinge` CLI surface.
- **Action list.**
  - `#[hinge_test]` proc-macro (Rust): extracts test name, current pinned value, intended final value, and phase from annotations; emits `HingeFlip` records to the audit store when flipped.
  - `// hinge_test:` doc-comment parser (Go): equivalent extraction at test collection time.
  - Unified registry: merges Rust and Go hinge metadata into a single queryable view persisted to the audit store.
  - `anvil hinge list` — shows all open hinge tests with pinned and intended states, triggering phase, and flip history.
  - `anvil hinge flip <id>` — records a `HingeFlip` audit record with reasoning; updates the registry.
  - Alternative-mechanism support: flagged registry entries for stacks without a test harness (per Charter *Deferred Decisions Are Tracked* invariant).
  - **Registry consensus check (R4 addition).** For hinges that should exist in both languages (typically cross-cutting contract hinges like `test_proto_package_version`, `test_error_class_count`, `test_handshake_required_fields`), the unified registry runs a *consensus check*: same hinge name → same pinned value, same intended value, same phase. **Asymmetric states are `BlockShip` violations.** Failure modes the check catches: same hinge declared in both languages with different pinned values (e.g., Rust says `pins=anvil.v1`, Go says `pins=anvil.v2` — schema drift); same hinge declared with different intended states (one side has accepted a migration the other hasn't); hinge declared in one language but missing from the other when the registry's metadata flags it as a *cross-language* hinge. The check runs as part of `anvil hinge list --strict` and is invoked automatically by the Ship gate; CI runs it on every build.
- **Deliverable.** Hinge tests are first-class queryable objects. Flipping a hinge creates an auditable record. Both Rust and Go hinge metadata are unified in one registry.
- **Acceptance criteria.**
  1. `#[hinge_test]` decorator extracts name, pinned value, intended value, and phase at collection time.
  2. Go `// hinge_test:` parser extracts equivalent metadata.
  3. Bi-language registry merges both without collision; persists across runs.
  4. `anvil hinge list` shows the registry's current state with correct metadata. (Per R3's pin convention, the count is derived from the registry rather than asserted as prose; `anvil hinge list --count` returns the current total.)
  4a. `anvil hinge list --strict` runs the consensus check; asymmetric cross-language hinges are reported as `BlockShip` and exit non-zero.
  5. `anvil hinge flip <id>` creates a `HingeFlip` audit record with non-empty reasoning; exits non-zero if reasoning is empty.
  6. Alternative-mechanism entries (non-test-harness deferred decisions) queryable alongside hinge tests.
- **Dependencies.** P2 (audit). Parallelizable with P9 and P10a.
- **Hinge-test list.**
  - `test_hinge_decorator_metadata_required` (Rust)
  - `test_hinge_comment_metadata_required` (Go)
  - `test_bi_language_registry_merge`
- **Evaluation-metric impact.** Deferred-decision resolution rate: hinge registry fully operational; P10a's metric collector now has complete HingeFlip data to compute the rate.
- **Estimated rounds-to-convergence.** 2.

---

### **P11 — Dogfooding and Documentation**

- **Goal.** Use Anvil v1 CLI to manage Anvil v1.1 design (Charter + Plan for the App addition) and to complete one external pilot project. Resolve all Provisional Locks. Produce docs.
- **Action list.**
  - **Dogfooding:** Run a Charter → Plan cycle on the Anvil v1.1 App design using the v1 CLI. Fix integration gaps surfaced by dogfooding. (Full Build → Ship on v1.1 happens in v1.1 itself.)
  - **External pilot:** Identify and run a small, non-self-referential project through a full Charter → Plan → Build → Ship cycle using the v1 CLI. **Pilot selection rubric (all must hold):**
    - *Scope ceiling:* the pilot's Plan has between 3 and 7 phases. Larger plans become unbounded; smaller plans don't exercise multi-phase rotation.
    - *Timebox:* 14 calendar days from `anvil setup` to `anvil ship`. If the pilot exceeds the timebox, the pilot's *current state* is the ship gate — partial completion is acceptable evidence; full Ship is preferred but not required.
    - *External user:* the Coordinator may operate the CLI for the pilot, but the *project being built* must originate from someone other than the Coordinator (a friend's tool, a community-suggested utility, a documentation site for an unrelated topic). The purpose is to validate Anvil works on someone else's problem domain.
    - *Domain unrelated to Anvil:* the pilot must not be about workflow tools, AI coding assistants, or developer productivity. Cross-domain stress is the whole point.
    - *Provider diversity stress (R4 addition):* the pilot must use at least two distinct provider connections from at least two distinct provider *types* (where "type" means: Anthropic direct, OpenAI direct, Google AI Studio direct, or — once they ship — Azure OpenAI, AWS Bedrock, Vertex AI, etc.). The point is to exercise the multi-provider adapter abstraction under non-self-referential conditions: a Claude-only pilot does not validate that the provider adapter layer works against other vendors' real API behaviors. If the pilot's reviewer pool already crosses provider types (typical case: Coder is Claude direct + reviewers are GPT direct + Gemini direct), this requirement is met by the existing pool configuration; no additional setup needed.
    - *Failure-class triage:* failures during the pilot are classified before they affect v1 satisfaction. **Pilot-blocking** (must fix before v1 ships): the workflow cannot complete a phase, the audit store loses integrity, the diversity floor is bypassed, the Cross-Reference Integrity check produces false positives or false negatives, *provider-diversity stress fails (e.g., one provider's adapter produces consistently malformed responses that the contract should have caught but didn't)*. **Pilot-informing but not blocking** (logged as v1.x issues): UX friction, suboptimal phrasing in CLI output, missing-but-not-critical commands, performance below targets.
  - The pilot's artifacts (Charter, Plan, dispositions, hardening history, audit-store records) are preserved as a *worked example* in `docs/examples/external-pilot/` for future users. Sensitive parts (if any) are redacted; the workflow itself is left visible.
  - **CLI UX audit:** Walk through each `anvil <resource> <verb>` command and document how it would map to an equivalent App UI action. Flag any command patterns that would not map cleanly (e.g., multi-step commands that imply a single-screen flow, or flags that have no obvious GUI analogue). The audit output is stored as a structured document in `docs/ux-audit.md` and becomes a primary input to the two "v1.1 prep" Provisional Lock reviews (Setup Wizard ordering and CLI command structure).
  - Convert each Provisional Lock to Final or revise (with new audit record). The two "v1.1 prep" locks (Setup Wizard ordering, CLI command structure) are reviewed against the CLI UX audit output and actual v1 usage; either confirmed or revised based on what the v1.1 App design needs.
  - Write `docs/runbook.md`: CLI operational guide covering all six gate operations.
  - Write `docs/onboarding.md`: getting-started for a new user.
  - Write `docs/contract.md`: sidecar contract documentation.
- **Deliverable.** Anvil v1 has managed at least one Charter → Plan cycle for v1.1 via its own CLI, and at least one external project through a full Charter → Plan → Build → Ship cycle. Documentation exists. All Provisional Locks resolved.
- **Acceptance criteria.**
  1. At least one Charter → Plan cycle completes via `anvil` CLI alone for the Anvil v1.1 App design.
  2. At least one external, non-self-referential project completes a full Charter → Plan → Build → Ship cycle via `anvil` CLI alone.
  3. The external pilot includes at least one Build phase that goes through multi-reviewer rotation.
  4. Every Provisional Lock confirmed (→ Final) or revised (with new audit record). No outstanding Provisional Locks at P11 ship.
  5. Runbook covers all six gate operations.
  6. Onboarding guide can be followed by a new user without consulting the runbook.
  7. The v1.1 Plan that comes out of dogfooding is the input for the v1.1 App design.
- **Dependencies.** All prior phases.
- **Hinge-test list.**
  - `test_no_outstanding_provisional_locks_after_dogfooding`
- **Evaluation-metric impact.** Deferred-decision resolution rate: all Provisional Locks resolved; rate reaches 100% for v1 hinge tests. Baseline values for all six metrics established from v1 usage; used to validate or revise Layer-2 numeric targets (see *Evaluation Metric Targets*).
- **Estimated rounds-to-convergence.** 3.

---

## Phase Dependency Graph

```
P0 — Bootstrap
  └── P1 — Config and Charter Loader
        └── P2 — Audit Store and Provenance Graph
              └── P3a — Contract Definition
                    ├── P3b — Rust Sidecar Client  ← ∥ with P3c
                    └── P3c — Go Sidecar             ← ∥ with P3b
                          └── (both required for) P4 — Interactive CLI Setup Wizard
                                └── P5 — Charter Stage Pipeline (Single Reviewer)
                                      └── P6 — Multi-Reviewer Rotation + Convergence
                                            └── P7 — Plan Stage Pipeline
                                                  └── P8 — Build Stage Pipeline
                                                        ├── P9  — Ship + Rollback          ← ∥
                                                        ├── P10a — Evaluation Infrastructure ← ∥
                                                        └── P10b — Hinge-Test Framework      ← ∥
                                                               └── P11 — Dogfooding + Docs
```

**Critical path:** P0 → P1 → P2 → P3a → (P3b ∥ P3c) → P4 → P5 → P6 → P7 → P8 → P11. P9, P10a, and P10b are parallel branches after P8; P11 requires all three.

---

## Cross-Cutting Concerns

- **Audit store record schemas** (P2; Rust types; versioned).
- **Hinge-test convention** spans Rust and Go (P0 stubs; P10 unifies).
- **Sidecar lifecycle** — workspace-scoped daemon, CLI-managed. Spawn logic lives in `anvil-core` so CLI and future App share the same implementation. See *Locked Required Project-Level Choices* for the full spec. **Multi-workspace behavior:** v1's design is single-active-project, but running `anvil` from multiple workspace directories is *supported-but-uncoordinated*, not unsupported. Each workspace spawns an independent daemon with its own PID/port files; daemons do not coordinate. On second-workspace activation, the CLI emits a visible warning naming the other active workspaces (detected via sibling `.anvil/run/` directories under the user's projects root) and the per-workspace resource footprint. The warning is informational; it does not block. Global sidecar sharing (one daemon serving multiple workspaces, with coordinated rate-limiting and shared connection pools) is a post-v1 consideration; see *Open Items*. Running the *same workspace* from two `anvil` processes simultaneously remains hard-blocked by file-system locking (P4's `test_workspace_lock_enforced`).
- **Trust-boundary invariants** — three rules locked at Plan level (no partial commit, stateless sidecar, App not on trust boundary). See *Plan-Level Trust-Boundary Invariants*. Enforced by `anvil-core`; not bypassable by CLI or App surface.
- **App-compatibility constraints** — eight v1 decisions made explicitly for App coexistence. See *App-Compatibility Design Decisions*.
- **CLI command structure** — verb-resource pattern (`anvil <resource> <verb>`). **Provisionally Locked** with `revision trigger = v1.1 App design begins`.
- **`anvil init` vs `anvil setup`** — distinct commands. `anvil init` is idempotent scaffolding; `anvil setup` is the interactive wizard. Locked in Draft 5.
- **Configuration storage** — TOML (`anvil.toml`), with encrypted-at-rest secrets via OS keychain.
- **Cross-reference keys** — Rust algorithm; stable across renderings.
- **UTF-8 and Pre-Flight Environment Check** — P2 audit-store lint; Pre-Flight wraps `anvil` CLI entry.
- **Protobuf schema versioning** — `anvil.v1` current; version handshake mandatory.
- **File-system locking** — in place from v1 so the v1.1 App can coexist without rework.
- **Vault API design for App-consumability** — `anvil-core` exposes a clean command/query API throughout v1; CLI is one consumer; the v1.1 App will be another. No terminal-specific assumptions leak in.
- **Build orchestration** — `justfile` as single entry point.
- **Headless / non-interactive operation.** Every interactive gate has a corresponding non-interactive path so scripted runs and CI flows are possible. Specifically: (a) the wizard accepts `--headless` and reads API keys from `ANVIL_API_KEY_*` env vars (already specified in P4); (b) every approval gate (`anvil charter approve`, `anvil phase approve`, `anvil ship`) accepts `--yes` to skip interactive confirmation, paired with `--reason "<text>"` to record the auto-approval reasoning to the audit store; (c) `--dry-run` is supported on every command that produces audit-store side effects, returning the would-be record(s) on stdout in structured JSON without writing them; (d) non-zero exit codes per command class are documented (0 success, 1 user error, 2 gate refused, 3 sidecar error, 4 audit-store integrity failure, 5 invariant violation); (e) `--format json` is supported on every read command. Headless flows produce identical audit-store records to interactive flows except for the per-step approval source (interactive: `coordinator-interactive`; headless: `coordinator-headless-with-reason`).
- **Error class taxonomy** — six classes defined in P3a.
- **Version display.** The running `anvil` version is surfaced consistently across all interactive CLI surfaces: `anvil --version` (established P0), the `anvil setup` wizard header and confirmation summary (P4), `anvil status` output (P6), and the `anvil phase build` / `anvil phase review` interactive prompts (P8). The version string is embedded at compile time from the Cargo workspace version. Sidecar version is displayed alongside the CLI version where relevant (e.g., `anvil status`). Every `--format json` response envelope includes an `anvil_version` field.
- **Model / provider cost controls.** Every sidecar invocation emits a `RotationLog` record that includes token counts (input + output, where the provider reports them) and a per-call cost estimate (computed from provider-published rates, configured per provider connection). P10a's metrics infrastructure aggregates these into per-phase, per-project, and per-rotation cost totals visible via `anvil metrics show`. **Hard stops:** the project config (`anvil.toml`) supports optional `cost_limits` block — `per_invocation_usd_max`, `per_phase_usd_max`, `project_usd_max`. When a configured limit would be exceeded by the next invocation, the Vault halts the workflow and surfaces a confirmation gate to the human (or returns exit code 2 in headless mode). Cost limits are advisory in v1 (warn-only by default; hard-stop requires explicit opt-in via `cost_limits.enforce = true`); v1.x may evolve the policy based on P11 + pilot usage.

---

## Deferred-Decision Registry (v1 hinge tests)

| Hinge | Language | Triggering Phase |
|---|---|---|
| `test_rust_toolchain_version_floor` | Rust | P0 |
| `test_go_toolchain_version_floor` | Go | P0 |
| `test_cli_entry_point_exists` | Rust | P0 |
| `test_sidecar_entry_point_exists` | Go | P0 |
| `test_required_choices_count` | Rust | P1 |
| `test_project_layout_directories` | Rust | P1 |
| `test_audit_store_required_types_present` | Rust | P2 |
| `test_rollback_resets_rotation_on_dependents` | Rust | P9 |
| `test_append_only_api_has_no_update_or_delete` | Rust | P2 |
| `test_append_only_filesystem_o_excl` | Rust | P2 |
| `test_cross_reference_key_stability` | Rust | P2 |
| `test_audit_store_detects_deleted_records` | Rust | P2 |
| `test_proto_package_version` | Build | P3a |
| `test_error_class_count` | Rust + Go | P3a |
| `test_handshake_required_fields` | Rust + Go | P3a |
| `test_handshake_refuses_no_version_overlap` | Rust | P3b |
| `test_schema_violation_classification` | Rust | P3b |
| `test_transport_failure_retries` | Rust | P3b |
| `test_partial_output_discarded_on_streaming_error` | Rust | P3b |
| `test_v1_minimum_provider_adapters` | Go | P3c |
| `test_provider_adapter_interface_extensibility` | Go | P3c |
| `test_error_class_mapping_anthropic` | Go | P3c |
| `test_error_class_mapping_openai` | Go | P3c |
| `test_error_class_mapping_google` | Go | P3c |
| `test_streaming_aborts_on_error_no_continuation` | Go | P3c |
| `test_wizard_step_count` | Rust | P4 |
| `test_diversity_policy_validation_rejects_same_family` | Rust | P4 |
| `test_workspace_lock_enforced` | Rust | P4 |
| `test_api_keys_encrypted_at_rest` | Rust | P4 |
| `test_wizard_cancellation_leaves_no_partial_state` | Rust | P4 |
| `test_api_keys_env_var_bypass_works_headless` | Rust | P4 |
| `test_findings_packet_schema` | Rust | P5 |
| `test_disposition_doc_required_sections` | Rust | P5 |
| `test_curation_audit_record_required` | Rust | P5 |
| `test_round_limit_default_is_5` | Rust | P6 |
| `test_severity_tiering_at_round_6` | Rust | P6 |
| `test_planner_contract_required_fields` | Rust | P7 |
| `test_plan_consolidation_preserves_provenance` | Rust | P7 |
| `test_phase_briefing_required_sections` | Rust | P8 |
| `test_phase_cannot_ship_without_termination` | Rust | P8 |
| `test_rollback_transitive_invalidation` | Rust | P9 |
| `test_audit_store_immutable_through_rollback` | Rust | P9 |
| `test_layer_1_metric_count` | Rust | P10a |
| `test_alert_kinds_count` | Rust | P10a |
| `test_hinge_decorator_metadata_required` | Rust | P10b |
| `test_hinge_comment_metadata_required` | Go | P10b |
| `test_bi_language_registry_merge` | Rust | P10b |
| `test_no_outstanding_provisional_locks_after_dogfooding` | Rust | P11 |

**Total hinges:** the table above is the canonical list. The count is derived from the table at validation time (`anvil hinge list --count`) rather than restated as prose, so it cannot drift. Prior drafts hard-coded counts in two places (here and in *Evaluation Metric Targets*); those references are removed in favor of "the canonical registry" so a hinge addition or removal does not require synchronized prose edits.

**Pin convention.** Hinge tests fall into two categories:

- *Constitutional pins:* the pinned value is fixed by a Charter invariant or Charter-amendment commitment. Examples: `test_audit_store_record_types_count` (pins 11; tied to *Audit-Store Minimum Schema* invariant), `test_error_class_count` (pins 6; tied to the contract's error-class enum), `test_proto_package_version` (pins `anvil.v1`; tied to schema-versioning policy). These remain *exact-equality* tests because flipping the count would mean the Charter or contract is changing — which is itself the signal the test is meant to produce.
- *Operational pins:* the pinned value is the current implementation choice but may grow without re-opening the Charter. Examples: `test_wizard_step_count`, `test_v1_minimum_provider_adapters`, `test_required_choices_count`. These should be *minimum-equality* tests (`assert count >= N`) paired with a registry entry naming the current value, so legitimate additions do not require synchronized prose edits across the Plan. P10b's hinge-framework implementation enforces this convention: the hinge proc-macro accepts a `style: "exact" | "minimum"` attribute; constitutional pins use `"exact"`, operational pins use `"minimum"`. Drift is detected by comparing the macro-emitted manifest against the Charter's constitutional list.

---

## Evaluation Metric Targets (Layer 2 — provisional)

These are the numeric thresholds for Anvil's own project (Layer 2 in the three-layer Evaluation system). They are provisional until P11 dogfooding and external pilot produce observational baselines; they will be confirmed or revised at that point.

| Metric | Target | Alert Threshold | Notes |
|---|---|---|---|
| Defect escape rate | 0 P1 defects post-Ship for any phase | Any P1 escape triggers re-open | P2/P3 escape budget: ≤2 across all phases for v1 (provisional) |
| Review finding precision | ≥70% grounded findings leading to Coder action | <50% sustained over 2 consecutive phases | Per Charter Layer-1 qualitative floor |
| Human minutes per shipped phase | ≤90 minutes average across Build phases | >150 min for 2 consecutive phases | Bootstrap phases (P0–P3c) excluded from average; setup (P4) tracked separately |
| Review rounds per phase | ≤3 rounds average; target ≤2 for foundation phases | ≥5 rounds for any single phase | Alert triggers Phase Review Briefing quality review |
| Cross-reviewer agreement | 30–60% agreement on finding classes | <15% or >80% for any artifact | Bimodal diagnostic per Charter; alert triggers pool-configuration review |
| Deferred-decision resolution rate | ≥90% of hinge tests resolved within 2 phases of creation | Any deferral open >5 phases at Ship | Total v1 hinge count is derived from the *Deferred-Decision Registry* above (canonical list); target at P11 ship is zero open hinges that have not been explicitly converted to long-lived registry entries with rationale. |

All thresholds are provisional. P11 dogfooding and external pilot produce the first real baselines; confirmed thresholds replace these before the Plan is declared fully satisfied.

---

## Plan-Specific Risks

- **Risk: Phase 4 underestimates integration complexity.** P4 integrates Vault + CLI + sidecar + encrypted secrets + seven-step wizard. *Mitigation:* allow 3 rounds of convergence; willing to split during review if acceptance criteria are not met.

- **Risk: Audit-store schema rigidity.** The 11 record types are hinge-tested with a count assertion (`test_audit_store_record_types_count`). If a new record type is needed during Build, adding it requires a deliberate hinge-flip — a controlled decision, not a silent migration. Extensions add new types without modifying existing schemas; append-only semantics make backward compatibility tractable. Risk: if a necessary type is missed at design time, the hinge-flip process is a mild friction that could delay a phase.

- **Risk: Dogfooding loop in P11.** Scoped to Charter → Plan cycle on v1.1 design, not the full Build → Ship. The external pilot fills the gap: it exercises Build → Ship on a real project. *Mitigation:* if the pilot surfaces workflow gaps, they are fixed before P11 ships.

- **Risk: Single-coder voice-consistency cost.** All human-facing artifacts are rendered by the same Coder (Claude). Over a multi-month v1 build, the pinned model may be updated or deprecated. *Mitigation:* Coder model is pinned per the Coder Model Pinning invariant; upgrade requires Charter amendment. The risk is that the pinned model becomes unavailable, forcing a Charter amendment at an awkward time. *Secondary mitigation:* audit store preserves all artifacts; a model migration can render new artifacts against existing records without rewriting history.

- **Risk: Cross-language development burden.** Two languages (Rust + Go); TypeScript / Tauri are v1.1 concerns. *Mitigation:* `justfile` single command surface; P0 establishes lint and test infrastructure for both languages before any feature work.

- **Risk: Protobuf schema evolution drift.** The `anvil.v1` proto schema must remain stable across the CLI-sidecar boundary. If Vault or sidecar evolves contract assumptions without bumping the schema version, silent incompatibilities arise. *Mitigation:* `test_proto_package_version` hinge test; mandatory version handshake on connect; `just gen` regenerates both sides atomically; breaking changes require a version bump.

- **Risk: Sidecar-becomes-second-brain.** The Go sidecar is stateless by design, but vendor API session contexts, streaming state, or retry state could leak across invocations if adapter implementations accumulate state inadvertently. *Mitigation:* Plan-Level Trust-Boundary Invariant #2 locks statelessness; the `anvil-sidecar-client` discards all state on connection close; `test_partial_output_discarded_on_streaming_error` guards a critical case. The daemon lifecycle (idle-timeout auto-exit) provides a natural state-reset boundary.

- **Risk: Vendor adapter brittleness.** Vendor APIs change response schemas, add error codes, deprecate endpoints, and change streaming behavior. Raw HTTP adapters absorb this churn directly. *Mitigation:* error class mapping is hinge-tested per adapter; vendor API test fixtures (recorded responses) used for contract-conformance tests; the `ErrorClass` taxonomy is stable even if vendor error codes change beneath it.

- **Risk: v1 → v1.1 transition harder than planned.** The v1 architecture is designed to make the App addition non-disruptive, but architectural assumptions made in v1 may not survive contact with real App requirements. *Mitigation:* two "v1.1 prep" Provisional Locks force re-evaluation at the v1 → v1.1 boundary; the eight decisions in *App-Compatibility Design Decisions* are explicitly named so they can be deliberated against rather than discovered.

- **Risk: CLI usability ceiling.** A CLI is inherently harder to make accessible to non-expert users. v1 will reach a smaller, more expert audience by design. *Mitigation:* this is intentional for v1; the audience is exactly the one most likely to surface real workflow issues. The external pilot in P11 validates that the CLI is usable on a real project by a real user. v1.1 broadens the audience via the App.

- **Risk: External pilot scope creep in P11.** The external pilot is added to strengthen acceptance criteria, but if it reveals deep workflow gaps, P11 could balloon. *Mitigation:* the pilot project is explicitly scoped as *small and non-self-referential*; the acceptance criterion is one full cycle, not a production deployment. Gaps found in the pilot are fixed in P11 but do not re-open earlier phases unless a P1-level issue is found.

- **Risk: Partial-output discard cost in long streams.** Plan-Level Trust-Boundary Invariant #1 mandates discarding all partial output on mid-stream sidecar error. For long Charter renderings or large Build-phase outputs, discarding a nearly-complete stream due to a transient network blip wastes LLM tokens and requires a full restart. *Mitigation:* the invariant is load-bearing and is not relaxed in v1 — partial output that cannot be verified as structurally complete is not safer because it is long. Retry/backoff for Transport failures (P3b) catches transient blips before they become stream aborts. *Post-v1:* if P11 observational data shows this is a frequent pain point, a checkpoint/resume mechanism can be designed for v1.1 (see *Open Items*) with appropriate verification semantics.

- **Risk: Multi-workspace sidecar resource accumulation.** v1's single-active-project model assumes one active workspace at a time. A user running `anvil` from multiple workspace directories simultaneously would spawn independent daemons with no global coordination, potentially accumulating log files and idle processes. *Mitigation:* the idle-timeout auto-exit (default 30 min) bounds resource accumulation; each daemon is lightweight (no model weights; vendor calls are HTTP). *Post-v1:* global sidecar sharing (one daemon serving multiple workspaces) is a recognized improvement; see *Open Items*.

---

## Open Items (Plan Stage)

- **Audit store query language.** `anvil audit list` and `anvil audit show` are sufficient for v1. A structured query language (SQL-like or a custom DSL) enabling cross-record queries is deferred to post-v1. Status: open, deferred beyond v1 scope.

- **Concurrent project support.** v1 is single-active-project by design; the storage layer may hold multiple projects. Workspace-level file-system locking (P4 hinge `test_workspace_lock_enforced`) prevents concurrent writes. True multi-project parallelism (multiple active projects simultaneously) is out of v1 scope. Status: not open — decided as out of scope.

- **Reviewer prompt management.** Prompts sent to reviewers are constructed by the Vault with fixed templates in v1. Per-project prompt customization (editable templates, version-controlled prompts) is deferred. Status: open, deferred to v1.1.

- **Reviewer findings deduplication.** When multiple reviewers raise semantically identical findings, deduplication is currently manual (human curation step). Automated deduplication (semantic similarity, clustering) is deferred. Status: open, deferred to v1.1.

- **Performance characterization.** No per-operation latency budgets are set in v1. Latency is dominated by model API calls. Baseline performance data will come from P11 dogfooding and pilot. Status: open, to be characterized after P11.

- **Distribution.** v1 ships as `cargo install` for Anvil's own development, plus release binaries for users. v1.1 transitions to installable desktop bundles (Tauri-produced). **v1 release acceptance (locked, was previously open):**
  - *Target platforms:* Windows x64 (primary — Coordinator's platform), macOS aarch64 + x64 (stretch), Linux x64 musl-static (stretch). Primary must ship; stretches are best-effort but their absence does not block v1.
  - *Install method:* per-platform release archive (`.zip` Windows, `.tar.gz` macOS / Linux) containing both `anvil` and `anvil-sidecar` binaries plus a top-level `INSTALL.md`. No platform-specific installer (`.msi` / `.dmg` / `.deb`) in v1; those land in v1.1 alongside the Tauri bundles.
  - *Signing and checksums:* SHA-256 checksums published alongside every release archive in a `SHA256SUMS.txt` file, signed via the project's GPG key (also used for the security-disclosure policy). No code-signing certificates for Windows or macOS in v1 — release notes document the expected unsigned-binary warnings on each OS and how to verify checksums manually.
  - *Release artifact layout:* `anvil-v1.<minor>.<patch>-<platform>.{zip,tar.gz}` + `SHA256SUMS.txt` + `SHA256SUMS.txt.asc` + `RELEASE_NOTES.md`. All four published to the GitHub Releases page atomically.
  - *Smoke tests:* every release candidate runs a scripted smoke test against the release archive (extract, run `anvil --version`, run `anvil-sidecar --version`, run `anvil init` in a temp dir, run `anvil setup --headless` with test credentials, run `anvil charter render` on a fixture Charter, verify the rendered output matches expected hash). The smoke-test script is itself a v1 deliverable in P11. **The smoke test must explicitly verify the exact text of the unsigned-binary warning each OS displays on first run** (Windows SmartScreen, macOS Gatekeeper, Linux distribution-specific warnings), so users encountering the warning in the wild see language that matches the runbook's expectations.
  - *Windows-specific daemon robustness (R5 addition; Windows is the primary platform).* The P11 smoke-test list includes Windows-only scenarios for the sidecar daemon:
    1. *User logoff:* daemon launched, user logs off, user logs back in — daemon either survives (preferred) or is detected as stale and cleaned by the next `anvil` invocation. Either is acceptable; the failure mode to prevent is "daemon is zombie-running but not reachable."
    2. *Laptop close-lid / sleep:* daemon launched, system enters sleep (>30 min), system resumes — daemon either survives or is detected as stale and cleaned. Same acceptance.
    3. *Fast user switching:* daemon launched under user A, switch to user B, switch back — daemon is workspace-scoped, so it remains accessible to user A; user B's `anvil` invocations in user B's workspaces do not see user A's daemon.
    4. *Antivirus quarantine:* if the `anvil-sidecar` binary is quarantined by Windows Defender or third-party AV mid-run, the next `anvil` invocation surfaces a clear typed error (`AdapterBug` with details about the missing binary) rather than hanging or silently retrying. The runbook documents how to add Anvil to the AV exclusion list.
    5. *Ungraceful terminal close:* daemon launched, terminal window is closed (X button) without `Ctrl-C` — the daemon's parent-process detection notes the orphan state on next heartbeat and exits within one heartbeat interval (60 seconds). The global stale-daemon sweep catches any daemon that fails to detect orphan state within 2× idle-timeout.
  These scenarios are first-week-support-ticket risk areas; addressing them in P11 smoke-tests prevents the most likely class of user-visible failures.

- **Auto-update.** Out of v1 scope.

- **Provider adapter expansion roadmap.** v1 ships three direct-API adapters (Anthropic, OpenAI, Google AI Studio) — the minimum set required to satisfy the family-floor invariant with Claude as Coder. The architecture supports cloud-hosted variants (Azure OpenAI, AWS Bedrock, Google Vertex AI) and additional direct APIs (xAI) as additive adapters that do not modify Vault, contract, or existing adapters. Gateways (DigitalOcean Gradient, OpenRouter) are likewise additive. Which adapters ship in v1.1 vs. later is informed by P11 dogfooding and pilot feedback on users' actual provider preferences. The family-floor invariant is unaffected by adapter expansion because it operates on model identity, not on access path. Status: open, prioritization deferred to post-P11 evidence.

- **External pilot project selection.** The specific project used for the P11 external pilot is not pre-selected in the Plan; it is chosen during P11 based on what is available and small enough to complete within the phase. Status: open, resolved in P11.

(Two prior Open Items — *Checkpoint/resume for long sidecar streams* and *Global sidecar sharing across workspaces* — were promoted in R5 to the new *v1.1 Design Seeds* appendix below, where they are documented as forward design work with explicit constraints rather than vague post-v1 notes.)

---

## v1.1 Design Seeds

This appendix collects items that are explicitly *not* v1 scope but are anticipated v1.1 design work. Each is recorded here with the problem statement, the constraints established at Plan-level (which the v1.1 design must preserve), and the v1 data points expected to inform the design (typically: observational data from P11 dogfooding and pilot). The intent is that these survive as deliberate design seeds rather than as undifferentiated post-v1 notes.

### Seed 1: Checkpoint/resume for long sidecar streams

**Problem statement.** Plan-Level Trust-Boundary Invariant #1 mandates full discard of accumulated stream state on any mid-stream `Error`. The cost is a full re-invocation when a long generation (multi-thousand-token Charter or Plan render) fails late in the stream — wasted tokens, wasted latency, observable user pain after retry/backoff exhausts.

**Constraints any v1.1 design must preserve.**

- The trust-boundary invariant is not abandoned. *"No commit on partial output"* remains constitutional.
- Any partial-stream preservation must define a *structural soundness* check — e.g., the partial output ends at a syntactically complete boundary (token-boundary alone is not sufficient; the system needs a semantic stopping point like an end-of-section marker or a verified-parseable prefix). A partial stream that fails the structural check is discarded as before.
- The preserved partial state must be auditable — the audit-store carries a `PartialStreamCheckpoint` record (proposed name) that names what was preserved, what the structural-soundness check was, and what the resume request will request *next* (not a re-do).
- Resume is not "retry from token N" — it is "given this preserved prefix, generate the remaining portion under a fresh idempotency key." This keeps the contract surface clean.

**v1 data points that will inform v1.1.** P11 must record observed mid-stream error rates on Charter/Plan rendering invocations, the typical token-position distribution of those errors, and the user-visible cost (wall-clock seconds wasted, re-invocation token count). If the rate is rare and the cost is small, the seed may not need v1.1 implementation. If the rate is common (e.g., >5% of long renders), v1.1 design begins.

### Seed 2: Global sidecar sharing across workspaces

**Problem statement.** v1's daemon is workspace-scoped. Multiple active workspaces each spawn their own daemon, accumulating resource overhead (one Go process per workspace, separate connection pools, separate provider-config copies, separate logs). The R4-added stale-daemon registry mitigates the *visibility* problem; it does not solve the *coordination* problem.

**Constraints any v1.1 design must preserve.**

- The Adversarial Diversity floor still applies per workspace. A shared daemon must respect per-workspace reviewer pool configurations; it cannot collapse credentials or models across workspaces.
- Per-workspace API key isolation. A shared daemon must namespace credentials by workspace; no leakage of credentials between workspaces.
- Per-workspace audit trail. Any record the shared daemon produces must be attributable to a specific workspace via the `provider_connection_id` or equivalent cross-reference.
- Backward-compatibility. v1 workspaces (workspace-scoped daemons) must continue to work alongside v1.1's shared-daemon mode. The migration must be opt-in, not forced.

**v1 data points that will inform v1.1.** P11 must record the actual multi-workspace usage patterns of the Coordinator's own development: how many workspaces are active simultaneously, what the daemon resource cost is (memory + connection count), how often the stale-daemon sweep cleans entries. If multi-workspace operation is rare, the seed may stay as a seed indefinitely. If common, v1.1 design begins.

### Seed 3: Cryptographic tamper-proofing of audit store (from R3)

**Problem statement.** The v1 audit store provides *local tamper detection* (index-vs-disk completeness check) but not *adversarial tamper-proofing*. An attacker who can modify both an audit-store file and its index entry defeats the local check.

**Constraints any v1.1 design must preserve.**

- Append-only operational semantics remain the floor. Cryptographic mechanisms add a layer; they do not relax the existing rule.
- Cryptographic mechanisms must not require always-online operation. Anvil is local-first; signed manifests or chained hashes must be computable and verifiable offline.
- Performance budget: the integrity check must remain `O(records)` at worst, not `O(records²)` or worse.

**v1 data points that will inform v1.1.** Whether any user surfaces a real adversarial-tampering scenario during v1 use. If users are operating Anvil in adversarial environments (shared workstations, untrusted contributors with filesystem write access), the seed moves forward. If users operate in trusted environments, the seed remains seeded.

### Seed 4: Reconsidering file-based credential encryption (from R4)

**Problem statement.** v1 removed file-based credential encryption with user passphrase as a fragile security surface. Users on no-keychain systems are forced to env-var-only mode, which may be too friction-heavy for daily use.

**Constraints any v1.1 design must preserve.**

- If file-based encryption returns in v1.1, it must use established libraries (e.g., `age`-rs or a vetted equivalent) rather than a homegrown scheme.
- The threat model must be explicitly documented (what it defends against; what it does not).
- The env-var-only mode remains the floor for users who decline the file-based option.

**v1 data points that will inform v1.1.** P11 and post-v1 user feedback on how many users actually hit the no-keychain case, and how many of those find the env-var floor too friction-heavy. If the case is rare, the seed stays seeded.

### Seed 5: Hard-stop cost-limit policy evolution

**Problem statement.** v1 ships cost limits as warn-only by default; users must opt into `cost_limits.enforce = true` for hard-stop behavior. This is the safe v1 default but may not match power-user expectations.

**Constraints any v1.1 design must preserve.**

- Cost-limit enforcement is per-project configurable, not global.
- Hard-stop must be overridable in-session by the Coordinator via an explicit `--override-cost-limit --reason "<text>"` flag, with audit-record logging.
- The Coordinator must be the human, not a model. No automated agent should be able to bypass cost limits.

**v1 data points that will inform v1.1.** Observed pattern: do users hit cost limits often? Do they want hard-stops, warn-only, or a graduated policy (warn at X%, soft-stop at Y%, hard-stop at Z%)? Data from P11 and pilot usage.

### Seed 6: Logo and tagline

**Problem statement.** Anvil v1 is CLI-only and needs no visual logo. The v1.1 App requires a visual identity — logo and tagline — before public launch. These are creative decisions that need a dedicated discussion with the Coordinator; they cannot be auto-generated or deferred to mid-App-build without losing the chance to shape the App design around them.

**What must happen before v1.1 App design begins.**
- Discuss and agree on the Anvil tagline (a refinement of *structure for vibe coding* or a departure, depending on how v1 usage shapes the product story).
- Commission or design the Anvil logo, consistent with the chosen tagline and the "workshop / craft / iteration" framing from the Charter name rationale (*a place where work is shaped through deliberate, repeated hammering*).
- Decide where logo and tagline appear: App splash, window title, README header, release page, CLI `--version` banner (optional).

**Constraints established at Plan level.**
- The Charter's workshop/craft framing is the intended semantic anchor for any visual identity work.
- The positioning anchor from *Product Positioning* (*structure for vibe coding*; broader reach via failure-modes framing) applies to tagline candidate evaluation.
- Trademark Posture A (no registration; naming-preference statement in README) is locked; visual identity decisions must be consistent with that posture.

**Trigger.** Discuss with Coordinator before v1.1 App design begins. Logo and tagline are inputs to App design, not outputs — deferring past that point creates rework.

---

## Post-Convergence Charter Amendments

The following Plan-level invariants are already locked and enforced in v1. They will be promoted to Charter constitutional level after Plan convergence via a Charter amendment cycle, which is itself reviewable through the full Charter Review process.

1. **"No commit on partial or invalid sidecar output."** Trust-boundary invariant between Vault and sidecar. Locked at Plan level in *Plan-Level Trust-Boundary Invariants*.
2. **"Sidecar must remain stateless across invocations."** Prevents sidecar from becoming a hidden second brain. Locked at Plan level in *Plan-Level Trust-Boundary Invariants*.
3. **"App frontend (when added in v1.1) is not on the trust boundary; Vault enforces all invariants regardless of frontend input."** Locks the trust-model invariant before v1.1 design begins. Locked at Plan level in *Plan-Level Trust-Boundary Invariants*.

All three are enforced now. Charter promotion is a constitutional bookkeeping step, not a behavioral change.

---

## Plan-Level Acceptance Criteria

The Plan is satisfied — and Anvil v1 is ready to ship — when:

1. All 15 phases (P0, P1, P2, P3a, P3b, P3c, P4, P5, P6, P7, P8, P9, P10a, P10b, P11) have shipped per the per-phase acceptance criteria.
2. The dogfooding test in P11 has produced a Charter and Plan for Anvil v1.1 using the v1 CLI alone.
3. At least one external, non-self-referential project has completed a full Charter → Plan → Build → Ship cycle using the v1 CLI alone, including at least one Build phase with multi-reviewer rotation.
4. All Provisional Locks are resolved (confirmed Final or explicitly revised with audit record).
5. The 6 Layer-1 product health metrics are being collected automatically.
6. Layer-2 numeric thresholds for Anvil itself have been confirmed or revised from observed P11 data.
7. Cross-Reference Integrity check passes against all shipped artifacts.
8. `convergence-declaration` log shows the rotation paths actually taken.
9. The Plan has been reviewed by at least two non-Coder reviewers and converged.
10. No outstanding hinge tests block Ship.
11. v1 binaries (`anvil`, `anvil-sidecar`) build correctly for the primary platform (Windows x64); stretch platforms (macOS aarch64/x64, Linux x64 musl-static) ship best-effort. A signed `SHA256SUMS.txt.asc` is published with every release archive. The smoke-test script in *Open Items / Distribution* passes against the primary-platform release candidate before v1 is declared shipped.

---

## Plan Review Process

The Plan (this document) is reviewed by the same reviewer pool configured for the project's Charter Review: Codex-class (OpenAI) + Gemini-class (Google). Neither is Claude, satisfying the Adversarial Diversity floor.

Each review round follows the same cycle as Charter Review:
- Reviewer receives the current Plan, produces a structured findings packet (id, severity P1/P2/P3, location, claim, evidence, recommendation).
- Coordinator curates findings (keep / drop / edit / annotate).
- Finding Verifier grounds each finding (grounded / refuted / cannot-be-verified).
- Coder applies fixes to this Plan document and renders a Disposition Document (`REVIEW_PLAN_R<N>.md`).
- Addressed findings are folded into `PLAN_HARDENING_HISTORY.md` as "Hardening Notes (R<N> — Consolidated)." They do not appear inline in this Plan.
- This Plan is always the current normative text; the history file is the legislative record.

Termination: full-pool clean — both reviewers produce a clean pass on the current state. Convergence safeguards apply (severity-tiering at round 6; human arbiter authority at any round).

The converged Plan becomes the constitutional input to the Build stage. Post-convergence charter amendments (see *Post-Convergence Charter Amendments*) are filed after Plan convergence, each going through its own Charter amendment review cycle.

---

## v1 → v1.1 Transition

This Plan is for **v1 (CLI)**. The v1.1 work — adding the Tauri + React + TypeScript desktop App — is its own Charter + Plan, scoped after v1 ships and after v1 usage produces design evidence for what the App needs to support.

The v1 → v1.1 transition is a deliberate review point, not an implicit re-decision. Two Provisional Locks in v1 carry `revision trigger = v1.1 App design begins`:

- *CLI Setup Wizard step ordering and prompts* — validated against v1 usage feedback before the App wizard is built.
- *CLI command structure* — validated that it maps cleanly to App view structure before the App's IPC is finalized.

Eight decisions in *App-Compatibility Design Decisions* are consciously made for App coexistence. These are the starting constraints for the v1.1 design; they may be revised by the v1.1 Charter process but not unilaterally.

The Vault library (`anvil-core`) is designed throughout v1 with no CLI-shaped assumptions, so the App can consume it directly in v1.1. The sidecar spawn logic lives in `anvil-core` for the same reason.

---

## Bottom Line

Anvil v1 is a **Rust CLI + Go sidecar** focused on the audience most likely to surface real issues with the workflow discipline: experienced developers using CLI surfaces in real workflows. The architecture (Rust Vault + versioned protobuf gRPC + Go sidecar) is the same architecture that will support the v1.1 App; v1 just does not ship the App.

The value prop is **structure for vibe coding** — review gates, provenance, adversarial cross-vendor diversity, and explicit workflow discipline replacing the unstructured agent-loop approach. v1 proves the discipline; v1.1 broadens the audience.

Fourteen phases, mostly linear, with parallelization at P3 (Rust client + Go sidecar) and after P8 (Ship + Eval). The critical path is P0 → P1 → P2 → P3a → (P3b ∥ P3c) → P4 → P5 → P6 → P7 → P8 → P11.

Three trust-boundary invariants are locked at Plan level and will be promoted to Charter constitutional layer after convergence. Seven Provisional Locks remain outstanding; two carry explicit "v1.1 App design begins" revision triggers.

Next step: Plan Review convergence. Draft 5 represents R1 findings applied.
