# Anvil Charter — Amendment A1

**Date:** 2026-05-19
**Status:** **Converged** — closed by human-arbiter declaration on 2026-05-19 after R1 + R2 rotation. Content applied to the Charter via the *Amendment A1 — Applied* section in `new_project_charter.md`. See `AMENDMENT_A1_CONVERGENCE.md` for the convergence record.
**Type:** Charter Amendment (post-convergence)
**Composition:** Three related constitutional additions proposed as a single document with **per-item disposition**: reviewers may converge on items individually, allowing partial adoption rather than all-or-nothing.
**Coordinator decision:** John Canady requested this amendment on 2026-05-19, four days after original Charter convergence (2026-05-15), to reflect Anvil's evolution from personal/internal tool to open-source flagship.
**R1 hardening:** see `AMENDMENT_A1_HARDENING_HISTORY.md`.
**R1 disposition document:** `REVIEW_CHARTER_AMENDMENT_A1_R1.md`.

---

## Scope

This amendment proposes three constitutional additions to the Anvil Project Charter:

1. **Anvil is open-source software** distributed under the Apache 2.0 license with documented open-source governance, security posture, and contribution policy.
2. **Artifact structures are defined** in the sibling specification document `ARTIFACT_SPECIFICATIONS.md` (Plan template, Phase Review Briefing template, Disposition document template, Findings Packet schema, and standard vocabularies).
3. **Anvil is designed as embeddable workflow infrastructure** — external programs may integrate with Anvil's workflow without forking, screen-scraping, or working around the CLI surface — with explicit embedding invariants and a structured-output stability contract.

The three items are presented together because they are interdependent (each makes Anvil more of an external-facing product). **However, per R1 review (P2 #1), this amendment supports per-item disposition:** reviewers may converge on items individually, and the Coordinator may declare partial convergence (e.g., Item 1 + Item 2 converged, Item 3 sent back for refinement) rather than all-or-nothing. See *Per-Item Disposition Mechanism* below.

---

## Motivation

Anvil's strategic role has evolved from "personal/internal tool" to **flagship open-source workflow infrastructure**. The Coordinator's strategic intent is that Anvil is the customer-acquisition vehicle for a services business: Anvil's quality demonstrates the company's discipline, Anvil's adoption seeds the audience for paid services built on the same workflow, and Anvil's open-source distribution gives the developer community a credible answer to the "vibe coding is fragile" critique.

This evolution propagates into the Charter's architectural commitments in ways the original Charter (pre-convergence on 2026-05-15) did not anticipate:

- **Open-source operations** introduce a third class of reviewers beyond the configured pool — the public, post-release — and require governance practices (Code of Conduct, security disclosure, contribution mechanism, semver discipline, threat modeling, dependency review, release signing) that internal-only operation does not.
- **Audit-store records may flow into public artifacts only through a controlled public-export path**, not by default. See *Public vs Private Audit Records* below — this is the R1-elevated safety boundary.
- **Embedding programs** depend on stable artifact structures. AiMe — the Coordinator's existing personal-cognition project, which is the natural first validation case for an external embedder — and any future embedder must be able to consume Anvil's Plans, Dispositions, and Findings Packets without screen-scraping markdown. The implicit pattern we have followed throughout Charter R1–R4 must become an explicit contract.
- **Embedding programs** depend on stable interfaces. The Vault library's public API, the audit store's schema, the artifact specifications, the structured CLI output, and the sidecar wire protocol are all public contracts under semver discipline. This is an architectural orientation, not a feature.

These are constitutional commitments because they propagate through every subsystem of Anvil. They belong in the Charter, not in a Plan-stage configuration block.

**On AiMe specifically (R1 P3 #5):** AiMe is named here as the *first anticipated validation case* for the embedding invariant — not as the primary design target. The constitutional embedding commitment must work for embedders that do not yet exist; AiMe is the case the Coordinator can prove against, not the case the architecture is shaped around.

---

## Proposed Amendment 1: Open-Source Distribution

### New Invariant

Add to the *Invariants (Never Violate)* section of the Charter:

> **Open-Source Distribution.** Anvil is distributed as open-source software under the Apache 2.0 license. The source repository becomes public at the **publication milestone** defined in *Publication Milestone* below. The project follows open-source governance practices: Contributor Covenant 2.1 Code of Conduct, DCO sign-off on all contributions with explicit AI-assistance and third-party-snippet provenance attestations, a documented security-disclosure policy in `SECURITY.md` (90-day window; threat-model document; vulnerability triage process; supported-version policy), and semver discipline for every public contract enumerated in *Contract Inventory* below. Forks are permitted under the license. The Anvil name's trademark posture is documented as a Required Project-Level Choice (see *Trademark Posture* below). The Coordinator role is fulfilled by the project's BDFL (Benevolent Dictator); the governance mechanics — maintainer admission/removal, conflict of interest handling, decision records, and BDFL-unavailable succession — are documented in `GOVERNANCE.md`. Evolution beyond BDFL-with-maintainers requires Charter amendment.

### Publication Milestone

(R1 P3 #2 + R2 Fix First #3 addressed.) "Source repository becomes public" is operationalized:

- The repository is created **private** at the start of P0 (Bootstrap) and remains private throughout v1 implementation.
- The repository **flips public** when v1 ships (per *Plan-Level Acceptance Criteria* item 11 — primary-platform binaries shipped, smoke tests passing) **AND the publication-safe-history gate below has cleared.**
- Audit-store records and hardening histories from pre-publication development are **selectively exported** into the public repo via the public-safe audit bundle (see *Public vs Private Audit Records*). Raw pre-publication records remain in private archival storage controlled by the Coordinator.

**Publication-safe git history gate (constitutional, R2 Fix First #3).** Pre-publication git history is preserved verbatim into the public repository — *but only after passing a publication-safe scan over the entire history*. The scan runs before the repository flips public:

- **Secret scan over full history.** A vetted secret-detection tool runs over every commit's content and message in the entire repository history (not just the current state). Any positive hit blocks publication until either (a) the secret is rotated and the history is selectively rewritten to remove the exposure, or (b) the secret is explicitly classified as a false positive and acknowledged by the Coordinator in an audit record.
- **License scan over full history.** A license-detection tool runs over every file ever introduced in any commit, flagging files whose declared or detected license is incompatible with Apache 2.0. Hits block publication until resolved (file removed from history; replaced with compatibly-licensed content; or explicitly justified as fair-use/nominative-use with an audit record).
- **Commit-message review pass.** The Coordinator (or a designated reviewer) reviews commit messages from the pre-publication period for non-secret-but-sensitive content: internal customer references, candid drafting language, third-party identifiers. Sensitive messages may be edited via interactive rebase before publication; rebases are themselves audit-record events naming what was changed and why.
- **Exceptional history rewrite allowance.** If the scans surface enough hits to make selective remediation impractical, the Coordinator may invoke a **one-time pre-publication history rewrite** that squashes pre-publication history into a smaller set of representative commits. The rewrite is itself an auditable event with the original history archived privately. After publication, no history rewrites are permitted — the publication state is the immutable starting point of public history.

The publication-safe-history gate is on the critical path for v1 ship. Acceptance: the scan tools' outputs are clean (zero unresolved hits) or every hit has an audit-record disposition (false-positive acknowledgment / remediation / exception).

### New Required Project-Level Choices

Add to the *Required Project-Level Choices* table in *Governance Taxonomy*:

| Choice | Lock Type | Value |
|---|---|---|
| Open-source license | Final | Apache 2.0 |
| Contribution mechanism | Final | DCO sign-off on all commits, extended with AI-assistance disclosure and third-party-snippet provenance attestations (see *DCO Extension* below). No CLA. |
| Governance model | Final | BDFL with optional maintainer additions; mechanics in `GOVERNANCE.md` (see *Governance Mechanics* below) |
| Code of Conduct | Final | Contributor Covenant 2.1 |
| Repository host | Final | GitHub primary (`https://github.com/ai-nhancement/Anvil`); optional Codeberg mirror permitted but not required |
| Security disclosure policy | Final | `SECURITY.md` with private email + GPG-key contact, 90-day disclosure window, threat-model summary, vulnerability triage workflow, supported-version policy (see *Security Posture as Constitutional* below) |
| Trademark posture | Final | **Posture A — no registration** (locked by Coordinator 2026-05-19 in response to R1 P2 #4). README naming-preference statement requests but does not legally require that forks adopt distinct names. Re-evaluation triggered if a confusing fork appears during v1 ramp; defensive registration remains an option for v1.x if adoption patterns warrant. |
| Publication milestone | Final | Repository private during P0–P11 implementation; flips public at v1 ship; pre-publication git history preserved; audit records exported via public-safe bundle |
| Dependency review and SBOM | Final | **Outcome (constitutional):** every release produces a Software Bill of Materials and runs language-appropriate vulnerability scanning at every CI run; SBOM signed alongside release artifacts. **Default tools (Plan-stage; replaceable without amendment):** CycloneDX format for SBOM; `cargo audit` (Rust) + `govulncheck` (Go) for scanning. The *outcome* is constitutional; the *tools* are Plan-defaults that the Plan may revise as ecosystem standards evolve. |
| Release signing | Final | **Outcome (constitutional):** release artifacts (binaries + SBOM + checksums) are cryptographically signed atomically with the project's signing key, in a format that consumers can verify offline. **Default mechanism (Plan-stage):** GPG-signed `SHA256SUMS.txt.asc` per platform release archive. The Plan may adopt alternative signing (sigstore, in-toto attestations) without amendment if the outcome (offline-verifiable, atomic with release) is preserved. |
| Secret scanning | Final | **Outcome (constitutional):** committed secrets are detected and blocked before they reach the public repository, at both pre-commit and CI stages. **Default tool (Plan-stage):** a vetted secret-detection tool, currently `gitleaks` as Plan default. The Plan may swap detectors without amendment provided the outcome (pre-commit + CI block on detection) is preserved. |

### Governance Mechanics

(R1 P1 #4 addressed.) `GOVERNANCE.md` ships as part of the publication milestone and contains, at minimum:

- **BDFL role and authority.** The BDFL has final authority over Charter amendments, ship/no-ship decisions, and disputes. The BDFL operates under the same Convergence Safeguards as any other Coordinator role — declarations are logged audit records with required reasoning.
- **Maintainer admission.** A contributor may be invited to maintainer status by the BDFL after demonstrated sustained contribution (minimum: three shipped phase contributions or equivalent reviewer-quality findings across multiple review rounds). Invitations are public; the rationale is logged.
- **Maintainer authority.** Maintainers may merge PRs that fall within their declared focus area (per `GOVERNANCE.md`'s maintainer-by-area table). Maintainers may not amend the Charter, declare convergence, or override the BDFL on contested decisions.
- **Maintainer removal.** A maintainer may be removed (a) at their own request, (b) by BDFL decision with public rationale, or (c) by sustained inactivity (≥6 months without contribution or response). All removals are logged.
- **Conflict of interest.** Maintainers and the BDFL must disclose financial, employment, or familial relationships with parties affected by Anvil decisions. Conflicts are surfaced in `GOVERNANCE.md`'s active-disclosures section and on individual decisions where relevant.
- **Decision records.** Significant project decisions (Charter amendments, major spec changes, governance changes) are logged as `decision-record` audit-store records and published in the public-safe bundle.
- **BDFL succession / unavailability.** If the BDFL is unavailable for >30 consecutive days (no responses to PRs, issues, or maintainer pings), the maintainer pool may elect a temporary acting-BDFL by simple majority. The acting-BDFL may not amend the Charter or declare convergence; they may merge non-controversial PRs and triage security issues. The acting-BDFL role expires when the BDFL returns, or after 90 days transitions to a maintainer-quorum mode that requires majority maintainer agreement for any single decision. The BDFL may name a designated successor in `GOVERNANCE.md` who would assume the role permanently if the BDFL becomes permanently unavailable.
- **BDFL-adversarial emergency (R2 Medium #9).** Distinct from unavailability: if the BDFL's credentials are believed compromised, or if the BDFL is taking actions believed hostile to the project (unilateral repository deletion, sudden license changes inconsistent with the Charter, mass-banning of maintainers without rationale), the maintainer pool may invoke an **emergency freeze**. The freeze is declared by a two-thirds majority of active maintainers, takes effect immediately, and halts all merges, releases, and governance-affecting actions until either (a) the BDFL responds and resolves the concern, (b) the maintainer pool unanimously certifies the BDFL is restored, or (c) the maintainers rotate to a new BDFL via the succession process. The freeze itself is an audit-record event (`emergency-freeze-declaration`) — a new record type added to v1's audit-store schema (16th record type; Plan Draft 7 reflects). Freezes are intentionally narrow: they pause project activity rather than transfer authority, so the legitimate BDFL retains ownership if the concern proves unfounded.

### Security Posture (constitutional)

(R1 P1 #3 addressed.) The Required Choices table above elevates threat modeling, dependency review, SBOM, release signing, and secret scanning from "Plan follow-ups" to constitutional Required Choices. `SECURITY.md` contains:

- **Threat-model summary.** What Anvil defends against (accidental credential exposure, contributor mistakes, supply-chain dependency vulnerabilities, audit-store tampering — local detection only per R3) and what it explicitly does not (adversarial tamper-proofing of audit store; nation-state actor protection; secure multi-party hosting).
- **Vulnerability triage roles.** Who reads `SECURITY.md` reports, who acknowledges within the 90-day window, who decides on coordinated disclosure timing, who issues advisories.
- **Supported-version policy.** Which versions receive security fixes (current major + previous major; older versions are end-of-life). Documented before v1.0 ships.
- **Coordinated disclosure workflow.** From private report → acknowledgment → assessment → fix → public advisory.

### DCO Extension — AI Assistance and Provenance

(R1 P2 #5 addressed.) Standard DCO sign-off is required on all commits. The DCO extension requires contributors to attest, in commit messages or PR descriptions:

- **AI-assistance disclosure.** Any non-trivial code, prose, or artifact authored with substantial AI assistance is disclosed via a `AI-Assisted-By:` trailer naming the AI system used (e.g., `AI-Assisted-By: Claude`). Trivial AI assistance (autocomplete, single-line suggestions) does not require disclosure; substantive authorship (the AI produced significant structure or logic) does.
- **Third-party snippet provenance.** Code copied or adapted from external sources is disclosed via a `Derived-From:` trailer naming the source and its license. Snippets incompatible with Apache 2.0 are rejected at PR review.
- **Generated-content acknowledgment.** Contributors confirm they have the right to contribute the code under the project's license, including any AI-generated portions. (This is the standard DCO #3 condition; the AI extension makes it explicit for generated content.)

The CI's DCO check is extended to look for the new trailers on PRs that touch **substantive code paths**, defined operationally (R2 Medium #10):

- *Substantive code path:* any PR that adds ≥20 lines of new code in a single commit OR introduces a new file OR modifies code inside `crates/anvil-core/`, `crates/anvil-audit/`, `crates/anvil-graph/`, `crates/anvil-sidecar-client/`, or `sidecar/internal/`. PRs that only touch documentation, tests, build configuration, or formatting are not "substantive code paths" for DCO-extension purposes (though the standard DCO sign-off still applies).
- *Persistently:* the trailer-check is a warning on the first PR submission. If the PR is updated (force-push or new commit) and trailers are still missing, the check escalates to a *blocking* status. The PR cannot merge until trailers are added. This gives contributors one cycle to add missing trailers without churn; subsequent updates without trailers block merge.

The exact thresholds (20 lines, listed crates) are operational and may be tuned by Plan-stage refinement without requiring Charter amendment, provided the principle (substantive AI/third-party-snippet contributions are disclosed) is preserved.

### Trademark Posture (locked: Posture A)

(R1 P2 #4 raised this as a strategic concern; the Coordinator made the decision on 2026-05-19.) The Draft 1 amendment proposed "no trademark registration; naming-preference statement in README." R1 review correctly observed that this maximizes fork freedom but may underprotect the brand clarity needed for Anvil's services-led customer-acquisition role: a third party could ship an inferior fork under the Anvil name, confusing prospective customers.

Two postures were evaluated:

- **Posture A (now locked).** No trademark registration. README naming-preference statement requests but does not legally require that forks adopt distinct names.
- **Posture B (considered, declined).** Defensive trademark registration with nominative-use carve-outs in a `TRADEMARK.md` document.

**Coordinator decision:** Posture A is locked for v1. Rationale: maximum fork freedom suits an early-stage open-source project where adoption patterns are unproven; defensive registration carries cost (fees + enforcement effort) that is not yet justified by demonstrated brand-confusion risk; Posture B remains available as a v1.x consideration if adoption patterns produce confusing forks during the v1 ramp.

**Re-evaluation trigger:** if a confusing fork appears during v1 ramp that materially harms users or the services-acquisition role, the Coordinator re-evaluates Posture B as a Charter amendment. Defensive registration is harder to obtain retroactively (prior-art arguments), so the trigger should fire on first confusing-fork sighting, not on accumulated harm.

### Risks Addressed

- **Public scrutiny.** The credibility cost of shipping fragile open-source workflow infrastructure is permanent. This invariant + the elevated security posture + dependency/SBOM/signing controls commit the project to a higher quality bar than internal-only operation.
- **Contributor fragmentation.** Without explicit governance and contribution discipline, external contributors will invent ad-hoc patterns. Mitigation: Item 2 (artifact structures), Item 1's governance mechanics, and the DCO extension all close specific drift surfaces.
- **API stability obligations.** External consumers depend on every contract enumerated in *Contract Inventory*. Semver discipline is now constitutional for each.
- **Audit-store leakage.** Without a public/private distinction, well-meaning publication could expose prompts, model outputs, project secrets, vulnerability details. Mitigated by *Public vs Private Audit Records*.
- **Governance bus-factor.** A single BDFL is a single point of failure. Mitigated by the succession mechanism and acting-BDFL provisions in `GOVERNANCE.md`.

---

## Public vs Private Audit Records

(R1 P1 #2 — the most consequential R1 finding.) The Draft 1 amendment said "audit-store records become public artifacts when the repo is public." This is unsafe as written: existing audit records may contain prompts, raw model outputs, project context, references to credentials, vulnerability discussion, third-party code excerpts, and other content that is fine in private development but harmful if exposed.

This amendment adds a constitutional distinction between two record classes:

### Two record classes

- **Local-private audit records.** Default class. Held in the project workspace's local `audit-store/`. Never automatically published. May contain any content the workflow produces — prompts, model outputs, raw findings packets, credentials references, internal discussions.
- **Public-project audit records.** Explicit export of selected records via a documented public-safe bundle process. Sanitized of secrets, credentials references, third-party code that isn't license-compatible, and any content the Coordinator flags as private.

### Public-safe audit bundle (mechanism)

Add to `anvil-audit` (P2 deliverable, extended): an `anvil audit export --public` command that produces a `public-audit-bundle/` directory containing the subset of audit records explicitly approved for public release. The bundle process (R2 Fix First #4 tightening):

1. **All records private by default.** Every audit record is private at creation; no record type defaults to public-safe. Record types may carry a **`recommended_visibility`** metadata field (`recommended_visibility: "publication-suitable"` or `"local-only"`) as guidance for the Coordinator's curation decision, but the recommendation never grants automatic public status. Even `ConvergenceDeclaration` records — which are typically structural rather than sensitive — remain private until the Coordinator explicitly approves them for the bundle. Reasoning: convergence-declaration reasoning text *can* contain sensitive content (vulnerability discussion, private project context); a record-type-default policy makes it too easy to publish such content accidentally.
2. **Per-record explicit opt-in.** The Coordinator marks individual records (not types) as `public_visibility: "approved"` via an explicit action. Bulk approval is permitted only with a per-record diff review.
3. **Multi-pass sensitivity scan** (replaces the brittle quoted-strings heuristic from Draft 2, per R2 Important Fix #6):
   - *Secret scan.* A vetted secret-detection tool runs over the record content; positive hits block export.
   - *License scan.* Records containing quoted third-party content are flagged; license-incompatible content blocks export.
   - *Structured sensitivity labels.* Records carry optional `sensitivity_label` fields (`secret`, `internal-customer-reference`, `vulnerability-discussion`, `third-party-quoted-content`, `pii`) set by record authors at creation. Labels block export by default; the Coordinator must explicitly approve any labeled record for inclusion.
   - *Coordinator manual review.* No automated pass can replace human judgment for "is this content appropriate to publish?" Every record approved for export is acknowledged by the Coordinator individually with a reason captured in the approval record.
4. **Coordinator review gate.** The export produces a diff between the records (with applied redactions) and the original records. The Coordinator must explicitly approve the diff before the bundle is published. Approval is a `PublicExportApproval` audit record naming every record included and the Coordinator's per-record reasoning summary.
5. **Cryptographic seal.** The published bundle is signed by the project's GPG key so consumers can verify it has not been tampered with after export.

**No automatic publication path exists.** Even records the Coordinator clearly wants public (e.g., final converged Charter) go through the explicit per-record approval gate. The friction is intentional: workflow tools whose value depends on durable artifacts should not also be in the business of publishing those artifacts automatically.

### Audit-store schema extensions

The audit-store record-type list (currently 13 in v1 post-R4) gains two new types:

- `PublicVisibilityPolicy` — per-record-type `recommended_visibility` metadata and per-record approval overrides (per R2 Fix First #4: recommendation only, never automatic).
- `PublicExportApproval` — Coordinator's explicit approval of an export bundle, with diff hash and bundle SHA-256.

Total v1 audit-store record types (post-Amendment A1 Draft 3): **16** (11 Charter-required + 5 Plan extensions: `ArbiterFindingResolution`, `SidecarReload`, `PublicVisibilityPolicy`, `PublicExportApproval`, `EmergencyFreezeDeclaration` — the latter added in Draft 3 per R2 Medium #9). **Plan Draft 7 must update record-type total from 13 to 16, including the constitutional hinge `test_audit_store_required_types_present` (the subset check is unchanged; only the per-implementation total grows).**

### What flips public at v1 ship

- The Charter, the converged Plan, the Artifact Specifications, all hardening histories, all convergence declarations, all disposition documents — these are inherently public-safe by content type and are exported with light redaction (removing any credentials references that may have appeared in early drafts).
- Reviewer findings packets are exported in *summary form* (the Coder's disposition prose is public; the raw reviewer-emitted text and prompt context are not, unless the Coordinator opts specific records in).
- Pre-publication development discussion transcripts (this conversation, for instance) are *not* exported automatically — they may contain decisions in progress that are not load-bearing on the final state. The Coordinator may opt to publish curated excerpts.
- The repository's git history is preserved verbatim (per *Publication Milestone*), which means pre-publication commit messages are public. Commit messages must follow the publication-safe convention from day one — no API keys, no internal customer references, no information the Coordinator does not want public.

---

## Contract Inventory

(R1 P1 #1 addressed.) The amendment makes multiple things "public contracts." This section enumerates them with explicit owner, versioning policy, and migration policy.

| Contract | Owner | Versioning | Migration policy |
|---|---|---|---|
| **`anvil-core` Vault library public API** | Coder (BDFL-final-approval) | Semver per crate version. Major bumps require Charter amendment. | Breaking change: 6-month deprecation window minimum; deprecated symbols continue to compile with deprecation warnings for one major version |
| **Audit-store record schemas** | Coder (BDFL-final) | Per-record-type semver. New record types are minor bumps; field additions are minor; field removals or type changes are major. | Major bump: writers may emit either old or new shape during a one-Plan-phase migration window. Readers must accept both. |
| **`ARTIFACT_SPECIFICATIONS.md` templates** | Coder (BDFL-final) | Document-level semver per the spec's own versioning policy. **Per R1 P1 #5: major template changes require full Charter amendment, not narrow review.** Minor/patch changes follow the narrower review process documented in the spec. | Major spec change: artifacts authored against the old spec remain valid for one Plan-phase boundary; new artifacts use the new spec. |
| **Sidecar wire protocol (`anvil.v1.*` protobuf)** | Coder (BDFL-final) | Package-level semver. `anvil.v1` is the current major. `anvil.v2` would be a new package. Both sides must support at least one common package version. | Major bump: sidecar and Vault each advertise supported packages in the handshake; mismatch is a startup error. |
| **Structured CLI output (`--format json`)** | Coder (BDFL-final) | Schema files in `schemas/cli/` per command, semver per schema file. Outputs include a `schema_version` field. | Major bump: CLI commands accept an `--output-schema-version` flag for one minor version after a major bump so scripts can pin. |
| **Stable machine-readable error codes** | Coder (BDFL-final) | Per-class semver; error-code stability is part of the CLI structured-output contract above. | New error codes are minor additions; renaming or removing existing error codes is a major bump. |

**Charter amendment is required for major bumps to the `anvil-core` API, audit-store record schemas, the artifact specification document (major changes only), the sidecar wire protocol (major package change), and the CLI structured-output schemas (major). Minor and patch changes follow each contract's documented review process.**

---

## Proposed Amendment 2: Defined Artifact Structures

### New Invariant

Add to the *Invariants (Never Violate)* section:

> **Artifact Structures Are Defined.** All canonical workflow artifacts — the Plan document, the Phase Review Briefing, the Disposition document, the Findings Packet — conform to templates and schemas specified in the project's sibling document `ARTIFACT_SPECIFICATIONS.md`. The Coder produces artifacts that satisfy the specifications; reviewers consume artifacts that satisfy the specifications; embedding programs depend on artifacts that satisfy the specifications. **Major changes** to the specification document (per its semver) **require full Charter amendment**, not narrow review (R1 P1 #5 tightening). Minor and patch changes follow the narrower review process documented in the spec. Migration window for major changes: one Plan-phase boundary during which both old and new shapes are accepted.

### New Required Project-Level Choice

(See *Contract Inventory* above for the cross-reference.)

| Choice | Lock Type | Value |
|---|---|---|
| Artifact specifications | Final | `ARTIFACT_SPECIFICATIONS.md` (sibling document); covers Plan template, Phase Review Briefing template, Disposition template, Findings Packet schema, and standard vocabularies. Versioning per spec's documented semver policy with the R1 tightening that major changes require full Charter amendment. |

### Why This Is Constitutional

For an open-source workflow tool, the artifact templates are the contract between the project and its contributors and embedders. Drift is opaque-by-default unless the spec is canonical.

### Risks Addressed

- **Template drift.** Without explicit templates, contributors will invent variants. Mitigated by the spec document + the strengthened amendment-required path for major changes.
- **Backward incompatibility.** Migration window protects existing artifacts.
- **Bypass via "narrower review."** R1 P1 #5 correctly noted that allowing all spec changes (including major) through narrow review could bypass constitutional oversight. The Draft 2 tightening closes this.

---

## Proposed Amendment 3: Embeddable Workflow Infrastructure

### New Invariant

Add to the *Invariants (Never Violate)* section:

> **Embeddable by Design.** Anvil is designed as workflow infrastructure that external programs may embed, not solely as a standalone tool. The Vault library (`anvil-core`) exposes a clean command/query API with no terminal-shaped, CLI-shaped, or App-shaped assumptions. The audit store schema is a public contract under semver discipline. The CLI provides structured-output modes (`--format json`) with schema files and explicit `schema_version` in every output, per *Structured CLI Output Stability* below. The full embeddable surface (network service, typed client libraries, gate-delegation protocol) is scoped as v1.2 development under explicit *Embedding Invariants* below. v1 ships only the "keep the door open" pieces; v1 must not introduce architectural decisions that preclude or significantly complicate the v1.2 surface.

### Embedding Invariants (constitutional)

(R1 P2 #2 addressed.) The full embeddable surface in v1.2 must satisfy these non-negotiable invariants. Stating them now prevents v1 architecture from foreclosing them:

- **Vault remains the trust authority.** Embedders may invoke workflow operations, but the Vault enforces every invariant (single-writer, family-floor, gate sequencing, audit-record creation). Embedders cannot bypass.
- **No gate bypass.** Embedding programs do not skip human gates. They may provide their own UX for surfacing gates (e.g., a chat embedder may render a gate as a confirmation prompt within the chat), but the gate must be human-decided. Automated approval by an embedder is forbidden.
- **Typed API only.** Embedders interact via the typed embedded contract (the v1.2 network service or client libraries). No screen-scraping the CLI; no parsing arbitrary CLI text output. The structured `--format json` mode is the v1 embedded surface; v1.2 adds richer typed surfaces.
- **Audit records mandatory.** Every embedder-initiated operation produces audit records identical to CLI-initiated operations. Embedders do not have "silent" modes. The `embedder_identity` field on operations identifies which embedder issued the request.
- **Per-embedder authentication.** Any embedded surface (network service, in-process linkage, local IPC, or other) requires per-embedder authentication. Tokens or equivalent credentials are managed by the Vault, scoped per workspace, and revocable. **The principle is locked; the specific authentication form is provisional** and selected in the v1.2 design cycle based on the chosen transport mechanism (R2 Medium #12: don't over-lock transport form before v1.2 design).
- **Multi-tenant isolation.** A single Anvil installation serving multiple embedding programs maintains per-embedder workspace isolation. Embedders cannot read or modify another embedder's workspace state. (v1.2 commitment.)

**Embeddable transport form (intentionally provisional, R2 Medium #12).** Draft 2 locked specific transport forms ("network service + typed client libraries + gate-delegation protocol"). R2 correctly noted that locking the specific form before v1.2 design risks foreclosing better alternatives. **Draft 3 commits only to the principles above (Vault authority, no gate bypass, typed API only, audit records mandatory, per-embedder auth, multi-tenant isolation).** The specific transport — network service, in-process linkage, local IPC, FFI bindings, or some combination — is a v1.2 design choice. The v1.2 Plan will surface the transport selection through its own Charter amendment cycle when the design is concrete.

### Structured CLI Output Stability (constitutional)

(R1 P2 #3 addressed.) `--format json` becomes a de facto API. v1 ships with:

- **Per-command JSON schemas.** Every command supporting `--format json` ships a JSON Schema file in `schemas/cli/<command>.json`. The schema is part of the release artifact.
- **`schema_version` in every output.** Every JSON output includes a top-level `schema_version` field (semver). Consumers pin to a version; the CLI's `--output-schema-version` flag lets scripts request a specific major if needed.
- **Stable machine-readable error codes.** Errors emitted via `--format json` include a stable `error_code` string drawn from an enumerated set documented in `schemas/cli/errors.json`. Error-code stability is part of the contract; renames/removals require major bumps per *Contract Inventory*.
- **Compatibility test suite.** Every release runs a compatibility check against the prior major version's schemas. Backward-incompatible drift fails CI.
- **Schema discovery via CLI (R2 Medium #11).** The CLI supports `--describe-schema` on every command that emits structured output. The flag returns the JSON Schema for that command's `--format json` output without requiring filesystem access to `schemas/cli/`. This is mandatory because subprocess embedders cannot rely on the filesystem layout of the schemas directory — the schema must be discoverable through the same channel as the output. Implementation: each command's schema is embedded into the binary at build time alongside the command implementation; `--describe-schema` reads from the embedded copy.

### New Required Project-Level Choice

| Choice | Lock Type | Value |
|---|---|---|
| Embeddable scope | Final | v1: structured CLI output (with per-command JSON schemas + `schema_version` + stable error codes per *Structured CLI Output Stability*) + clean Vault library API + public audit-store schema. v1.2: full embedding surface (network service, typed client libraries, gate-delegation protocol) satisfying *Embedding Invariants*. v1 must not introduce architectural decisions that preclude the v1.2 surface. |

### Risks Addressed

- **Architectural lock-in.** Without commitment, v1 could optimize for CLI in ways that foreclose v1.2. Mitigated by the invariant.
- **Differentiation moat erosion.** Workflow + cross-vendor reviews + open source + embeddability is the four-leg stack. Making each leg explicit in the Charter prevents quiet abandonment.
- **Hidden second brain.** Embedders absorbing workflow logic would defeat the bounded-authority story. Mitigated by *Embedding Invariants*' Vault-authority rule.
- **API drift.** Without schemas/`schema_version`/error codes, `--format json` would become a de facto API with no compatibility guarantees. Mitigated by *Structured CLI Output Stability*.

---

## What This Amendment Does NOT Change

To be explicit about non-goals:

- **No existing Charter invariant is amended or weakened.** All R4 Charter invariants remain in force.
- **The Coordinator role is not redefined.** The BDFL is the same role with a different audience and explicit governance mechanics.
- **The reviewer pool composition (Codex-class + Gemini-class)** is not changed. Public post-release readers are advisory, not pre-Ship reviewers.
- **The 13 Plan-v1 audit-store record types** are extended to 15 (adding `PublicVisibilityPolicy` and `PublicExportApproval`); the original 13 are unchanged.
- **The converged Plan's phase decomposition** is not invalidated. The Plan will need a Draft 7 *impact matrix* (see below) to reflect open-source-readiness work, but the 15-phase structure is largely preserved.

---

## Plan Draft 7 Impact Matrix (R1 P3 #4 requirement)

After this amendment converges, the Plan requires a **Draft 7 revision with an explicit impact matrix** showing which phases gain work from the amendment. The matrix is a Plan-stage deliverable, not part of this amendment's content; this section locks the requirement.

Anticipated phase impacts (illustrative; full matrix produced in Draft 7):

| Phase | Amendment impact |
|---|---|
| P0 | LICENSE, NOTICE, CONTRIBUTING.md, CODE_OF_CONDUCT.md, SECURITY.md, GOVERNANCE.md, DCO check in CI, secret-scanning pre-commit hook, SBOM generation pipeline, release-signing setup |
| P2 | Two new record types (`PublicVisibilityPolicy`, `PublicExportApproval`); public-export bundle command; redactor pass; cryptographic seal mechanism |
| P4 | DCO-extension trailers documented in the setup wizard's CI/headless guidance |
| P7 | Plan-template validators that check Plan documents against the new spec versioning rules |
| P10a | Metrics collection for security-relevant events (secret-scan hits, dependency-vulnerability counts) |
| P11 | Public-export bundle smoke test; `docs/p4-walkthrough.md` becomes public; pre-publication-to-public audit-record migration smoke test; trademark posture finalization (if Posture B chosen, registration certificate stored in audit) |

The Plan Draft 7 fills in concrete acceptance criteria, hinge tests, and rounds-to-convergence per impacted phase. Plan Draft 7 itself goes through its own review cycle.

---

## Repo-Readiness Acceptance Gates (R1 P3 #1 addressed)

The amendment is not satisfied — and the publication milestone cannot fire — until the following are in the repository:

1. `LICENSE` (Apache 2.0 text)
2. `NOTICE` (attributions; third-party licenses)
3. `README.md` (with trademark posture statement per Coordinator decision)
4. `CONTRIBUTING.md` (with DCO + AI-assistance + third-party provenance requirements)
5. `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1)
6. `SECURITY.md` (per *Security Posture as Constitutional*)
7. `GOVERNANCE.md` (per *Governance Mechanics*)
8. `TRADEMARK.md` (only if Posture B is chosen)
9. CI configuration that runs: DCO check; secret scanning; dependency vulnerability scanning (`cargo audit` + `govulncheck`); artifact-spec validators; structured-CLI compatibility checks
10. Release-signing workflow with GPG key documented in `SECURITY.md`
11. SBOM generation step producing CycloneDX-format SBOM per release
12. Public-safe audit bundle export validated against the in-repo bundle (the repo's own bundle must round-trip through the export command without changes — proves the export mechanism works on the project's own data)

---

## Per-Item Disposition Mechanism

(R1 P2 #1 addressed.) This amendment composes three items in one document for review efficiency, but reviewers may converge on items independently. Mechanically:

- Reviewers structure findings per-item (Item 1 / Item 2 / Item 3) where natural; findings that span multiple items are noted as such.
- The Coordinator's disposition (per-round) lists per-item convergence status: **Item 1 converged / pending / refuted**, same for Items 2 and 3.
- A round may close some items and continue iterating others. The amendment ships when all three items converge, or when the Coordinator declares partial-acceptance and explicitly refuses one or more items.
- If an item is refused, it is removed from the amendment text and the remaining items proceed. A refused item may be resubmitted as a separate future amendment.

For Draft 3, all three items are still in scope; no item has been refused.

### Cross-Document Convergence (R2 Important #7)

The amendment and the sibling `ARTIFACT_SPECIFICATIONS.md` are in concurrent review. R2 correctly noted that Draft 2 did not define how to handle cross-document conflicts. Draft 3 closes this:

- **Same reviewer pool.** Both documents are reviewed by the same configured reviewer pool. This ensures the diversity floor applies consistently and prevents one document from being reviewed by a vendor that contradicts the family-floor for the other.
- **Round-aligned dispositions.** A reviewer producing findings on either document in a given round is expected to flag any cross-document inconsistency they notice (e.g., "the amendment says X about the spec; the spec says Y"). Such findings get **`cross-document`** as their primary label and are addressed in both documents by the Coder in the round's disposition.
- **Convergence requires both.** "Artifact specifications converged" means **both `ARTIFACT_SPECIFICATIONS.md` and the relevant parts of this amendment that reference it have cleared the same set of findings**. The amendment cannot converge with respect to Item 2 if the spec has unresolved findings, and vice versa.
- **Conflict resolution rule.** If a finding identifies a contradiction between the two documents that cannot be resolved by editing both to agree (e.g., a structural disagreement about which document owns a given commitment), the Coordinator decides which document holds the authoritative version and edits the other to defer. The decision is logged as a `decision-record` audit-store entry. Default rule: constitutional invariants live in the amendment (and thus, after convergence, in the Charter); operational details live in the spec.

---

## Questions for Amendment Reviewers (Draft 2)

R1 reviewer findings are addressed in Draft 2; the questions below are for R2 reviewers (or a convergence call from the Coordinator).

1. **Public/private audit boundary.** Is the public-safe audit bundle mechanism (`anvil audit export --public` + default-deny + per-record opt-in + redactor + Coordinator review gate + cryptographic seal) sufficient to defend against accidental publication of sensitive content? Specifically: any failure modes not covered?
2. **Contract Inventory coverage.** The inventory enumerates six contracts. Are any others (e.g., the workspace file-system layout, the per-project config schema, the hinge-test annotation conventions) constitutional and missing?
3. **Governance bus-factor.** The acting-BDFL + designated-successor mechanism handles BDFL unavailability. Does it cover the case where the BDFL becomes adversarial (compromised credentials, hostile takeover attempt)? Is a maintainer-quorum override for adversarial-BDFL events warranted, or out of v1 scope?
4. **Trademark posture (locked: Posture A).** R1 raised the trademark question; the Coordinator locked Posture A (no registration) on 2026-05-19 with a re-evaluation trigger documented in the *Trademark Posture* section. R3 reviewers may surface residual risks of Posture A (e.g., specific confusing-fork scenarios not yet anticipated) but the headline choice is settled.
5. **DCO extension and AI assistance.** The `AI-Assisted-By:` trailer documents AI involvement but does not regulate it. Is that the right calibration, or should certain classes of AI-generated contribution (e.g., bulk code generation without explicit human review per change) require additional review?
6. **Embedding Invariants timing.** The v1.2 commitments (network service, client libraries, gate-delegation, per-embedder auth, multi-tenant isolation) are locked here even though implementation is post-v1. Is locking them at amendment time appropriate, or should v1.2 have its own Charter amendment cycle when the design surfaces?
7. **Structured CLI output stability — schema discovery.** Schemas live in `schemas/cli/`. Should the CLI expose a `--describe-schema` flag so consumers can fetch the schema for a given command without filesystem access (e.g., for embedders that consume CLI output through subprocess pipes)?
8. **Per-item disposition.** Is the per-item mechanism (Item 1 / Item 2 / Item 3 converge independently) appropriate, or should related items be required to converge together (e.g., embedding invariants depend on structured-output stability — can Item 3 converge without Item 1's stability rules)?

---

## Acceptance Criteria

The amendment is satisfied — and the Charter is ready to be updated — when:

1. All three items (Open-Source Distribution, Defined Artifact Structures, Embeddable Workflow Infrastructure) have been reviewed through R-N rounds and either each converged or explicitly refused per *Per-Item Disposition Mechanism*.
2. Hardening notes for each round are appended to `AMENDMENT_A1_HARDENING_HISTORY.md`.
3. Per the locked termination condition (full-pool clean), every reviewer in the pool has produced a clean pass on the most recent amendment state for each remaining item, OR the Coordinator has declared human-arbiter convergence per *Convergence Safeguards*.
4. The `ARTIFACT_SPECIFICATIONS.md` sibling document has converged through its own concurrent review cycle (which may overlap with the amendment review).
5. The Coordinator's Trademark Posture decision is locked. (Closed 2026-05-19; Posture A locked. See *Trademark Posture* section and `AMENDMENT_A1_HARDENING_HISTORY.md`'s *Coordinator Decisions (post-R1)* entry.)
6. The repo-readiness gates (12 items above) are achievable in the converged Plan Draft 7 — i.e., the Plan accommodates their implementation. (Actual implementation happens at v1 ship per the publication milestone; the gate here is "Plan covers the work," not "work is done.")
7. The Plan Draft 7 impact matrix is produced and reviewed.

---

## Provenance

- **Amendment proposed:** 2026-05-19 by Coordinator (John Canady).
- **Original Charter convergence:** 2026-05-15 (`CHARTER_CONVERGENCE.md`).
- **Days between original convergence and amendment:** 4.
- **Trigger:** Coordinator's strategic decision to open-source Anvil as customer-acquisition vehicle, combined with two related architectural needs (artifact structures defined; embeddable design) surfaced during the same conversation.
- **R1 review reviewer:** R1 reviewer family per Charter rotation; different model family from Coder per Adversarial Diversity floor.
- **R1 findings count:** 15 (5 P1, 5 P2, 5 P3); 14 Fixed in Draft 2; 1 (Trademark Posture) pending Coordinator decision.
- **Composition rationale:** all three items remain constitutional in scope; R1's per-item disposition mechanism allows partial adoption if any item is refused in later rounds.

---

## Bottom Line

Draft 2 absorbs R1's substantial review. The major additions:

- **Public vs Private Audit Records** with a default-deny export bundle, Coordinator-review gate, and cryptographic seal — closing the most consequential R1 finding (P1 #2: publication safety).
- **Contract Inventory** with explicit owner, versioning, and migration policy for six public contracts (Vault API, audit-store records, artifact specifications, sidecar wire protocol, structured CLI output, error codes) — closing R1 P1 #1.
- **Security Posture as constitutional** (threat modeling, dep scanning, SBOM, signing, secret scanning, vulnerability triage roles, supported-version policy) — closing R1 P1 #3.
- **Governance Mechanics** (maintainer admission/removal, succession, conflict of interest, decision records, BDFL-unavailable provisions) — closing R1 P1 #4.
- **Artifact spec major-change-requires-Charter-amendment** tightening — closing R1 P1 #5.
- **Per-Item Disposition Mechanism**, **Embedding Invariants**, **Structured CLI Output Stability**, **DCO Extension**, **Publication Milestone**, **Repo-Readiness Acceptance Gates**, **Plan Draft 7 Impact Matrix requirement**, AiMe reframing.

One R1 finding (P2 #4, Trademark Posture) is intentionally left **pending Coordinator decision** rather than auto-resolved: the strategic question (no registration vs defensive registration with nominative-use carve-outs) is the Coordinator's to make.

Two new audit-store record types added (`PublicVisibilityPolicy`, `PublicExportApproval`), bringing the Plan's record-type total from 13 to 15 in v1.

Next step: Coordinator's decision on Trademark Posture, then R2 amendment review (or convergence call). The Plan Draft 7 impact-matrix workstream is a follow-on after amendment convergence.
