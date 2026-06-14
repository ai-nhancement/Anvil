# Charter Amendment A1 — R1 Disposition

**Date:** 2026-05-19
**Scope:** Response to R1 findings on Charter Amendment A1 (Open-Source Distribution + Defined Artifact Structures + Embeddable Workflow Infrastructure). R1 raised 15 findings (5 P1, 5 P2, 5 P3). Fourteen Fixed in Draft 2; one (P2 #4 Trademark Posture) intentionally left as **Coordinator decision** pending input.
**Spec:** `CHARTER_AMENDMENT_A1.md` (updated to Draft 2 this round; hardening notes appended to `AMENDMENT_A1_HARDENING_HISTORY.md`).
**Prior round:** none — this is the first amendment review round.
**R1 reviewer:** first reviewer family per Charter rotation; different model family from Coder per Adversarial Diversity floor.

**Shell assumption for verification commands:** POSIX shell utilities (`grep`, `awk`).

---

## What changed since Draft 1

R1's central thesis was that Draft 1 named the *direction* of three constitutional commitments correctly but underspecified the *operational guardrails* in five constitutionally-significant places. The Draft 2 changes close each.

The highest-impact resolutions:

- **Public vs Private Audit Records** (P1 #2 — the most consequential R1 finding). Draft 1 said "audit-store records become public artifacts." This was unsafe as written — existing records may contain prompts, raw model outputs, credentials references, vulnerability details. Draft 2 adds a constitutional distinction: local-private records (default; never published) vs. public-project records (explicit export via `anvil audit export --public`, default-deny per record, automated redactor pass, Coordinator review gate with signed approval, cryptographic seal). Two new audit-store record types (`PublicVisibilityPolicy`, `PublicExportApproval`); total v1 record types now 15.
- **Contract Inventory** (P1 #1). Six public contracts enumerated with explicit owner, versioning policy, migration policy: Vault library API, audit-store record schemas, artifact specification document, sidecar wire protocol, structured CLI output, error codes. Each contract's major bumps require Charter amendment.
- **Security Posture as constitutional** (P1 #3). Threat-model summary, vulnerability triage roles, supported-version policy, coordinated disclosure workflow, dependency review with SBOM (CycloneDX), release signing (GPG `SHA256SUMS.txt.asc`), secret scanning (gitleaks or equivalent in pre-commit + CI) — all elevated from Plan follow-ups to Required Choices.
- **Governance Mechanics** (P1 #4). `GOVERNANCE.md` content specified: BDFL role and authority; maintainer admission/removal; maintainer authority scope; conflict-of-interest disclosure; decision-record audit-store entries; BDFL-unavailability fallback (acting-BDFL elected by maintainer majority after 30 days; transitions to maintainer-quorum after 90 days; designated-successor option).
- **Artifact-spec amendment path tightened** (P1 #5). Major spec changes now require full Charter amendment, not narrow review. Visible in both Item 2 invariant text and the Contract Inventory.

Substantial P2 additions:

- **Per-Item Disposition Mechanism** — reviewers may converge on items independently; refused items removed, remaining proceed.
- **Embedding Invariants** — six non-negotiable invariants for the v1.2 surface (Vault authority, no gate bypass, typed API only, audit records mandatory, per-embedder auth, multi-tenant isolation).
- **Structured CLI Output Stability** — per-command JSON Schema files, `schema_version` in every output, stable machine-readable error codes, compatibility test suite.
- **DCO Extension** — `AI-Assisted-By:` and `Derived-From:` trailers for AI assistance and third-party-snippet provenance.

P3 documentary additions: **Publication Milestone** defined (repo private during P0–P11; flips public at v1 ship; pre-publication git history preserved; audit records via public-safe bundle); **Repo-Readiness Acceptance Gates** (12 concrete items); **Plan Draft 7 Impact Matrix requirement**; corrected Required Choices count; **AiMe reframed** as first validation case rather than design target.

One R1 finding (P2 #4, Trademark Posture) is intentionally left **pending Coordinator decision** rather than auto-resolved. See *Coordinator Decision Required* below.

---

## Verification of R1 finding citations

All R1 line citations verified against Draft 1 of the amendment. Each premise grounded.

| Finding | Citation | Verified? | Premise |
|---|---|---|---|
| P1 #1 Contracts not bounded | Implicit across L43 (semver discipline for Vault API only) | ✓ | Other public contracts named but versioning not specified |
| P1 #2 Public audit visibility unsafe | L29 ("records become public artifacts in a public repo") | ✓ | No public/private distinction; raw records could contain sensitive content |
| P1 #3 Security posture too thin | L56 (SECURITY.md + 90-day window only) | ✓ | No threat model, SBOM, signing, secret scanning, triage roles, supported-version policy |
| P1 #4 Governance/BDFL bus-factor | L43 (BDFL named without mechanics) | ✓ | No maintainer admission/removal, succession, conflict-of-interest, decision records |
| P1 #5 Spec amendment may bypass | L73 ("narrower review process") | ✓ | All spec changes including major could bypass Charter-level review |
| P2 #1 Three items as one | L19 (interdependence argument) | ✓ | Reviewers locked to all-or-nothing convergence |
| P2 #2 Embeddability underspecified | L102 (v1.2 surface named, invariants absent) | ✓ | No trust-boundary, auth, isolation, or gate-bypass rules |
| P2 #3 CLI output stability | L102 (`--format json` named without rules) | ✓ | No schema/version/error-code policy |
| P2 #4 License/trademark | L43 (no registration locked) | ✓ | May underprotect brand for services-led acquisition role |
| P2 #5 DCO IP hygiene | L52 (DCO chosen) | ✓ | No AI-assistance or third-party-snippet provenance |
| P3 #1 Repo-readiness gates | L151 (acceptance focused on review convergence) | ✓ | No concrete LICENSE/NOTICE/CONTRIBUTING/CI gates |
| P3 #2 Publication ambiguous | L43 ("public from initial release") | ✓ | Could mean v1.0 release, before v1.0, or immediately |
| P3 #3 Choices count | L177 (says eight; tables show nine) | ✓ | Counting drift |
| P3 #4 Plan impact understated | L133 (phase structure "largely preserved") | ✓ | Several phases gain real work; matrix needed |
| P3 #5 AiMe overfit | L30, L116 (AiMe as primary motivation) | ✓ | Risk of over-anchoring constitutional framing |

Result: 15/15 grounded.

---

## Disposition of R1 findings

| # | Severity | Finding (one-line) | Disposition |
|---|---|---|---|
| 1 | P1 | Versioned contracts not operationally bounded | **Fixed.** New *Contract Inventory* section enumerates six public contracts (Vault API, audit-store schemas, artifact specs, sidecar protocol, structured CLI output, error codes) with owner/versioning/migration policy. Major bumps require Charter amendment. |
| 2 | P1 | Public audit-store visibility unsafe | **Fixed.** New *Public vs Private Audit Records* section adds constitutional distinction between local-private and public-project records. New `anvil audit export --public` mechanism with default-deny, per-record opt-in, automated redactor, Coordinator review gate, cryptographic seal. Two new audit-store record types (`PublicVisibilityPolicy`, `PublicExportApproval`) added; total v1 record types now 15. |
| 3 | P1 | Security posture too thin | **Fixed.** Security posture elevated to constitutional via Required Project-Level Choices: threat-model summary, vulnerability triage roles, supported-version policy, coordinated disclosure workflow, dependency review and SBOM (CycloneDX), release signing (GPG), secret scanning. `SECURITY.md` content specified. |
| 4 | P1 | Governance/BDFL bus-factor | **Fixed.** New *Governance Mechanics* section specifies `GOVERNANCE.md` content: BDFL authority; maintainer admission/removal; conflict-of-interest; decision records; BDFL-unavailable fallback (acting-BDFL after 30 days; maintainer-quorum after 90 days; designated-successor option). |
| 5 | P1 | Spec amendment path may bypass constitutional review | **Fixed.** Item 2 invariant tightened: **major spec changes require full Charter amendment**, not narrow review. Minor/patch changes follow the narrower process documented in the spec. Reflected in Contract Inventory. |
| 6 | P2 | Three items as one amendment | **Fixed.** New *Per-Item Disposition Mechanism* section: reviewers structure findings per-item; per-item convergence allowed; refused items removed and remaining proceed. |
| 7 | P2 | Embeddability underspecified | **Fixed.** New *Embedding Invariants* section: six non-negotiable invariants (Vault authority, no gate bypass, typed API only, audit records mandatory, per-embedder auth in v1.2, multi-tenant isolation in v1.2). |
| 8 | P2 | Structured CLI output needs stability rules | **Fixed.** New *Structured CLI Output Stability* section: per-command JSON Schemas in `schemas/cli/`, `schema_version` in every output, stable error codes in `schemas/cli/errors.json`, `--output-schema-version` flag, compatibility test suite. |
| 9 | P2 | License/trademark posture under-protected | **Pending Coordinator decision.** Draft 2 retains both Posture A (no registration; Draft 1 default) and Posture B (defensive registration with nominative-use carve-outs in `TRADEMARK.md`) as options. The Choice is flagged as pending; the Coordinator selects in the next-round disposition. See *Coordinator Decision Required* below. |
| 10 | P2 | DCO IP hygiene | **Fixed.** New *DCO Extension* section: `AI-Assisted-By:` trailer for substantive AI assistance; `Derived-From:` trailer for third-party snippets with license; right-to-contribute explicit for generated content; CI DCO check extended to look for trailers. |
| 11 | P3 | Acceptance criteria lack repo-readiness gates | **Fixed.** New *Repo-Readiness Acceptance Gates* section adds 12 concrete items (LICENSE, NOTICE, README, CONTRIBUTING, CODE_OF_CONDUCT, SECURITY, GOVERNANCE, optional TRADEMARK, CI config with DCO/secret-scan/vuln-scan/spec-validators/CLI-compat, release-signing workflow, SBOM step, public-safe audit bundle self-validation). |
| 12 | P3 | "Public from initial release" ambiguous | **Fixed.** New *Publication Milestone* section: repo private during P0–P11, flips public at v1 ship, pre-publication git history preserved verbatim, audit records via public-safe bundle. |
| 13 | P3 | Required Choices count inconsistent | **Fixed.** Count no longer hard-coded in prose; tables are canonical. Open-source-related Required Choices now total eleven (open-source license, contribution mechanism, governance model, Code of Conduct, repository host, security disclosure policy, trademark posture, publication milestone, dependency review and SBOM, release signing, secret scanning) plus one in Amendment 2 (artifact specifications) plus one in Amendment 3 (embeddable scope). |
| 14 | P3 | Plan impact understated | **Fixed.** New *Plan Draft 7 Impact Matrix* section locks the requirement: after amendment convergence, Plan revision to Draft 7 includes an explicit impact matrix per phase. Anticipated phases impacted: P0, P2, P4, P7, P10a, P11. Matrix itself is a Plan-stage deliverable, not part of this amendment. |
| 15 | P3 | AiMe over-anchored | **Fixed.** AiMe reframed as "the first anticipated validation case" — not the design target. Constitutional embedding commitment must work for embedders that do not yet exist. |

---

## Coordinator Decision Required: Trademark Posture (P2 #4)

R1 P2 #4 raised a real strategic concern: Apache 2.0 + no trademark maximizes fork freedom but may underprotect brand clarity for Anvil's services-led customer-acquisition role. A third party could ship an inferior fork under the Anvil name and confuse prospective customers.

Two viable postures, both compatible with open-source distribution:

- **Posture A** (Draft 1 default): No trademark registration. README naming-preference statement only. Lowest cost; maximum fork freedom; weakest brand defense.
- **Posture B** (R1-suggested): Defensive trademark registration in primary jurisdiction. Apache 2.0 + nominative-use carve-outs in `TRADEMARK.md` permit forks to reference the Anvil name descriptively ("compatible with Anvil," "fork of Anvil") while reserving the right to require name changes for products marketed *as* Anvil-the-product. Moderate cost (registration fees + occasional enforcement); reasonable brand defense.

The decision is intentionally pending in Draft 2. The Coordinator's selection is locked in the disposition that closes R2 (or the convergence declaration, if the Coordinator declares partial-convergence on Items 1+2 and continues iterating on Item 1's Trademark sub-decision).

**Reviewer R1 input on enforcement-cost feasibility of Posture B is welcome but not required**; the decision is the Coordinator's.

---

## Files changed since Draft 1

| File | Action | Purpose | Approximate delta |
|---|---|---|---|
| `CHARTER_AMENDMENT_A1.md` | MODIFY (substantial rewrite) | Apply 14 Fixed dispositions + 1 pending Coordinator decision. Add: Publication Milestone, Governance Mechanics, Security Posture as Constitutional, DCO Extension, Public vs Private Audit Records, Contract Inventory, Embedding Invariants, Structured CLI Output Stability, Per-Item Disposition Mechanism, Plan Draft 7 Impact Matrix, Repo-Readiness Acceptance Gates. Reframe AiMe. Tighten Item 2 (spec amendment path). Status header bumped to Draft 2. | +~400 lines net |
| `AMENDMENT_A1_HARDENING_HISTORY.md` | CREATE | New file for amendment-specific hardening notes; appends `Hardening Notes (R1 — Consolidated)` covering 15 findings. | ~145 lines |
| `REVIEW_CHARTER_AMENDMENT_A1_R1.md` | CREATE | This document. | ~250 lines |
| `ARTIFACT_SPECIFICATIONS.md` | (UNTOUCHED in this round) | Concurrent review workstream; not affected by Amendment R1. Spec's own R1 review proceeds independently. |

---

## Corrections to prior dispositions

None this round. This is the first amendment review.

---

## Residual / deferred

- **Trademark Posture decision** (P2 #4). Pending Coordinator.
- **Plan Draft 7 impact matrix** — required by this amendment's *Plan Draft 7 Impact Matrix* section but itself a Plan-stage workstream after amendment convergence.
- **`GOVERNANCE.md` / `SECURITY.md` / `CONTRIBUTING.md` / `TRADEMARK.md` actual content** — these documents ship as P0 deliverables in Plan Draft 7; this amendment locks the *requirements* and *content scope* for each, not the drafted content.
- **`schemas/cli/` per-command schemas** — required by *Structured CLI Output Stability* but content is per-command, lands in Plan phases P5–P10a as commands are implemented.
- **Per-embedder auth + multi-tenant isolation** — Embedding Invariants list these as v1.2 commitments; v1 implementation is out of scope.
- **`anvil audit export --public` implementation** — committed in this amendment; lands in Plan P2's audit-store work (Plan Draft 7 will reflect).

---

## Reproducibility

```bash
# --- R1 #1 — Contract Inventory section present ---
grep -n "^## Contract Inventory" CHARTER_AMENDMENT_A1.md
# Expected: 1 match.

awk '/^## Contract Inventory/,/^---$/' CHARTER_AMENDMENT_A1.md | grep -cE "Vault library public API|Audit-store record schemas|ARTIFACT_SPECIFICATIONS.md|Sidecar wire protocol|Structured CLI output|machine-readable error codes"
# Expected: 6.

# --- R1 #2 — Public vs Private Audit Records section ---
grep -n "^## Public vs Private Audit Records" CHARTER_AMENDMENT_A1.md
# Expected: 1 match.

awk '/^## Public vs Private Audit Records/,/^---$/' CHARTER_AMENDMENT_A1.md | grep -E "Local-private audit records|Public-project audit records|anvil audit export --public|PublicVisibilityPolicy|PublicExportApproval"
# Expected: ≥5 matches.

# --- R1 #3 — Security posture as constitutional ---
awk '/^### Security Posture/,/^### /' CHARTER_AMENDMENT_A1.md | grep -E "Threat-model|Vulnerability triage|Supported-version policy|Coordinated disclosure"
# Expected: ≥4 matches.

# --- R1 #4 — Governance Mechanics section ---
grep -n "^### Governance Mechanics" CHARTER_AMENDMENT_A1.md
# Expected: 1 match.

awk '/^### Governance Mechanics/,/^### /' CHARTER_AMENDMENT_A1.md | grep -E "Maintainer admission|Maintainer removal|Conflict of interest|BDFL succession|acting-BDFL"
# Expected: ≥5 matches.

# --- R1 #5 — Spec amendment requires full Charter amendment for major ---
grep -E "major spec changes require full Charter amendment|Major changes.*require full Charter amendment" CHARTER_AMENDMENT_A1.md
# Expected: ≥2 matches.

# --- R1 #6 — Per-Item Disposition Mechanism ---
grep -n "^## Per-Item Disposition Mechanism" CHARTER_AMENDMENT_A1.md
# Expected: 1 match.

# --- R1 #7 — Embedding Invariants ---
grep -n "^### Embedding Invariants" CHARTER_AMENDMENT_A1.md
# Expected: 1 match.

awk '/^### Embedding Invariants/,/^### /' CHARTER_AMENDMENT_A1.md | grep -E "Vault remains the trust authority|No gate bypass|Typed API only|Audit records mandatory|Per-embedder authentication|Multi-tenant isolation"
# Expected: 6 matches.

# --- R1 #8 — Structured CLI Output Stability ---
grep -n "^### Structured CLI Output Stability" CHARTER_AMENDMENT_A1.md
# Expected: 1 match.

# --- R1 #10 — DCO Extension ---
grep -n "^### DCO Extension" CHARTER_AMENDMENT_A1.md
# Expected: 1 match.

awk '/^### DCO Extension/,/^### /' CHARTER_AMENDMENT_A1.md | grep -E "AI-Assisted-By|Derived-From"
# Expected: ≥2 matches.

# --- R1 #11 — Repo-Readiness Acceptance Gates ---
grep -n "^## Repo-Readiness Acceptance Gates" CHARTER_AMENDMENT_A1.md
# Expected: 1 match.

# --- R1 #12 — Publication Milestone ---
grep -n "^### Publication Milestone" CHARTER_AMENDMENT_A1.md
# Expected: 1 match.

# --- R1 #14 — Plan Draft 7 Impact Matrix ---
grep -n "^## Plan Draft 7 Impact Matrix" CHARTER_AMENDMENT_A1.md
# Expected: 1 match.

# --- R1 hardening notes appended ---
grep -n "^## Hardening Notes (R1 — Consolidated)" AMENDMENT_A1_HARDENING_HISTORY.md
# Expected: 1 match.
```

---

## Bottom line

R1 was a high-impact review round. The reviewer correctly identified that Draft 1's *constitutional commitments* were directionally right but their *operational guardrails* were underspecified in five constitutionally-significant places. Draft 2 closes each gap with substantial new content.

The single most important fix was the Public vs Private Audit Records distinction (P1 #2): writing "records become public artifacts" without a default-deny export mechanism would have created a real risk of accidentally publishing prompts, model outputs, credentials, or vulnerability discussion. The fix is constitutional, includes a concrete mechanism (`anvil audit export --public` with Coordinator review gate and cryptographic seal), and adds two audit-store record types.

The Contract Inventory (P1 #1) is the second most important: making versioning explicit and per-contract is what makes open-source distribution actually safe for downstream consumers.

One R1 finding is intentionally pending Coordinator decision (Trademark Posture, P2 #4) rather than auto-resolved. The strategic question is the Coordinator's to make.

**Next step:** Coordinator's decision on Trademark Posture, then R2 amendment review (or convergence call). The Plan Draft 7 impact-matrix workstream is a follow-on after amendment convergence.
