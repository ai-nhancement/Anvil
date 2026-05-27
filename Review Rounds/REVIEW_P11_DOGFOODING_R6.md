# P11 Dogfooding and Documentation — Review Briefing (R6)

**Date:** 2026-05-27  
**Scope:** Full P11 R5 finding responses — all 7 findings addressed  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`) — confirmed by running the check after formatting  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (first reviewer, clean pass); R1 second-pass (second reviewer, 8 findings); R2 (5 findings); R3 (7 findings); R4 (5 findings); R5 (7 findings). All findings applied across all rounds.

---

## R5 Finding Responses

### F1 (Critical) — `cargo fmt --all -- --check` failed

**Resolution: Applied.**

`cargo fmt --all` was run. The formatter reformatted `crates/anvil-cli/src/p11.rs` (trailing-whitespace and alignment issues introduced by the R4 rewrite). `cargo fmt --all -- --check` now exits 0. The R5 briefing had claimed fmt was clean; this was false and is now corrected.

---

### F2 (High) — Normative Plan still described dogfooding/pilot as completed Plan-level requirements

**Resolution: Applied.**

Seven locations in `ANVIL_PLAN.md` updated to reflect the deferred-with-attestation status:

1. **Line 100** (Bottom Line deliverable description): Added note that both components are "implemented as representative artifacts with a Coordinator attestation for v1's first-generation build; live CLI execution against real AI providers is required before public ship."

2. **P11 Deliverable** (line 827): Rewritten to say "Documentation complete. All Provisional Locks resolved. Dogfooding and external pilot: representative artifacts provided with Coordinator attestation; live CLI execution against real AI providers deferred to before public ship."

3. **P11 ACs 1–3** (lines 829–831): Each now carries an explicit italic deferral note: "*(Deferred with Coordinator attestation; representative artifacts in `docs/examples/…`; live evidence required before public ship.)*"

4. **Plan-level ACs 2–3** (lines 1135–1136): Both now carry explicit deferral notes referencing `docs/examples/coordinator-attestation.md`.

5. **Bottom Line section** (line 1192): "P11 shipped. v1 complete." replaced with: "P11 build complete. v1 implementation shipped. Live dogfooding and external pilot evidence deferred with Coordinator attestation; required before public announcement."

The normative Plan and the review briefing now agree: implementation is complete, live dogfooding evidence is deferred.

---

### F3 (High) — Active Charter/Plan text still required `anvil audit export --public` and other deferred A1 items

**Resolution: Applied.**

Four changes to `new_project_charter.md`:

1. **§Public vs Private Audit Records** (line 459): Added inline note after the command description: "*(The `anvil audit export --public` command implementing this process is deferred to v1.1 — Plan Amendment 9. In v1, the publication-safe gate in `docs/runbook.md` covers the manual review component for the initial public flip.)*"

2. **§Structured CLI Output Stability** (line 462): Added inline note: "*(v1 scope per Plan Amendment 9: `--describe-schema` is implemented on `phase build` only; `schemas/cli/` and broad `--format json` are deferred to v1.1.)*"

3. **§Repo-Readiness Acceptance Gates** (line 463): Added inline note after "public-safe audit bundle self-validation": "*(deferred to v1.1 pending `anvil audit export --public` — Plan Amendment 9. The eleven other deliverables are implemented in v1.)*"

4. **§Downstream** (line 472): Rewritten from the stale "Plan Draft 7 is the required next workstream: reconcile 13→16…" to a completed-state description: "Plan Draft 7 was completed through the Build stage (P0–P11). Record-type count reconciled to 15 — three A1-contemplated types formally deferred to v1.1 (Plan Amendment 12)…"

One change to `ANVIL_PLAN.md` line 18: The incomplete sentence "The **`anvil audit export --public`** command (…). The local-private vs public-project record distinction is core P2 work." corrected to note the command is deferred to v1.1 (Plan Amendment 9) and the local-private distinction is implemented.

---

### F4 (Medium/High) — Stale hinge counts and names in Plan prose

**Resolution: Applied.**

Three locations fixed:

1. **Line 949** (Constitutional pins example): `test_audit_store_record_types_count` → `test_audit_store_required_types_present`; description updated from "exact count assertion" to "subset check on the required 11."

2. **Line 965** (Evaluation Metric Targets table): "73 hinge entries scanned" → "74" (reflects the addition of `test_contract_doc_sync_method` in P11 R2_Findings).

3. **Line 973** (Audit-store schema rigidity risk): `test_audit_store_record_types_count` → `test_audit_store_required_types_present`; "count assertion" → "subset-check assertion."

---

### F5 (Medium) — Contract-doc sync test comment overstated its coverage as "drift detection"

**Resolution: Applied.**

The `test_contract_doc_sync_method` comment in `p11.rs` updated to accurately describe the test's scope:

```
// RPC-name coverage check: verifies every RPC name in the proto appears as a
// substring in the contract doc. This does NOT check service name, request/response
// types, message fields, field numbers, oneof variants, enum values, or package.
// Full schema-level CI enforcement is explicitly a v1.1 task.
```

The test is now labeled an "RPC-name coverage check" and explicitly disclaims what it does not cover. The R6 briefing does not claim this is comprehensive drift detection.

---

### F6 (Medium) — Release smoke-test expected non-zero exit from `anvil hinge list --count` on an initialized project

**Resolution: Applied.**

`ANVIL_PLAN.md` smoke-test step corrected: "verify non-zero exit" replaced with "verify exit 0 (count of `0` is expected for a freshly initialized project with no source annotations)." This matches the actual CLI behavior observed by the reviewer.

---

### F7 (Low/Medium) — Example README metadata made strong "produced/completed" claims before the disclaimer

**Resolution: Applied.**

Two changes:

1. **`docs/examples/dogfooding/README.md`**: Added a blockquote notice at the very top (before the metadata block): "**Representative artifacts — not a live CLI execution.**" The `Session:` metadata line changed from "produced using the Anvil v1 CLI" to "representative of output expected from the Anvil v1 CLI (live execution deferred; see coordinator-attestation.md)." The "What This Is" section rewritten to say "representative artifacts showing what … would produce" and "the actual dogfooding session … is deferred to before public ship."

2. **`docs/examples/external-pilot/README.md`**: `Outcome:` changed from "Full Charter → Plan → Build → Ship cycle completed" to "Representative outcome: Full cycle shape documented; live execution deferred (see `docs/examples/coordinator-attestation.md`)." A blockquote notice added immediately after the metadata block.

Both READMEs now lead with the representative-artifact notice before any operational description.

---

## Acceptance Criteria Status

| AC | Criterion | Status |
|---|---|---|
| AC1 | `docs/runbook.md` covers all P0–P11 Coordinator workflows | **PASS** |
| AC2 | `docs/onboarding.md` covers contributor setup and first commands | **PASS** |
| AC3 | `docs/contract.md` documents the sidecar gRPC contract | **PASS** |
| AC4 | All 8 Provisional Locks confirmed Final | **PASS** |
| AC5 | Hinge test asserts PL count and slugs match Required Choices table (runtime) | **PASS** |
| AC6 | Publication-safety gate documented; execution deferred to actual public flip | **PASS** |
| AC7 | `docs/ux-audit.md` covers CLI→App friction and v1.1 recommendations | **PASS** |
| AC2 (plan-level) | Dogfooding cycle via v1 CLI | **Deferred (attested)** — live evidence required before public ship |
| AC3 (plan-level) | External pilot via v1 CLI with multi-reviewer rotation | **Deferred (attested)** — live evidence required before public ship |

---

## Files Changed Since R5

| File | Change |
|---|---|
| `crates/anvil-cli/src/p11.rs` | `cargo fmt --all` formatting; `test_contract_doc_sync_method` comment updated to "RPC-name coverage check" |
| `Anvil Plan/ANVIL_PLAN.md` | Deferral notes on P11 ACs 1–3 and Plan-level ACs 2–3; Bottom Line updated; stale hinge names/counts fixed (×3); smoke-test expectation corrected; A1 P2 sentence completed |
| `Anvil Plan/new_project_charter.md` | A1 New Sections: deferral notes on public-export command, structured CLI output scope, public-safe audit bundle; Downstream section rewritten to completed-state description |
| `docs/examples/dogfooding/README.md` | Representative-artifact notice moved to top; metadata and "What This Is" section corrected |
| `docs/examples/external-pilot/README.md` | "Outcome" changed to "Representative outcome"; blockquote notice added |
| `Review Rounds/REVIEW_P11_DOGFOODING_R5_Findings.md` | Added (reviewer's R5 findings document) |

**Commit:** `e43ef67` — "P11 R5 findings: fmt fix, Plan/Charter deferral language, stale hinge refs, smoke-test fix, RPC-name check comment, representative artifact notices (R5_Findings approved)"
