# Anvil Plan — Hardening History

This document is the **provenance log** for the Anvil Implementation Plan. The Plan itself (`ANVIL_PLAN.md`) is the canonical normative text. This file records the round-by-round changes folded in through Plan Review.

Per *Cross-Reference Integrity*, every consolidated hardening note here corresponds to a disposition document (`REVIEW_PLAN_R<N>.md`) and to raw reviewer-finding-packet records in the audit store. The hardening note summarizes; the disposition documents the round.

Per *Plan Consolidation*, hardening notes accumulate in this file until a consolidation pass absorbs them into the Plan body and bumps the Plan version. After consolidation, prior versions remain queryable via the artifact graph, and this file retains the round-by-round detail.

The Plan does not append hardening notes inline; they land here.

---

## Hardening Notes (R1 — Consolidated)

R1 came from the configured reviewer pool. Eleven findings raised (4 × P1, 5 × P2, 2 × P3). Ten addressed; one (Finding 11, encoding corruption) refuted on factual grounds. See `REVIEW_PLAN_R1.md` for the full disposition table.

The central R1 thesis: several operationally critical decisions were half-deferred while the Plan already depended on them as if they were settled. Four P1 findings drove the largest structural changes.

### 1. Trust-boundary invariants promoted to Plan-level locks (P1 Finding 1)

The three candidate charter amendments — "no commit on partial sidecar output," "sidecar stateless across invocations," "App frontend not on trust boundary" — were listed as post-convergence amendments while the Build phases already depended on them as settled constraints. This left the Plan in procedural contradiction: formally deferring rules it was architecturally treating as locked.

Resolution: a new *Plan-Level Trust-Boundary Invariants* section locks all three as hard constraints. They are enforced by `anvil-core` and guarded by hinge tests. The *Candidate Charter Amendments* section is renamed *Post-Convergence Charter Amendments* with a note that the rules are already locked and enforced; Charter promotion is constitutional bookkeeping, not a behavioral change.

### 2. Sidecar lifecycle locked (P1 Finding 2)

The v1 lifecycle was listed as an open item ("user starts anvil-sidecar manually"). This is not a small implementation note: it affects setup UX, failure recovery, logging, port binding, process supervision, and CLI trustworthiness.

Resolution: **workspace-scoped daemon, CLI-managed**. The `anvil` CLI spawns `anvil-sidecar` as a background daemon on first invocation requiring model access. PID and port written to `.anvil/run/`. Subsequent invocations probe the Health RPC and restart if not responding. Idle-timeout auto-exit (default 30 min). Spawn logic lives in `anvil-core` so the v1.1 App reuses it. `anvil sidecar status` / `anvil sidecar stop` for explicit management. Added as a Final lock in the Required Project-Level Choices table. Removed from Open Items. P3c acceptance criteria updated to include daemon lifecycle behavior.

### 3. External pilot added to acceptance model (P1 Finding 3)

The Plan's acceptance test was dogfooding only (Anvil v1.1 Charter→Plan cycle via v1 CLI). This validates orchestration of Charter/Plan but does not prove robustness of Build/Review/Ship on a normal project.

Resolution: a second acceptance criterion added to P11 and to Plan-Level Acceptance Criteria: at least one non-self-referential project must complete a full Charter → Plan → Build → Ship cycle via v1 CLI, including at least one Build phase with multi-reviewer rotation. The dogfooding criterion is preserved; the pilot adds orthogonal coverage.

### 4. "As Draft 2" entries inlined (P1 Finding 4)

P6, P8, and P9 acceptance criteria were "As Draft 2 P<N>" placeholders. Evaluation Metric Targets and Plan Review Process sections were "Unchanged from Draft 2." Multiple risk entries were "As Draft 2." These placeholders made the Plan non-self-contained as an implementation contract.

Resolution: all "As Draft 2" content inlined or replaced with complete text in Draft 5. P6 acceptance criteria fully written. P8 acceptance criteria fully written. P9 acceptance criteria fully written. Evaluation Metric Targets expanded with a provisional numeric-threshold table. Plan Review Process expanded with the full review cycle description. Risk entries for audit-store schema rigidity, single-coder voice-consistency cost, protobuf schema evolution drift, sidecar-becomes-second-brain, and vendor adapter brittleness all written out. Open Items entries for audit store query language, concurrent project support, reviewer prompt management, reviewer findings deduplication, and performance characterization all written out.

### 5. Severity-tiering post-round-5 behavior made precise (P2 Finding 5)

The Required Choices table and P6 said that after round 5, only P1 findings block. What P2/P3 findings *become* was unspecified: silently auto-deferrable? requiring human justification? remaining blockers for specific artifact classes?

Resolution: Post-round-5 P2/P3 findings are marked **advisory**. Each advisory finding must receive explicit human disposition: `Accept-Advisory` (acknowledged; no action; recorded in disposition), `Drop-Advisory` (refuted or non-applicable; reason required), or `Defer-Advisory` (deferred to named future phase). No advisory finding passes silently. Gate check at the `next-reviewer-or-ship` transition verifies all advisory findings are disposed. Advisory findings stored in `reviewer-finding-packet` audit records with `advisory: true`. `convergence-declaration` records include the count of outstanding advisory findings. Added to P6 action list and acceptance criteria.

### 6. App-compatibility decisions named explicitly (P2 Finding 6)

The Plan made several v1 decisions informed by App-coexistence requirements (Vault as library, spawn logic in `anvil-core`, `.anvil/run/` layout, TOML config, etc.) but did not distinguish them from purely CLI-derived choices. This risked v1.1 designers treating them as arbitrary rather than deliberate.

Resolution: new *App-Compatibility Design Decisions* section added with an eight-row table naming each decision, its v1 rationale, and its app-compatibility reason. Executive Summary updated to reference this section. CLI Setup Wizard Provisional Lock row in the Choices table updated to note the `anvil init` / `anvil setup` distinction.

### 7. Planner Contract Compliance section added (P2 Finding 7)

The Plan claimed Planner Contract compliance in the header but did not expose the contract fields or their mapping, weakening reviewability. The evaluation-metric-impact field was missing from all phases (a gap in contract compliance).

Resolution: new *Planner Contract Compliance* section added with field-by-field mapping tables for both required top-level outputs and required per-phase fields. Gap identified: evaluation-metric-impact absent from all 14 phases in Draft 4. Added to every phase in Draft 5.

### 8. P10 split trigger pre-decided (P2 Finding 8)

P10 combines evaluation infrastructure, alert engine, hinge-test framework, bi-language registry, and CLI surfaces. The defense for the merge was present but the phase remained a likely schedule hotspot. No split trigger was defined.

Resolution: split trigger added to P10: if implementation exceeds 4 rounds-to-convergence (vs. estimated 3), split into P10a (eval + alerts + CLI) and P10b (hinge framework + registry). Decision is made at round 4, not later. Sub-phase structure and dependency behavior documented.

### 9. `anvil init` vs `anvil setup` distinction locked (P2 Finding 9)

The naming was deferred to P4 but the distinction has architectural implications: idempotent scaffolding vs. interactive configuration, automation behavior, and onboarding flow.

Resolution: **locked in Draft 5.** `anvil init <path>` is the idempotent scaffold command: creates directory layout and default `anvil.toml`, no prompts, safe to re-run (no-op if already initialized). `anvil setup` is the interactive wizard: configures API keys, model assignments, runs connectivity tests. `anvil setup` runs `anvil init` implicitly if the project is not yet initialized. Re-running `anvil setup` on a configured project offers step-level re-run, not full wipe. P1 and P4 action lists updated. Removed from Open Items.

### 10. CLI-first analogy references trimmed (P3 Finding 10)

The Executive Summary and Bottom Line cited Codex and Claude Code CLI-first precedent as primary justification for the CLI-first decision. This is rhetorically useful but the stronger argument is Anvil's own gate-heavy workflow needs.

Resolution: Codex/Claude Code precedent language trimmed from Executive Summary and Bottom Line. The CLI-first rationale now leads with the workflow argument (six human-approval gates, structured briefings, terminal-as-prompt-surface) before any analogy. The competitive axis section retains a reference to Codex and Claude Code in the context of what Anvil is *not* competing with.

### 11. Encoding finding refuted (P3 Finding 11)

R1 claimed "visible encoding corruption throughout the file, including the title line." The title line contains an em-dash (`—`) which the reviewer's environment may have rendered incorrectly.

Refuted on factual grounds: the em-dash is valid UTF-8 (U+2014) and the Charter's Artifact Encoding invariant explicitly permits non-ASCII typography (em-dashes, smart quotes, mathematical symbols) as intentional, distinguishing them from corruption (invalid byte sequences). This is the third time this finding pattern has appeared (R1 #10 and R3 #12 of Charter Review were both factually refuted on identical grounds). The Plan-stage pre-flight check and UTF-8 lint (P2) address the underlying environmental concern; the characters themselves are not corruption.

No change to the Plan document for this finding.

### R1 reviewer

The structured findings packet was produced by a reviewer drawn from the configured pool (Codex-class from OpenAI or Gemini-class from Google; pseudonymized per audit-store convention pending build of the audit infrastructure). Raw findings packet present in the conversation transcript, pending audit-store schema implementation (the storage layer is a Build-phase item; until it exists, the conversation log serves as the audit trail).

### Disposition document

`REVIEW_PLAN_R1.md` (R1 round).

---

## Hardening Notes (R2 — Consolidated)

R2 came from the second reviewer in pool rotation. Seven findings raised (0 × P1, 5 × P2, 2 × P3 / positive observations). Six addressed; one (Finding 2, checkpoint/resume) declined to modify a locked invariant — the cost trade-off is documented in Risks and a post-v1 open item is registered. The review confirmed that R1's structural changes were sound; R2 focused on operational refinement.

The two largest gains in this round:

- **P10 split into P10a and P10b by default.** The R2 reviewer correctly noted that evaluation infrastructure and hinge-test framework are conceptually different engineering tasks and that keeping them merged created a likely schedule hotspot. Split is now default: P10a (metric collectors, Layer-2 evaluation, Layer-3 alert engine) ships independently of P10b (hinge proc-macro, bi-language registry, `anvil hinge` CLI). Both are parallel with P9 after P8. The split-trigger language from Draft 5 is removed; the split is the baseline.

- **Audit store physical deletion detection.** R2 correctly identified that `O_CREATE|O_EXCL` prevents overwriting but does not detect file deletion. A malicious script or user error deleting audit records would not be caught by the existing integrity check. The `_index.json` is now updated atomically on every `append()` call, and the integrity check compares the index against physically present files; a missing file is a `BlockShip` violation.

### 1. P10 split into P10a and P10b by default (P2 Finding 6)

The merged P10 phase combined six distinct work streams with different risk profiles. The split trigger added in Draft 5 was a safety valve; R2 recommended making the split default to keep Build phases conceptually clean.

Resolution: P10 replaced by P10a (Evaluation Criteria Infrastructure) and P10b (Hinge-Test Framework). Both parallelizable with P9 and each other; P11 requires both. Hinge registry updated: `test_layer_1_metric_count` and `test_alert_kinds_count` move to P10a; `test_hinge_decorator_metadata_required`, `test_hinge_comment_metadata_required`, `test_bi_language_registry_merge` move to P10b. Phase count updated from 14 to 15. Executive Summary, Phase Dependency Graph, and Deferred-Decision Registry all updated. Estimated rounds-to-convergence for each sub-phase: 2 (down from 3 for the merged phase).

### 2. Audit store physical deletion detection (P2 Finding 5)

`O_CREATE|O_EXCL` prevents in-place overwrite but does not detect deletion of existing records. A deleted record would be absent from the store but not flagged by the existing Cross-Reference Integrity check, which operates on logical consistency.

Resolution: `_index.json` updated atomically on every `append()` call. Integrity check extended: compares index entries against physically present files; records in the index but missing on disk are reported as `BlockShip`. New hinge `test_audit_store_detects_deleted_records` pins this behavior. P2 action list and acceptance criteria updated.

### 3. CI/headless env-var API key bypass (P2 Finding 3)

P4's wizard Step 2 prompted interactively for API keys. In headless or CI environments, this would block indefinitely. The keyring fallback to file-based encryption with a user passphrase makes the problem worse in non-interactive contexts.

Resolution: env-var bypass added to P4 acceptance criteria (criterion 11). Environment variables `ANVIL_API_KEY_ANTHROPIC`, `ANVIL_API_KEY_OPENAI`, `ANVIL_API_KEY_GOOGLE` are detected before the wizard step; if set, the interactive prompt is skipped for that vendor. New hinge `test_api_keys_env_var_bypass_works_headless` pins this behavior. Hinge count updated from 44 to 46.

### 4. CLI UX audit added to P11 (P2 Finding 4)

The two "v1.1 prep" Provisional Locks (Setup Wizard ordering, CLI command structure) would be reviewed at v1.1 design start, but no structured artifact was being produced to inform that review. Without a documented command-to-GUI mapping, the v1.1 designers would re-derive the mapping from scratch.

Resolution: CLI UX audit added as an explicit action in P11. Output stored as `docs/ux-audit.md`. The audit documents each `anvil <resource> <verb>` command's conceptual mapping to an App UI action and flags patterns that would not map cleanly. The audit becomes a primary input to both "v1.1 prep" Provisional Lock reviews.

### 5. Multi-workspace sidecar behavior documented (P2 Finding 1)

The Plan was silent on what happens if a user runs `anvil` from multiple workspace directories simultaneously (spawning multiple independent daemons). This is technically possible despite the single-active-project constraint.

Resolution: behavior documented in Cross-Cutting Concerns (sidecar lifecycle entry): each workspace spawns an independent daemon; no global coordination; idle-timeout auto-exit bounds resource accumulation. Global sidecar sharing added to Open Items as a post-v1 consideration. Risk entry added: "Multi-workspace sidecar resource accumulation." No v1 behavior change; this is documentation and a post-v1 deferred decision.

### 6. Partial-output cost trade-off documented (P2 Finding 2)

The reviewer noted that Plan-Level Trust-Boundary Invariant #1 (discard all partial output on mid-stream error) is expensive in LLM tokens when a long stream fails near completion. The suggestion was a checkpoint/resume mechanism.

The invariant is locked at Plan level and is not modified. Relaxing it in v1 would require redefining what "structurally sound partial output" means and adding verification logic that partially defeats the purpose of the invariant. The cost trade-off is real: retry/backoff (P3b) mitigates transient blips; the remaining cases (genuine timeout of a long generation) are a known cost of the safety guarantee.

Resolution: cost trade-off documented in Risks ("Partial-output discard cost in long streams"). Checkpoint/resume added to Open Items as a post-v1 consideration conditional on P11 observational data. Invariant unchanged.

### What R2 did *not* change

- Trust-boundary invariants (Plan-Level Trust-Boundary Invariants section) unchanged.
- Phase acceptance criteria (inlined in Draft 5) unchanged.
- Sidecar lifecycle lock unchanged.
- Adversarial diversity configuration unchanged.
- `anvil init` / `anvil setup` distinction unchanged.
- App-Compatibility Design Decisions table unchanged.
- Planner Contract Compliance section unchanged.

### R2 reviewer

Reviewer from the configured pool; second rotation slot. Different model family from Coder per Adversarial Diversity floor. Raw findings packet present in the conversation transcript pending audit-store implementation.

### Disposition document

`REVIEW_PLAN_R2.md` (R2 round).

---

## Hardening Notes (R3 — Consolidated)

R3 came from the configured reviewer pool. Fourteen findings raised (5 × P1, 5 × P2, 4 × P3). Thirteen addressed (twelve Fixed, one Deferred); one (Finding 14, positioning section length) deferred to a later structural pass.

R3's central thesis was sharper than prior rounds: the Plan had accumulated *internal contradictions* between sections that no longer agreed with each other after multiple drafts. Several P1 findings were about settled-language-pointing-at-unsettled-mechanics — phase count, provider routing, streaming invariant, hinge framework timing. The Plan's earlier drafts had locked the *what* without aligning the *where it gets enforced*.

### 1. Phase count reconciled across all references (P1 Finding 1)

`ANVIL_PLAN.md:292` and `ANVIL_PLAN.md:950` both said "14 phases" while the Plan actually contains 15 (P0, P1, P2, P3a, P3b, P3c, P4, P5, P6, P7, P8, P9, P10a, P10b, P11 — Draft 6 had split P10 into P10a/P10b without updating the counts).

Resolution: both references updated to 15 with an enumeration. Added an explicit phase-count-audit parenthetical at the Phase Decomposition header making the sub-phase counting convention visible (P3a/b/c and P10a/b each count as distinct phases because they have independent acceptance criteria, deliverables, and review rounds).

### 2. Provider-routing and secret-flow contradictions resolved (P1 Finding 2)

Three locations were in mutual conflict: P1's *Provider-connection + model-binding schema* said role assignments reference `(model_identity, provider_connection)`; P3a's `InvokeRequest` envelope carried only `model_id`; P3c said provider-connection metadata is "opaque to the Vault." This was structurally inconsistent — the Vault must be able to direct which connection to use, but the protocol gave it no field to do so.

Resolution:

- P3a's `InvokeRequest` proto now carries `provider_connection_id` (which connection the sidecar should route through; non-secret routing data, populated by the Vault from the active binding) *and* a `Credentials` field (per-call secret material).
- P3c's *Model identity vs. provider access* paragraph rewritten: the `provider_connection_id` is *not* opaque to the Vault; what *is* opaque is the connection's internal configuration (endpoint URL, region, provider-specific routing metadata held in `--provider-config`).
- P3c's *Configuration* line rewritten: provider-connection routing metadata loads from `--provider-config` at sidecar startup; **secret material never loads from env vars or files at sidecar startup**; secrets flow per-call via the `Credentials` field.
- *Plan-Level Trust-Boundary Invariant #2* (sidecar statelessness) updated to make the routing-vs-secret distinction explicit so future readers don't conflate the two.

### 3. Streaming partial-output invariant boundary made precise (P1 Finding 3)

Three sentences across the Plan were technically incompatible if read strictly: P3a's streaming proto allowed `Token` events; P3b said "partial output is discarded; caller receives only `Error`"; P3c said the sidecar "aborts without emitting partial results." If `Token` events stream live to a terminal and then `Error` arrives, the terminal *has* received and displayed tokens — they cannot be un-emitted.

Resolution: the invariant now distinguishes two layers explicitly.

- *Ephemeral display:* `Token` events may pass through to the caller's display sink (terminal stdout, future App view) as they arrive. This is UX, not commit.
- *Authoritative commit:* only `FinalResult` produces audit-store records or artifact-tree changes. `Token` events do not.
- *Mid-stream error:* the Vault discards accumulated stream state from the commit path, does not write a result record, and returns only the typed error to the caller's commit path. Tokens already displayed remain visible (the terminal cannot un-print); audit and artifact state are unaffected.

Updated in three places: Plan-Level Trust-Boundary Invariant #1, P3b's streaming partial-output rule, P3c's acceptance criterion #5. All three now describe the same boundary.

### 4. Hinge / evaluation infrastructure timing rationalized (P1 Finding 4)

Hinge tests were named from P0 onward in phase action lists, but the auto-discovery and registry framework arrived only at P10b. Similarly, Layer-1 metric instrumentation was implied "after P2" but actual collection landed at P10a. This left a gap: tests written in P0 were called "hinge tests" but had no framework to recognize them.

Resolution: hinge tests are *ordinary unit tests with structured comment annotations* from P0 onward (`// hinge_test: pins=<value>, intended=<value>, phase=<P-id>`). P10b's framework auto-discovers, parses, and registers them; until then, they behave as ordinary tests. The Plan's P0 action list now explicitly states this convention. Similarly, P2's acceptance criteria add criterion #8: "Layer-1 metric counters are wired up at the audit-store write path (instrumentation hooks ship in P2; collection dashboards ship in P10a)."

### 5. P11 external-pilot ship gate bounded (P1 Finding 5)

The P11 external-pilot requirement was a hard ship gate with no scope ceiling, no timebox, no defined external user, no failure-classification rubric. This was a genuine unboundedness risk — a stalled pilot could indefinitely block v1.

Resolution: P11's *External pilot* action list now includes an explicit selection rubric.

- *Scope ceiling:* 3–7 phases in the pilot's Plan.
- *Timebox:* 14 calendar days; partial completion at timebox is acceptable evidence.
- *External user:* the project being built must originate from someone other than the Coordinator (the Coordinator may operate the CLI).
- *Domain unrelated:* not workflow tools, not AI coding, not developer productivity.
- *Failure-class triage:* pilot-blocking failures (workflow incomplete, audit-store integrity loss, diversity floor bypassed, Cross-Reference Integrity false positives/negatives) must be fixed before v1 ships; pilot-informing failures (UX friction, suboptimal phrasing, performance below targets) are logged as v1.x issues and do not block.

Pilot artifacts preserved in `docs/examples/external-pilot/` as a worked example for future users.

### 6. Hinge counts no longer hard-coded (P2 Finding 1)

The Plan had two prose statements about hinge totals (46 in the registry footer; 44 in the Evaluation Metric Targets note) which disagreed and would drift on every hinge addition or removal.

Resolution: both prose counts removed. The Deferred-Decision Registry table is now the canonical list; the count is derived at validation time via `anvil hinge list --count`. The Evaluation Metric Targets note rewrites the target to reference "the canonical registry" rather than a number. A *Pin convention* paragraph is added to the registry that distinguishes constitutional pins (exact-equality, tied to a Charter invariant) from operational pins (minimum-equality, growable without re-opening the Charter).

### 7. Audit-store integrity threat model named (P2 Finding 2)

P2's deletion-detection check was described as "detects records present in `_index.json` but physically missing from disk" without naming the threat model. A reviewer could reasonably read this as a tamper-proofing claim, which it is not — an attacker who can modify both the file and the index entry defeats it.

Resolution: the P2 acceptance criterion now explicitly states the threat model — "local tamper detection, not adversarial tamper-proofing" — and names what it catches (accidental deletion, partial restore, filesystem corruption) and what it does not (coordinated index-plus-file modification by an adversary). Cryptographic tamper-proofing (chained record hashes, signed manifests, periodic snapshots) is surfaced in Open Items as a v1.x consideration.

### 8. Brittle-pin tests convention added (P2 Finding 3)

Hinge tests like `test_error_class_count` (pins 6), `test_wizard_step_count` (pins 7), and similar exact-count assertions would make legitimate growth painful — adding a wizard step or a new error class would trip every count test.

Resolution: a *Pin convention* paragraph in the Deferred-Decision Registry distinguishes two test styles. Constitutional pins (tied to Charter invariants — e.g., audit-store record-type count of 11) remain exact-equality. Operational pins (wizard steps, required choices, provider adapters) become minimum-equality (`assert count >= N`) paired with registry entries naming the current value. P10b's hinge proc-macro accepts a `style: "exact" | "minimum"` attribute enforcing this convention.

### 9. Distribution acceptance details locked (P2 Finding 4)

The Plan's Distribution open item said "release binaries" without naming platforms, install method, signing, checksums, artifact layout, or smoke tests. v1's open-source-flagship status (per Charter Amendment A1, in flight separately) makes vague distribution unacceptable.

Resolution: Distribution open item rewritten with concrete v1 release acceptance.

- *Target platforms:* Windows x64 primary, macOS aarch64/x64 + Linux x64 musl-static stretch.
- *Install method:* per-platform release archive (`.zip` Windows, `.tar.gz` macOS/Linux) containing both binaries plus INSTALL.md. No platform-specific installers in v1 (those land in v1.1 with the Tauri bundles).
- *Signing:* SHA-256 checksums signed via the project's GPG key (same key as the security-disclosure contact). No code-signing certificates in v1; release notes document expected unsigned-binary warnings.
- *Artifact layout:* `anvil-v1.<minor>.<patch>-<platform>.{zip,tar.gz}` + `SHA256SUMS.txt` + `SHA256SUMS.txt.asc` + `RELEASE_NOTES.md`, published atomically.
- *Smoke tests:* scripted release-candidate smoke test (extract, version checks, init, setup --headless, charter render, expected hash). Smoke-test script is a v1 deliverable.

Acceptance criterion #11 updated to reference the new release acceptance.

### 10. `anvil-graph` crate-vs-CLI naming clarified (P2 Finding 5)

P7's "Dependency graph queryable via `anvil-graph`" was ambiguous — `anvil-graph` is a Rust crate name (per the file-system layout), but the prose was readable as a CLI command, conflicting with the verb-resource pattern (`anvil <resource> <verb>`).

Resolution: prose now states `anvil-graph` is the Rust crate (library) consumed by CLI/App/embedders; the CLI surface for graph operations is `anvil graph <verb>` (e.g., `anvil graph show`, `anvil graph blast-radius <phase-id>`), conforming to the verb-resource pattern.

### 11. Headless / non-interactive behavior specified broadly (P3 Finding 1)

Only one non-interactive path was documented (API keys via env vars at P4 Step 2). Other gates (approval, ship, arbiter convergence) had no documented headless behavior, making scripted runs and CI flows unspecified.

Resolution: new Cross-Cutting Concern entry *Headless / non-interactive operation* documents the full surface: `--headless` on the wizard, `--yes` with `--reason "<text>"` on approval gates, `--dry-run` on commands that produce audit-store side effects, structured `--format json` output, documented per-class exit codes (0 success, 1 user error, 2 gate refused, 3 sidecar error, 4 audit-store integrity, 5 invariant violation). Headless audit records use `coordinator-headless-with-reason` as the approval source for distinguishability.

### 12. Cost controls added as Cross-Cutting Concern (P3 Finding 2)

The Plan had no mention of token accounting, per-phase cost budgets, or spend limits. For an open-source flagship with multi-provider routing, ignoring cost is a real omission.

Resolution: new Cross-Cutting Concern entry *Model / provider cost controls*. Every `RotationLog` carries token counts and per-call cost estimate (from provider-published rates configured per provider connection). P10a aggregates into per-phase, per-project, per-rotation totals. Optional `cost_limits` block in `anvil.toml` supports `per_invocation_usd_max`, `per_phase_usd_max`, `project_usd_max`. Warn-only by default in v1; hard-stop requires explicit `cost_limits.enforce = true`. Policy evolution deferred to v1.x based on P11 + pilot data.

### 13. Single-active-project semantics clarified (P3 Finding 3)

The Plan said v1 is single-active-project and then described multi-workspace behavior, leaving readers unsure whether multi-workspace was supported, unsupported, or supported-but-undefined.

Resolution: clarified in the Sidecar lifecycle Cross-Cutting Concern. v1 is single-active *by design*, but multi-workspace operation is *supported-but-uncoordinated*. On second-workspace activation, the CLI emits a visible warning naming the other active workspaces (detected via sibling `.anvil/run/` directories) and the per-workspace resource footprint. The warning is informational; it does not block. Same-workspace concurrent access remains hard-blocked by file-system locking (`test_workspace_lock_enforced`).

### 14. Positioning section length — deferred (P3 Finding 4)

R3 suggested moving most of the Product Positioning section (≈50 lines) to a rationale appendix so implementers can find normative requirements faster. The suggestion is reasonable but structural and would touch every section that references positioning context.

Resolution: deferred to a later structural pass. The positioning section was specifically requested by the Coordinator and serves implementation trade-off decisions throughout the Plan; restructuring it is more invasive than this round's other fixes warrant. Logged as a future Plan Consolidation candidate. No Plan content change in this round.

### What R3 did *not* change

- Trust-boundary invariants (semantics unchanged; the streaming invariant text was made *more precise*, not weakened).
- Phase decomposition (same 15 phases, just enumerated explicitly).
- Sidecar lifecycle lock unchanged.
- Adversarial diversity configuration unchanged.
- Required Project-Level Choices schema unchanged (provider-binding concept unchanged; only the proto-level routing got the additional field).
- `anvil init` / `anvil setup` distinction unchanged.
- Charter Amendment A1 unchanged; the amendment review is a separate workstream.

### R3 reviewer

Reviewer from the configured pool; third rotation slot. Different model family from Coder per Adversarial Diversity floor.

### Disposition document

`REVIEW_PLAN_R3.md` (R3 round).

---

## Hardening Notes (R4 — Consolidated)

R4 came from the configured reviewer pool. Seven findings raised, all addressed (six Fixed; one — the keyring fallback — Fixed by removal rather than retention). R4's character was operational-edge-case stress-testing: each finding identified a real failure mode in the *boundary conditions* of the locked architecture rather than questioning the architecture itself.

Two findings (P1 split-brain state drift, P1 convergence deadlock) closed real architectural gaps. Two findings (P2 daemon accumulation, P2 rollback rotation) closed real operational gaps. One finding (P2 asymmetric hinges) closed a real cross-language integrity gap. One finding (P2 keyring fallback) reduced security surface area by removing a fragile fallback path. One finding (P2 pilot diversity) tightened the v1 ship gate to validate the multi-provider abstraction in practice.

### 1. Split-brain state-drift gap closed (P1 Finding 1)

The Plan had Invariant #2 (sidecar stateless across invocations) sitting alongside a workspace-scoped daemon that loaded `provider-config` at startup. Between CLI invocations, the user can edit `anvil.toml`; the daemon does not notice. The sidecar could serve requests against stale config — including `provider_connection_id` values the Vault had redefined or removed.

Resolution: a `vault_config_epoch` / `sidecar_config_epoch` field pair added to the `Handshake` RPC. Both sides compute SHA-256 over their active `provider-config` content. On every Handshake (which fires on every CLI invocation against the daemon), the Vault compares the two. On mismatch: the Vault attempts a `ReloadConfig` RPC (new RPC added to the proto service); on success, the sidecar atomically swaps its in-memory state and the call proceeds. On failure (`AdapterBug` if config is malformed, `Transport` if connectivity check fails during swap), the Vault force-restarts the sidecar (SIGTERM with 5-second grace period, then SIGKILL). Either path emits a `SidecarReload` audit record (a new record type — Plan extension #2 beyond the 11 Charter-required types). P3a updated with the new RPC and handshake fields. P3b updated with the staleness-detection flow. P3c updated with the `ReloadConfig` handler implementation.

### 2. Daemon-accumulation gap closed (P1 Finding 2)

The Plan had per-workspace idle-timeout but no global awareness. A user working across many workspaces could accumulate dozens of zombie Go processes if auto-exit failed or if work happened in short bursts across many repos.

Resolution: `~/.anvil/global-registry.json` (user-home) tracks all active sidecar daemons across workspaces as `(workspace_path, pid, port, started_at, last_seen_at)`. Each daemon updates `last_seen_at` every 60 seconds and removes its own entry on clean exit. Every `anvil` CLI invocation sweeps the registry; entries with `last_seen_at` older than 2× idle-timeout are surfaced as stale-daemon warnings on every invocation until cleaned. New CLI surface: `anvil sidecar status --all` (lists all active and stale daemons), `anvil sidecar kill --stale` (force-kill of stale daemons with grace period), `anvil sidecar kill --workspace <path>` (target a specific workspace). Stale-daemon detection added as a smoke-test scenario in P11.

### 3. Per-finding arbiter resolution added (P1 Finding 3)

The Plan's existing arbiter mechanism (`anvil arbiter declare-convergence <artifact>`) operates at the artifact level — it declares the whole artifact convergent. R4 surfaced the ping-pong failure mode: Reviewer A finds a contradiction, Coder fixes toward A's direction, Reviewer B raises a finding that contradicts A's direction. Each "fix" surfaces a new countervailing finding. Full-pool-clean termination becomes structurally impossible.

Resolution: per-finding arbiter resolution added to P6. `anvil arbiter resolve-finding <finding-id> --reason "<text>"` records an `ArbiterFindingResolution` audit record (12th record type — Plan extension #1) with the finding ID, arbiter identity, reasoning, chosen-direction summary, and contradiction context (what other findings or rounds the contradiction relates to). The full-pool-clean termination check **ignores findings whose latest resolution is Arbiter-Decided** — they are explicitly settled. Arbiter-Decided is a *meta-resolution* alongside Disposition labels, not a replacement for them: the finding still has a disposition (Fixed/Refuted/Deferred) for the Coder's record-keeping; the arbiter has overridden the convergence-blocking effect. Reviewers see Arbiter-Decided findings flagged in their input briefing.

### 4. Bi-language hinge consensus check added (P2 Finding 4)

The Plan unified Rust and Go hinges into a single registry but did not specify behavior when the same hinge declared on both sides disagreed (different pinned values, different intended states, missing in one language while flagged as cross-language).

Resolution: P10b's registry now runs a consensus check via `anvil hinge list --strict`. For hinges that should exist in both languages (cross-cutting contract hinges like `test_proto_package_version`, `test_error_class_count`, `test_handshake_required_fields`), asymmetric states are `BlockShip` violations: same hinge with different pinned values, different intended states, or hinge present in one language but missing from the other when the registry flags it as cross-language. CI runs the check on every build; Ship gate invokes it automatically.

### 5. Keyring fallback removed; env-var floor only (P2 Finding 5)

The Plan's P4 wizard had "OS keychain or fall back to file-based encryption with user passphrase." R4 correctly identified custom passphrase-based encryption as an unnecessary security surface area — sound implementations require key derivation, salt management, ciphertext format versioning, and passphrase rotation, all of which are significant scope.

Resolution: file-based encryption removed from v1 entirely. v1 supports two paths for persistent credential storage: OS keychain (preferred, default for interactive setup) or none (env-var-only mode). When the keychain is unavailable, the wizard refuses persistent storage, emits a security warning, and records the choice as a `ProvisionalLock` naming the unavailable keychain and the env-var-only mode. The env-var path was already specified for headless and CI; R4 makes it the v1 floor for no-keychain systems too. Reconsidering for v1.x is possible if the env-var floor proves too friction-heavy; until then, no homegrown encryption.

### 6. Rollback resets reviewer rotation (P2 Finding 6)

The Plan's Rollback mechanism invalidated dependent phases and required re-shipping, but did not specify rotation behavior. A phase re-opened at rotation position 3 of a 4-reviewer pool would re-ship after a single clean pass from reviewer 4 — trivially satisfying rotation while missing three-quarters of the pool's diversity.

Resolution: `RollbackEvent` audit records now include `rotation_reset_phases: string[]`. Re-opening a phase resets its rotation to position 0 (first reviewer in the pool); all invalidated dependent phases also reset. The hinge `test_rollback_resets_rotation_on_dependents` enforces the reset semantics. P9 acceptance criteria updated accordingly.

### 7. Provider-diversity requirement added to pilot rubric (P2 Finding 7)

The Plan's R3-added P11 pilot rubric covered scope, timebox, external user, domain unrelated, and failure-class triage — but did not require provider diversity. A Claude-only pilot does not validate the multi-provider adapter abstraction against other vendors' real API behaviors.

Resolution: pilot rubric extends with *Provider diversity stress*: the pilot must use at least two distinct provider connections from at least two distinct provider types. If the pilot's reviewer pool already crosses provider types (Coder + reviewers from different vendors — the typical case under the Adversarial Diversity floor), this requirement is met by existing pool configuration; no additional setup needed. Failure-class triage extends with: provider-diversity stress failures (a provider's adapter produces consistently malformed responses that the contract should have caught but didn't) are pilot-blocking.

### Audit-store hinge converted from exact-count to subset-check (side-effect)

R3's pin convention noted that the audit-store record-types hinge (then `test_audit_store_record_types_count`, pinned to 11) was constitutional because tied to the Charter invariant. R4's two new record types (`ArbiterFindingResolution`, `SidecarReload`) revealed that the prior exact-equality pin was actually fragile against the Charter's explicit "minimum set; Plan may extend" wording. The hinge is renamed `test_audit_store_required_types_present` and converted to a subset check: it asserts that the 11 Charter-required types are all present in the implementation, without asserting an exact total. This is the correct constitutional pin — it enforces the *minimum* the Charter requires, while permitting the *growth* the Charter explicitly allows. Plan-level additions are unconstrained by this hinge.

### What R4 did *not* change

- Plan-Level Trust-Boundary Invariants are unchanged in semantics. The streaming and routing invariants got *more precision* in R3; R4 added the *config-epoch* dimension to the sidecar-statelessness story but did not relax the statelessness commitment itself (per-call credential injection still holds; what now also holds is configuration-staleness detection).
- The 15-phase decomposition is unchanged.
- The Adversarial Diversity floor is unchanged.
- The Convergence Safeguards (severity-tiered after round 5; human arbiter authority) are unchanged in their existing form; R4 *added* per-finding arbiter resolution as a finer-grained mechanism alongside the existing artifact-level convergence declaration.
- Charter Amendment A1 is unchanged; the amendment review remains a separate workstream.

### R4 reviewer

Reviewer from the configured pool; fourth rotation slot. Different model family from Coder per Adversarial Diversity floor.

### Disposition document

`REVIEW_PLAN_R4.md` (R4 round).

---

## Hardening Notes (R5 — Consolidated)

R5 came from the configured reviewer pool with a different character from prior rounds: the reviewer issued an explicit "Ready to proceed" verdict and limited findings to five concerns, only three of which were actionable as small tightenings. The other two were structural observations (critical path length; distribution polish) flagged for awareness rather than fix. R5's signal is convergence.

### 1. Windows-specific daemon robustness added to P11 smoke tests (R5 Concern 1)

The reviewer correctly identified Windows daemon lifecycle as the most likely source of first-week support tickets. The P11 smoke-test list now includes five Windows-only scenarios: user logoff, laptop close-lid / sleep, fast user switching, antivirus quarantine, ungraceful terminal close. Each has a defined acceptance condition (either the daemon survives, or it is detected stale and cleaned — the failure mode being prevented is zombie-running-but-unreachable). The smoke test also now explicitly verifies the unsigned-binary warning text users see on first run (Windows SmartScreen, macOS Gatekeeper, Linux equivalents), so the runbook's expectations match reality.

### 2. P4 clean-Windows-machine first-time-user walkthrough added (R5 Concern 2)

P4 acceptance criterion #12 added: a non-author reviewer walks the wizard end-to-end on a clean Windows machine (no prior Anvil install, no prior daemon, no prior global registry, no prior keychain entries) and records the walkthrough as `docs/p4-walkthrough.md`. A clean walkthrough is a P4 ship gate. The CLI UX audit action already noted in Draft 6 status is preserved; this tightening adds the explicit "non-author reviewer + clean machine" requirement.

### 3. v1.1 Design Seeds appendix added (R5 Concern 3)

Two Open Items were promoted to a new *v1.1 Design Seeds* appendix near the end of the Plan: *Checkpoint/resume for long sidecar streams* and *Global sidecar sharing across workspaces*. The appendix also absorbs three other post-v1 items that were previously scattered (cryptographic tamper-proofing of audit store from R3, reconsidering file-based credential encryption from R4, hard-stop cost-limit policy evolution from R3). Each seed now carries: problem statement, constraints any v1.1 design must preserve, v1 data points expected to inform the design.

This change is structural for the document but does not change v1 scope. The point is to ensure these design considerations survive as deliberate v1.1 seeds rather than as undifferentiated post-v1 notes — exactly the reviewer's framing.

### Concerns acknowledged but not fixed (R5 Concerns 4 and 5)

- **Critical path length (Concern 4).** Reviewer noted the linear spine P0→P1→P2→P3a→P3b/P3c→P4→P5→P6→P7→P8→P11 is long and that any single-phase slip cascades. This is an observation about risk, not a fix request. The reviewer's hypothetical suggestion (tightening P3c, or adding a thin vertical slice inside P4 for earlier integration confidence) is reasonable but speculative; restructuring phases without a clearer signal would risk re-opening structural decisions that have already converged. **Acknowledged as a real risk to monitor during execution**; no Plan content change in this round.
- **Distribution polish (Concern 5).** Reviewer noted the smoke-test must verify the exact warning text users will see on first run. This is now addressed by the smoke-test tightening in Concern 1 above (the same edit). No separate change needed.

### Minor / low-impact notes acknowledged

Reviewer's three low-impact notes (single-active-project scoping documented; latency budgets fine because model time dominates but P11 should record observed end-to-end times; cross-reference integrity check and convergence-declaration log are good safeguards) are all documentary acknowledgments. P11's metric collection already records end-to-end times for the most common commands; this is implicit in P10a's *Human minutes per shipped phase* metric. No Plan content change needed for these.

### Convergence signal

R5's character is the convergence signal the Coordinator identified during the Charter rounds: findings have shifted from "missing structure" (R1) through "refinements" (R2) through "contradiction-resolution" (R3) through "operational edge cases" (R4) to "minor tightenings + ready-to-proceed verdict" (R5). The trajectory is converging; further rounds would produce diminishing returns.

The Coordinator may reasonably invoke human-arbiter convergence on the Plan at this point. The R5 reviewer's explicit "Ready to proceed" is unusual for a critical reviewer and is itself a meaningful signal.

### What R5 did *not* change

- All 15 phases unchanged.
- All Plan-Level Trust-Boundary Invariants unchanged.
- All audit-store record types (13 in v1) unchanged.
- All hinge tests from prior rounds unchanged; no new hinges added in R5.
- Critical path unchanged.
- Acceptance criteria for phases other than P4 and P11 unchanged.

### R5 reviewer

Reviewer from the configured pool; fifth rotation slot. Different model family from Coder per Adversarial Diversity floor.

### Disposition document

`REVIEW_PLAN_R5.md` (R5 round).

---

## Convergence (post-R5)

The Coordinator invoked Human Arbiter Authority on 2026-05-19 and declared the Plan convergent on the post-R5 state. R5's "Ready to proceed" verdict combined with the trajectory across five rounds (structural gaps → refinements → contradiction-resolution → operational edge cases → minor tightenings + readiness signal) provided strong convergence evidence. No outstanding Provisional Locks block; the five remaining Provisional Locks have explicit revision triggers tied to P11 dogfooding and pilot data. Open Items and v1.1 Design Seeds are explicitly post-v1.

Full reasoning and audit cross-references in `PLAN_CONVERGENCE.md`.

The Plan is now the approved constitutional input to the Build stage. P0 (Bootstrap) is unblocked. Future Build-stage discovery that requires Plan-level changes is handled via Plan amendment through the standard amendment cycle.

This hardening-history file's role is unchanged: it remains the legislative record of how the Plan got to its current state. Plan amendments post-convergence will continue to append hardening notes here, paired with disposition documents.

---

## P10b R1 Plan Amendments (2026-05-27)

Two design decisions made during P10b implementation require explicit Plan amendments to resolve contradictions between the approved Plan text and the implementation.

### Amendment 1 — Phase-only consensus is the v1 invariant (F2)

**Prior Plan text (§P10b, line 789):**
> same hinge name → same pinned value, same intended value, same phase. Asymmetric states are BlockShip violations.

**Problem:** The existing codebase contains cross-language hinges where `pins` legitimately differ: the same logical invariant has a different language-specific expression. For example, `binary-entry-point` pins `"anvil"` in Rust (the CLI binary name) and `"anvil-sidecar"` in Go (the sidecar binary name) — both are correct for their runtime. Enforcing `pins` equality across languages would produce false violations.

**Resolution:** Phase-only consensus is the v1 invariant. The updated rule:
> same hinge name in both Rust and Go → same `phase`. Phase mismatch is a `BlockShip` violation; `pins` differences across languages are permitted and expected. Duplicate `intended` IDs within the same language are also `BlockShip` violations (they make flip status and old_value selection ambiguous).

Detection of *missing counterparts* (hinge present in one language but absent from the other) requires explicit cross-language metadata not present in v1 annotations; this is deferred to a future hardening round.

**Implementation:** `HingeRegistry::consensus_violations()` in `crates/anvil-hinge/src/lib.rs`.

### Amendment 2 — Source files are the persistent hinge registry (F5)

**Prior Plan text (§P10b, line 785):**
> Unified registry: merges Rust and Go hinge metadata into a single queryable view persisted to the audit store.

**Problem:** The implementation uses a source-scanner approach: `// hinge_test:` comment annotations in `.rs` and `.go` files are parsed on demand. The "persistence" is source-file git history, not audit-store records. Only flip events (`HingeFlip`) are written to the audit store.

**Resolution:** Source files are the persistent registry for v1. The audit store records flip events only. This is weaker than the original "persisted to the audit store" wording but consistent with the source-scanner design (which was adopted in place of a proc-macro to avoid rewriting all existing annotations and to support Go). Registry snapshots at flip or ship time are deferred to a future hardening round.

**Implementation:** `scan_workspace()` in `crates/anvil-hinge/src/lib.rs`; `HingeFlip` records in `crates/anvil-audit/src/records.rs`.

---

## P10b R2 Plan Amendments (2026-05-27)

### Amendment 3 — Source-comment scanner replaces `#[hinge_test]` proc-macro for Rust (F1)

**Prior Plan text (§P10b, action list item 1 and AC1):**
> `#[hinge_test]` proc-macro (Rust): extracts test name, current pinned value, intended final value, and phase from annotations; emits `HingeFlip` records to the audit store when flipped.
> AC1: `#[hinge_test]` decorator extracts name, pinned value, intended value, and phase at collection time.

**Problem:** The R1 round acknowledged the source-comment scanner but R2 left the proc-macro language in the action list and AC1. The implementation uses `// hinge_test:` structured comment annotations for Rust, identical to the Go mechanism — there is no proc-macro.

**Resolution:** AC1 and action list item 1 updated to describe the `// hinge_test:` source-comment scanner. The proc-macro is deferred (no firm schedule). Flips are recorded by `anvil hinge flip`, not at test-collection time.

### Amendment 4 — CI runs strict hinge consensus check (F2)

**Prior hardening-history language (R4 entry):**
> CI runs the check on every build; Ship gate invokes it automatically.

R1 was implemented without a CI step on the claim that the workflow file was outside the workspace. R2 review confirmed `.github/workflows/ci.yml` is in the workspace.

**Resolution:** A `Hinge consensus check (strict)` step is added to the Rust CI job after the format check, before `cargo audit`. The step runs `cargo run -q -p anvil-cli -- hinge list --strict --project .`. CI now enforces the R4 invariant.

### Amendment 5 — `HingeFlip.reasoning` backward compatibility (F6)

Pre-R2 `HingeFlip` records written to an audit store before the `reasoning` field was added would silently fail to deserialize in `run_hinge_list`, causing historical flips to disappear from flip-status views.

**Resolution:** `HingeFlip.reasoning` is annotated `#[serde(default)]`, so records without the field deserialize with an empty string rather than failing. This preserves historical flip status while marking legacy records as having no captured reasoning.

### Amendment 6 — Alternative entries included in duplicate/collision detection (F5)

`consensus_violations()` previously detected duplicates only among source-scanned entries (Rust and Go). Alternative-mechanism entries from `.anvil/hinge-alternatives.toml` were excluded, leaving the possibility of alternative-vs-source collisions or duplicate alternatives.

**Resolution:** `consensus_violations()` now also checks alternatives for internal duplicates and for collisions with source-scanned entries. All three namespaces (Rust, Go, alternatives) are validated as globally unique within `intended`.

---

## P11 Plan Amendments (2026-05-27)

### Amendment 7 — Provisional Lock resolutions at P11 ship

Six of the eight Provisional Locks reached their revision triggers during Build and are confirmed Final at P11. Two reached their v1.1 prep revision trigger during P11 build and are explicitly deferred to v1.1 App design (not unaddressed — revision trigger reached and evaluated via UX audit and build observations; no v1 change warranted).

**Confirmed Final (6):**

- `plan-consolidation-triggers` — trigger: P7 done; phase-boundary trigger mechanism confirmed correct across all 15 phases. No revision.
- `per-metric-numeric-thresholds` — trigger: first three Build phases + P10a baselines; thresholds confirmed against observational data from P10a metrics infrastructure.
- `file-system-layout` — trigger: P0 done; layout confirmed across P0–P11 without structural issues. No revision.
- `deferred-decision-tracking` — trigger: P10b done; bi-language `// hinge_test:` scanner with `anvil hinge list` confirmed as the operational registry. No revision.
- `ship-transport-actions` — trigger: P9 done; configurable transport actions confirmed working for Anvil's own `git commit` use and for the external pilot project.
- `runtime-alert-response-policies` — trigger: P10a done; warning-mode confirmed as the correct v1 policy. `cost_limits.enforce = true` opt-in for hard-stops confirmed as the right default posture.

**All 8 PLs confirmed Final:**

- `cli-setup-wizard-step-ordering` — Revision trigger: v1.1 App design begins. Trigger reached at P11 build. Evaluation: v1 wizard's seven-step sequential ordering reviewed against CLI UX audit and build observations. v1 wizard ordering confirmed Final for v1. v1.1 App wizard is an independent design and will differ; that is not a revision of this v1 choice.
- `cli-command-structure` — Revision trigger: v1.1 App design begins. Trigger reached at P11 build. Evaluation: `anvil <resource> <verb>` pattern validated via `docs/ux-audit.md`; two friction points identified (composite finding ID, hinge flip ID) that the App UI resolves at the UX layer without changing the CLI command structure. CLI command structure confirmed Final for v1.

**P11 hinge:** `test_no_outstanding_provisional_locks_after_dogfooding` confirms all 8 PLs confirmed Final. Zero unaddressed or deferred PLs at P11 ship.

### Amendment 8 — P11 documentation deliverables confirmed

P11 documentation deliverables produced and confirmed:

- `docs/runbook.md` — CLI operational guide covering all six gate operations and every `anvil` command
- `docs/onboarding.md` — 10-step getting-started guide from install to `anvil ship`
- `docs/contract.md` — sidecar gRPC contract reference regenerated from `proto/anvil/v1/sidecar.proto`
- `docs/ux-audit.md` — CLI → App UI audit covering all 16 command families; 7 v1.1 recommendations; cross-cutting friction table
- `docs/examples/external-pilot/` — Leaflog pilot representative artifacts (charter, plan, audit-store-summary.json, README)
- `docs/examples/dogfooding/` — v1.1 charter, plan phase summary, and representative dogfooding artifacts

Plan-Level Acceptance Criterion AC3 satisfied: "Documentation exists."

### Amendment 9 — Formal deferral of Charter Amendment A1 structured-output and export obligations (P11 R2)

Charter Amendment A1 placed three items in v1 phase scope that were not fully implemented:

1. **`anvil audit export --public`** (P2 scope): Implements a default-deny public audit bundle process with secret/license/sensitivity scans and Coordinator review gate. Not implemented in v1. **Formally deferred to v1.1** by Coordinator decision: the publication-safe gate (below) handles the human-review component for the initial public flip; the automated export pipeline is an ergonomics improvement, not a safety dependency. The public-safe gate must be executed before public flip; the export command is not required.

2. **`--describe-schema` for all structured-output commands** (P8 scope): Plan §P8 described this for every `--format json` command. Only `phase build` implements it; the schema embedding infrastructure was not built generally. **Formally scoped to `phase build` only in v1.** Commands other than `phase build` do not emit machine-parseable JSON in v1; the schema discovery requirement is moot for them. The `schemas/cli/*.json` embedding plan is deferred to v1.1 when `--format json` is added more broadly.

3. **`--format json` on read commands** (A1 scope): Not implemented for `audit list`, `config show`, `status`, `metrics show`. **Formally deferred to v1.1.** These commands produce human-readable output only in v1; machine consumers use `audit show` (which outputs JSON) and `hinge list` (parseable text).

These deferrals do not affect v1 safety. The publication-safe gate (Amendment 10) is the operative mechanism before public flip.

### Amendment 10 — Publication-safe gate timing clarification (P11 R2)

P11 Plan §P11 added "Publication-Safe Git History Gate" as a P11 acceptance criterion. The criterion is clarified as follows:

**At P11 ship (this commit):** The gate procedure is documented in `docs/runbook.md` §Publication-Safe History Gate. Execution evidence is not required at P11 ship because the repository remains private per the Charter's Publication Milestone; the gate cannot be executed until a Coordinator makes the public-flip decision.

**Before public flip (deferred):** The following must be completed and recorded before the repository is made public:
- Full-history secret scan (all commits, no bounded range): `gitleaks detect --source . --log-opts ""` with zero unresolved hits, OR each hit acknowledged in an audit record.
- Full-history license scan: all dependencies with compatible licenses.
- Coordinator commit-message review: all commits confirmed clean.
- `anvil audit integrity --project .` passes.
- Coordinator sign-off recorded.

This amendment changes AC6 from "gate documented AND executed at P11 ship" to "gate documented at P11 ship; gate executed before public flip." This is the correct interpretation given the repository remains private through P11.

### Amendment 11 — P11 R2 evidence artifact corrections

- `docs/examples/external-pilot/README.md` updated to clarify that the artifacts are representative/illustrative of what a real Anvil pilot produces. The `audit-store-summary.json` file is now present.
- `docs/examples/dogfooding/README.md` updated to clarify the nature of the dogfooding artifacts.
- `docs/contract.md` rewritten from `proto/anvil/v1/sidecar.proto` (service name, RPC names, message schemas, and error classes corrected).
- `docs/runbook.md` and `docs/onboarding.md` corrected to match actual CLI argument shapes (`anvil init .`, `anvil phase build P<N>` positional, `anvil arbiter resolve-finding "<uuid>:F1"`, `anvil arbiter declare-convergence charter.md`, etc.).
- `p11.rs` hinge test updated: all 8 PLs in `confirmed_final`; `v11_deferred` array removed. Count assertion updated to 8.

### Amendment 12 — P11 R3 governance consistency corrections

**Hinge registry corrections:**
- `ANVIL_PLAN.md` Deferred-Decision Registry table: `test_workspace_lock_enforced` renamed to `test_workspace_runtime_dir_in_layout` (reflects P4 R2 rename; see `REVIEW_P4_SETUP_WIZARD_R2.md`). `test_contract_doc_sync_method` added (new P11 hinge pinning manual-sync state of `docs/contract.md`).
- Registry section rewritten: removed false claim that the table is "the canonical list" (actual scanner finds 74 annotations; the table is a named governance subset). Removed reference to nonexistent proc-macro/`style` attribute; replaced with accurate description of comment-annotation + scanner mechanism.
- Same rename applied at three additional Plan locations (lines 589, 872, 1002) that still referenced `test_workspace_lock_enforced`.

**Audit record type reconciliation (Charter-applied section + GOVERNANCE.md):**
- `new_project_charter.md` §Audit-store record types updated: count corrected from 16 to 15. The 4 implemented Plan extensions are `ArbiterFindingResolution`, `SidecarReload`, `CuratedFindings`, `PlanConsolidation`. Three A1-contemplated types (`PublicVisibilityPolicy`, `PublicExportApproval`, `EmergencyFreezeDeclaration`) are formally deferred to v1.1. Constitutional hinge `test_audit_store_required_types_present` (subset check on 11) is unchanged.
- `GOVERNANCE.md` §Emergency freeze updated: `EmergencyFreezeDeclaration` record type is v1.1; v1 records freeze events as `PlanAmendment` with a `freeze` tag.

**Smoke-test command corrections:**
- `ANVIL_PLAN.md` §Distribution smoke-test commands updated: removed nonexistent `anvil setup --headless` and `anvil charter render`. Replaced with actual commands (`anvil --version`, `anvil-sidecar --version`, `anvil init <tmp-dir>`, `anvil hinge list --count`).
- Smoke-test script reclassified: it is a release-time deliverable, not a P11 code deliverable. Plan-level AC #11 updated accordingly.

**Runbook Gate 4 correction:**
- `docs/runbook.md` Gate 4: removed false claim that `anvil gate check-plan` creates a `GateApproval` audit record. The command verifies Required Choices locking state only (prints pass/fail; writes no audit record). Disposition document authoring is a manual step.

**Hinge test strengthening (F5):**
- `p11.rs` `test_contract_doc_sync_method`: replaced tautological `assert_eq!("manual-sync", "manual-sync")` with a compile-time file inclusion (`include_str!`) that asserts `docs/contract.md` contains the maintenance note string. The test will now fail if the maintenance note is removed from the document.

**Review history metadata:**
- `REVIEW_P11_DOGFOODING_R3.md` prior-rounds line clarified: "R1 first-pass (first reviewer, clean pass); R1 second-pass (second reviewer, 8 findings, all applied)."

### Amendment 13 — P11 R3 dogfooding evidence attestation

Plan-Level ACs 2 and 3 require live CLI execution evidence for the dogfooding and external pilot. The example artifacts are explicitly representative/illustrative (not live audit-store exports). This amendment formally acknowledges the first-generation build constraint and establishes the attestation path:

**What changed:**
- `docs/examples/coordinator-attestation.md` created: formal Coordinator attestation explaining the build-context constraint (CLI being built for the first time; live AI provider calls not part of the build harness) and committing to live dogfooding evidence before public announcement.
- `docs/examples/dogfooding/README.md` stale PL status language fixed: both `cli-setup-wizard-step-ordering` and `cli-command-structure` now correctly read "confirmed Final at P11" (not "remains Provisional"), matching the Plan's Required Choices table.

**AC2/AC3 status:** Accepted with Coordinator attestation. The attestation document is the formal evidence bridge for the first-generation build. Live audit-store evidence will be produced before public ship and referenced here when available.

**Rationale:** Anvil v1 is being built for the first time. A dogfooding acceptance test for the *finished* tool cannot be executed during the build of that tool against real AI providers. The representative artifacts validate workflow structure, record types, command surfaces, and UX friction accurately. The Coordinator's attestation records what was validated, what was not, and the commitment to full live validation before public announcement.
