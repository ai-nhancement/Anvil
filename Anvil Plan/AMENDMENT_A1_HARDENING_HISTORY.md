# Charter Amendment A1 — Hardening History

This document is the **provenance log** for Charter Amendment A1 (Open-Source Distribution + Defined Artifact Structures + Embeddable Workflow Infrastructure). The amendment itself (`CHARTER_AMENDMENT_A1.md`) is the canonical proposal text. This file records round-by-round changes folded in through Amendment Review.

Per *Cross-Reference Integrity*, every consolidated hardening note here corresponds to a disposition document (`REVIEW_CHARTER_AMENDMENT_A1_R<N>.md`).

When the amendment converges, the constitutional invariants are applied to the Charter, the amendment document remains as a historical artifact, and a summary of these hardening notes is appended to `CHARTER_HARDENING_HISTORY.md` to preserve the lineage from the converged Charter back to the amendment process.

---

## Hardening Notes (R1 — Consolidated)

R1 came from the configured reviewer pool. Fifteen findings raised (5 × P1, 5 × P2, 5 × P3). Fourteen Fixed in Draft 2; one (P2 #4, Trademark Posture) intentionally left as Coordinator decision rather than auto-resolved.

R1's central thesis: Draft 1 named the *direction* of open-source / artifact-structure / embeddable commitments correctly, but underspecified the *operational guardrails* in five constitutionally-significant places — versioned contracts, public/private audit boundaries, security posture depth, governance bus-factor, and the artifact-spec amendment path. The Draft 2 changes address each.

### 1. Contract Inventory added (P1 Finding #1)

Draft 1 named "semver discipline for the public Vault library API" but left other public contracts under-specified. Draft 2 adds a *Contract Inventory* section enumerating six public contracts with explicit owner, versioning policy, and migration policy:

- `anvil-core` Vault library public API
- Audit-store record schemas
- `ARTIFACT_SPECIFICATIONS.md` templates
- Sidecar wire protocol (`anvil.v1.*` protobuf)
- Structured CLI output (`--format json`)
- Stable machine-readable error codes

Each contract's major bumps require Charter amendment. The inventory also closes a downstream gap (P1 #5): the artifact-spec major-change-requires-Charter-amendment rule is now visible in the inventory rather than only in the spec's own versioning policy.

### 2. Public vs Private Audit Records — the safety-critical fix (P1 Finding #2)

The most consequential R1 finding. Draft 1 stated "audit-store records become public artifacts" without distinguishing what's safe to publish from what isn't. Reviewer correctly noted that existing records may contain prompts, raw model outputs, project context, credentials references, vulnerability details, or third-party code excerpts.

Draft 2 adds a constitutional distinction:

- **Local-private audit records** — default class, never automatically published, may contain anything the workflow produces.
- **Public-project audit records** — explicit export via documented public-safe bundle process: default-deny, per-record opt-in, automated redactor pass (secret-scanning patterns + long-quote detection + explicit redact markers), Coordinator review gate (signed approval), cryptographic seal.

Two new audit-store record types added: `PublicVisibilityPolicy` (per-record-type defaults and per-record overrides) and `PublicExportApproval` (Coordinator's signed approval of an export bundle). Total v1 audit-store record types now **15** (11 Charter-required + 4 Plan extensions: `ArbiterFindingResolution`, `SidecarReload`, `PublicVisibilityPolicy`, `PublicExportApproval`).

A new `anvil audit export --public` command is committed to `anvil-audit` (P2 deliverable in Plan Draft 7).

The audit-store-required-types hinge remains a subset check (per R4); the four extensions are Plan-level growth permitted by the Charter's "minimum set; Plan may extend" wording.

### 3. Security Posture elevated to constitutional (P1 Finding #3)

Draft 1's `SECURITY.md` + 90-day window were thin. Draft 2 elevates security posture to Required Project-Level Choices:

- **Threat-model summary** in `SECURITY.md` naming what Anvil defends against (accidental exposure, supply-chain vulnerabilities, local audit-store tampering) and what it does not (adversarial tamper-proofing, nation-state actors, secure multi-party hosting).
- **Vulnerability triage roles** (who reads reports, who acknowledges, who decides on disclosure timing).
- **Supported-version policy** (current major + previous major; older = end-of-life).
- **Coordinated disclosure workflow** documented.
- **Dependency review and SBOM** — CycloneDX SBOM per release; `cargo audit` + `govulncheck` at every CI run.
- **Release signing** — GPG-signed `SHA256SUMS.txt.asc` per platform; release artifacts signed atomically with SBOM.
- **Secret scanning** — pre-commit hook + CI step with `gitleaks` or vetted equivalent; CI fails on detection.

### 4. Governance Mechanics added (P1 Finding #4)

Draft 1 named "BDFL" but didn't define mechanics, creating bus-factor and legitimacy risk. Draft 2 specifies in `GOVERNANCE.md`:

- BDFL role and authority (Charter amendments, ship decisions, dispute resolution).
- Maintainer admission process (BDFL invitation after demonstrated sustained contribution; minimum three shipped phase contributions or equivalent reviewer-quality findings).
- Maintainer authority scope (merge PRs in declared focus areas; cannot amend Charter or override BDFL).
- Maintainer removal (own request / BDFL decision with public rationale / sustained inactivity ≥6 months).
- Conflict of interest disclosure.
- Decision records as audit-store `decision-record` entries.
- BDFL unavailability: acting-BDFL elected by maintainer majority after 30 days of unresponsiveness; acting-BDFL authority limited (no Charter amendments, no convergence declarations); transitions to maintainer-quorum mode after 90 days.
- Designated-successor option (named in `GOVERNANCE.md`).

### 5. Artifact-spec amendment path tightened (P1 Finding #5)

Draft 1's "narrower review process" could have allowed major spec changes to bypass constitutional review. Draft 2 tightens: **major spec changes require full Charter amendment**; only minor/patch changes use the narrower review. This change is reflected in both the Item 2 invariant text and the Contract Inventory.

### 6. Per-Item Disposition Mechanism added (P2 Finding #1)

R1 correctly noted that single-document composition forces all-or-nothing convergence even though reviewers might accept some items and reject others. Draft 2 adds a *Per-Item Disposition Mechanism* section: reviewers structure findings per-item where natural; the Coordinator's disposition lists per-item convergence (converged / pending / refuted); refused items are removed and remaining items proceed; a refused item may be resubmitted as a separate future amendment.

### 7. Embedding Invariants added (P2 Finding #2)

Draft 1's embeddability section named the future v1.2 surface without explicit invariants. Draft 2 adds six non-negotiable embedding invariants:

- Vault remains trust authority — embedders cannot bypass.
- No gate bypass — embedders may surface gates in their own UX but cannot automate approval.
- Typed API only — no screen-scraping; v1 structured CLI output is the v1 embedded surface.
- Audit records mandatory — every embedder operation produces records identical to CLI operations; `embedder_identity` field identifies the caller.
- Per-embedder authentication (v1.2 commitment).
- Multi-tenant isolation (v1.2 commitment).

### 8. Structured CLI Output Stability added (P2 Finding #3)

Draft 1 named `--format json` without stability rules. Draft 2 specifies:

- Per-command JSON Schema files in `schemas/cli/`.
- Top-level `schema_version` field in every output.
- Stable machine-readable error codes enumerated in `schemas/cli/errors.json`.
- `--output-schema-version` flag for pinning during major bumps.
- Compatibility test suite in CI that runs against prior major version's schemas.

### 9. Trademark Posture flagged as pending Coordinator decision (P2 Finding #4)

R1 raised the strategic concern that Apache 2.0 + no trademark may underprotect brand clarity for a services-led customer-acquisition vehicle. Draft 2 retains both postures (A: no registration; B: defensive registration with nominative-use carve-outs) as options and flags the Choice as **pending Coordinator decision** in the next-round disposition. The Coordinator's selection is logged in the disposition that closes R2 (or the convergence declaration).

### 10. DCO Extension — AI assistance and provenance (P2 Finding #5)

Draft 1's DCO was standard. Draft 2 extends with:

- `AI-Assisted-By:` trailer for substantive AI-authored contributions.
- `Derived-From:` trailer for third-party snippets with their license.
- Explicit acknowledgment that contributors confirm right-to-contribute including AI-generated portions.
- CI's DCO check extended to look for trailers on PRs touching substantive code; missing trailers warn on first pass, then block.

### 11. Repo-Readiness Acceptance Gates added (P3 Finding #1)

Draft 1's acceptance criteria focused on review convergence. Draft 2 adds twelve concrete repo-readiness gates the publication milestone cannot fire without: LICENSE, NOTICE, README.md, CONTRIBUTING.md, CODE_OF_CONDUCT.md, SECURITY.md, GOVERNANCE.md, TRADEMARK.md (conditional on Posture B), CI configuration with DCO + secret-scan + vulnerability-scan + spec-validator + structured-CLI-compatibility checks, release-signing workflow, SBOM generation step, public-safe audit bundle self-validation.

### 12. Publication Milestone defined (P3 Finding #2)

Draft 1's "public from initial release" was ambiguous. Draft 2 specifies: repo created private at P0, flips public at v1 ship, pre-publication git history preserved verbatim, audit records exported via public-safe bundle.

### 13. Required Choices count corrected (P3 Finding #3)

R1 noted the count said "eight" while the tables listed nine. Draft 2's tables now reflect actual content (open-source license, contribution mechanism, governance model, Code of Conduct, repository host, security disclosure policy, trademark posture, publication milestone, dependency review and SBOM, release signing, secret scanning — eleven open-source-related Required Choices in Amendment 1 alone, plus artifact specifications in Amendment 2, plus embeddable scope in Amendment 3). The count is no longer hard-coded in prose; the table is canonical.

### 14. Plan Draft 7 Impact Matrix requirement added (P3 Finding #4)

R1 noted Draft 1 understated Plan impact. Draft 2 adds a *Plan Draft 7 Impact Matrix* section that locks the requirement: after amendment convergence, the Plan requires a Draft 7 revision with an explicit impact matrix showing which phases gain work (P0, P2, P4, P7, P10a, P11 are anticipated). The matrix itself is a Plan-stage deliverable, not part of this amendment.

### 15. AiMe reframed as first validation case (P3 Finding #5)

R1 noted that Draft 1 over-anchored on AiMe as the embedding motivation, risking constitutional over-fitting. Draft 2 reframes: AiMe is "the first anticipated validation case" — not the design target. The constitutional embedding commitment must work for embedders that do not yet exist.

### What R1 did *not* change

- The three core commitments (open source under Apache 2.0; defined artifact structures; embeddable workflow infrastructure) remain in place.
- The choice of Apache 2.0 as license is unchanged (R1 reviewer's question about AGPL/BSL is not reopened; Coordinator + R1 reviewer agreement remains).
- The choice of DCO over CLA is unchanged (only extended with the AI/provenance trailers).
- The choice of Contributor Covenant 2.1 is unchanged.
- The choice of GitHub primary hosting is unchanged.

### R1 reviewer

Reviewer from the configured pool; first amendment round. Different model family from Coder per Adversarial Diversity floor.

### Disposition document

`REVIEW_CHARTER_AMENDMENT_A1_R1.md` (R1 round).

---

## Coordinator Decisions (post-R1)

### Trademark Posture: Posture A locked (2026-05-19)

R1 P2 #4 raised the trademark question as a strategic concern. The R1 disposition document recorded the finding as "pending Coordinator decision" rather than auto-resolving. On 2026-05-19, the Coordinator selected **Posture A — no registration** with the following rationale:

- Maximum fork freedom suits an early-stage open-source project where adoption patterns are unproven.
- Defensive registration carries cost (registration fees + occasional enforcement effort) that is not yet justified by demonstrated brand-confusion risk.
- Posture B (defensive registration with nominative-use carve-outs) remains an option for v1.x if adoption patterns produce confusing forks during the v1 ramp.

A re-evaluation trigger is documented in the amendment: if a confusing fork appears during v1 ramp that materially harms users or the services-acquisition role, the Coordinator re-evaluates Posture B as a Charter amendment. The trigger fires on first confusing-fork sighting (not on accumulated harm) because defensive registration is harder to obtain retroactively due to prior-art arguments.

This decision closes R1 P2 #4 as "Fixed (Posture A locked)." The amendment's Required Project-Level Choices table is updated; the *Trademark Posture* section in the amendment body is rewritten to reflect the locked choice with rationale.

No re-review of R1 P2 #4 is required; the Coordinator's selection is final unless the re-evaluation trigger fires.

---

## Hardening Notes (R2 — Consolidated)

R2 came from the configured reviewer pool. Twelve findings raised (4 Fix First, 4 Important, 4 Medium). All twelve addressed in Draft 3. The reviewer's central guidance was constructive: **"produce Draft 3 focused on consistency cleanup"** — don't reopen the architectural surface, just close internal inconsistencies. Draft 3 follows that scope discipline.

The four Fix First items closed the most consequential gaps:

### 1. Trademark consistency restored (Fix First #1)

R1's Coordinator-decision Posture A lock was reflected in the Required Choices table and the Trademark Posture section, but the Reviewer Questions section and Acceptance Criteria still said the decision was "pending." Draft 3 updates both to reference the locked state and the re-evaluation trigger.

### 2. Artifact-spec governance aligned (Fix First #2)

Draft 2's amendment said "major artifact-spec changes require full Charter amendment," but `ARTIFACT_SPECIFICATIONS.md`'s own versioning policy still said major changes require only "Charter-amendment notification." Cross-document contradiction. Draft 3 updates `ARTIFACT_SPECIFICATIONS.md` to match: major spec changes require full Charter amendment; minor and patch use the narrower spec-amendment review process. The Selection Rule states explicitly that ambiguity defaults to MAJOR.

### 3. Publication-safe git history gate added (Fix First #3)

Draft 2 said "pre-publication git history is preserved verbatim." This conflicted with the default-deny audit-export model: commit messages, diffs, accidentally committed secrets, and generated artifacts existed outside the protected export path. Draft 3 adds a constitutional **publication-safe git history gate** that runs before the repository flips public: full-history secret scan (blocks on positive hits), full-history license scan (blocks on incompatible licenses), commit-message sensitivity review by Coordinator, and an exceptional history-rewrite allowance for cases where remediation is impractical. The gate is on the critical path for v1 ship.

### 4. Audit-record private-by-default (Fix First #4)

Draft 2 said `ConvergenceDeclaration` records "default to public-safe." R2 correctly noted that convergence reasoning *can* contain sensitive vulnerability or private project context — a record-type default policy makes accidental publication too easy. Draft 3 makes **all records private by default**. Record types carry only a non-binding `recommended_visibility` metadata field as guidance for Coordinator curation; no record is publicly exported without explicit per-record approval through the gated flow (secret scan, license scan, sensitivity labels, Coordinator manual review). The brittle "quoted strings ≥40 chars" redaction heuristic is replaced (this also closes Important #6) by structured sensitivity labels + multiple specialized scanners + mandatory human approval.

### Important fixes

- **Plan Draft 7 record-type count reconciliation (Important #5).** The amendment now explicitly states Plan Draft 7 must update the record-type total from 13 to 16 (after Draft 3's `EmergencyFreezeDeclaration` addition for Medium #9). The constitutional hinge `test_audit_store_required_types_present` remains a subset check, unchanged.
- **Redaction heuristics restructured (Important #6).** Closed alongside Fix First #4 above. Multi-pass sensitivity scan replaces the brittle quoted-strings heuristic.
- **Cross-document conflict resolution defined (Important #7).** New *Cross-Document Convergence* subsection in *Per-Item Disposition Mechanism*: same reviewer pool reviews both documents; cross-document findings are first-class; convergence on Item 2 requires both documents clean; conflict-resolution rule (constitutional invariants → amendment; operational details → spec).
- **Constitutional outcomes vs implementation tools (Important #8).** Tool-specific names (CycloneDX, cargo audit, govulncheck, gitleaks, GPG) removed from constitutional Required Choices. Choices now lock the *outcome* (SBOM generated; dependency scanning at every CI run; secrets blocked pre-commit + CI; offline-verifiable signing) with the specific tools named as Plan-stage defaults that the Plan may revise without amendment.

### Medium fixes

- **BDFL-adversarial emergency mechanism added (Medium #9).** New emergency-freeze provision in Governance Mechanics: two-thirds majority of active maintainers may declare a freeze halting all merges/releases/governance actions if BDFL credentials are believed compromised or hostile actions are observed. Freeze is intentionally narrow (pauses activity; does not transfer authority). New audit-record type `EmergencyFreezeDeclaration` (16th record type).
- **DCO extension enforceability defined (Medium #10).** "Substantive code paths" defined operationally (≥20 lines new code, or new file, or code inside listed core crates; doc/test/build PRs excluded). "Persistently" defined as: warning on first submission, blocking on subsequent updates without trailers.
- **Schema discovery promoted to requirement (Medium #11).** `--describe-schema` is now a mandatory CLI flag on every command emitting structured output. Schemas are embedded into the binary; subprocess embedders can discover schemas without filesystem access.
- **v1.2 transport form provisional (Medium #12).** Draft 2 locked "network service + typed client libraries + gate-delegation protocol." Draft 3 locks only the *principles* (Vault authority, no gate bypass, typed API only, audit records mandatory, per-embedder auth, multi-tenant isolation). The specific transport form is a v1.2 design choice with its own Charter amendment cycle.

### What R2 did *not* change

- The three core commitments (open source under Apache 2.0; defined artifact structures; embeddable workflow infrastructure) remain in place.
- The Trademark Posture A lock from the Coordinator's R1 decision is unchanged.
- The per-item disposition mechanism is unchanged in spirit; only extended with cross-document convergence rules.
- The Repo-Readiness Acceptance Gates are unchanged in count (12) and content; only the tool-specific names within them are now framed as Plan-stage defaults rather than Charter-locked.

### R2 reviewer

Reviewer from the configured pool; second amendment round. Different model family from Coder per Adversarial Diversity floor.

### Disposition document

`REVIEW_CHARTER_AMENDMENT_A1_R2.md` (R2 round).
