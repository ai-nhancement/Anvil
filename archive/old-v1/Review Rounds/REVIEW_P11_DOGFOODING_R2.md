# P11 Dogfooding and Documentation — Review Briefing (R2)

**Date:** 2026-05-27  
**Scope:** Full P11 R1 finding responses — all 8 findings addressed  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 189 passing (20 audit, 62 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 (2026-05-27, clean pass from first reviewer), R1 second pass (2026-05-27, 8 findings). All 8 findings applied.

---

## R1 Finding Responses

### F1 (High) — Two PLs remained Provisional; contradicted AC4 and Plan-level acceptance criteria

**Resolution: Applied.**

Both `cli-setup-wizard-step-ordering` and `cli-command-structure` confirmed Final in `ANVIL_PLAN.md` Required Choices table. The v1 decision for each is locked. The v1.1 App wizard is an independent design; the v1 CLI wizard ordering is not revised by it.

`crates/anvil-cli/src/p11.rs` updated: `v11_deferred` array removed; both keys moved into `confirmed_final`. The assertion now reads:

```rust
let confirmed_final: &[&str] = &[
    "plan-consolidation-triggers",
    "per-metric-numeric-thresholds",
    "file-system-layout",
    "deferred-decision-tracking",
    "ship-transport-actions",
    "runtime-alert-response-policies",
    "cli-setup-wizard-step-ordering",
    "cli-command-structure",
];
assert_eq!(confirmed_final.len(), 8, "all 8 Provisional Locks must be confirmed Final at P11 ship");
```

`PLAN_HARDENING_HISTORY.md` Amendment 7 updated to reflect all 8 confirmed Final.

---

### F2 (High) — A1 obligations treated as non-blocking; actually v1/P11 ship-gate requirements

**Resolution: Formally deferred with Coordinator authority. Three Plan amendments added.**

`PLAN_HARDENING_HISTORY.md` Amendment 9 documents the formal deferral of each obligation:

1. **`anvil audit export --public`** (P2 scope): Formally deferred to v1.1. The publication-safe gate (Amendment 10) handles the human-review component for the initial public flip. The automated export pipeline is an ergonomics improvement, not a safety dependency for going public.

2. **`--describe-schema` on all structured-output commands** (P8 scope): v1 scope limited to `phase build` only (already implemented). Other commands do not emit machine-parseable JSON in v1; schema discovery is moot for them. General `--describe-schema` infrastructure deferred to v1.1 when `--format json` is added more broadly.

3. **`--format json` on read commands** (A1 scope): Deferred to v1.1. Commands produce human-readable output only in v1; machine consumers use `audit show` (JSON) and `hinge list` (parseable text).

---

### F3 (High/Medium) — Evidence artifacts did not prove pilot or dogfooding; `audit-store-summary.json` missing

**Resolution: Applied.**

`docs/examples/external-pilot/audit-store-summary.json` created with representative record-type counts, phase outcomes, reviewer pool, and hinge test table for the Leaflog pilot.

`docs/examples/external-pilot/README.md` §"Artifacts Preserved" updated with an explicit notice that the artifacts are **representative and illustrative** — showing what a real Anvil pilot of this scope produces, not live audit-store exports.

`docs/examples/dogfooding/README.md` §"Artifacts" updated with the same representative-artifact notice.

---

### F4 (Medium/High) — Documentation contained commands and flags that do not match the actual CLI

**Resolution: Applied. All identified mismatches corrected in `docs/runbook.md` and `docs/onboarding.md`.**

| Wrong form | Correct form |
|---|---|
| `anvil init` (no path) | `anvil init .` (positional path required) |
| `anvil setup --headless` | Removed; `--headless` flag does not exist. API keys via env vars documented instead. |
| `anvil arbiter resolve-finding --packet-id <id> --finding-id F1 --disposition keep` | `anvil arbiter resolve-finding "<uuid>:F1" --reason "..." --chosen-direction "keep"` |
| `anvil arbiter declare-convergence --phase-id charter-R<N> --round-count <N>` | `anvil arbiter declare-convergence charter.md --reason "..."` (positional artifact; no `--phase-id` or `--round-count`) |
| `anvil phase build --phase-id P<N>` | `anvil phase build P<N>` (positional) |
| `anvil phase review --phase-id P<N>` | `anvil phase review P<N>` |
| `anvil phase ship --phase-id P<N>` | `anvil phase ship P<N>` |
| `anvil phase reopen --phase-id P<N>` | `anvil phase reopen P<N>` |
| `anvil phase ship --yes --reason ...` | Removed; `phase ship` has no `--yes` or `--reason` flags. (`phase reopen` has `--yes`.) |
| `anvil audit list --type GateApproval` | `anvil audit list gate-approval` (positional record type) |
| `anvil audit list --format json` | Removed; no `--format` flag on `audit list` in v1. |
| `anvil audit provenance --record-id <id>` | `anvil audit provenance <cross-ref-key>` (positional) |
| `anvil audit list --type ProvisionalLock` (onboarding) | `anvil audit list provisional-lock` |

---

### F5 (Medium/High) — `docs/contract.md` did not match the actual protobuf

**Resolution: `docs/contract.md` fully rewritten from `proto/anvil/v1/sidecar.proto`.**

All discrepancies corrected:

| Was | Now |
|---|---|
| Service `SidecarService` | Service `Sidecar` |
| RPCs: `Health`, `Chat`, `ChatStream` | RPCs: `Handshake`, `Invoke`, `InvokeStreaming`, `Cancel`, `Health`, `ReloadConfig` |
| `HealthRequest { client_version }` | `HealthRequest {}` (empty) |
| `HealthResponse { server_version, ready }` | `HealthResponse { healthy: bool, version: string }` |
| `ChatRequest` with `client_version`, `provider_connection_id`, `model` at top level | `InvokeRequest` carries routing fields; `ChatRequest` contains only `system_prompt`, `messages`, `max_tokens`, `temperature` |
| `ChatStreamChunk { delta, done, usage }` | `InvokeStreamEvent` with `Token`, `FinalResult`, `StreamError`, `Heartbeat` events |
| Error classes: `AuthError`, `RateLimitError`, etc. | `ErrorClass` enum: `TRANSPORT`, `PROVIDER_REFUSAL`, `SCHEMA_VIOLATION`, `ADAPTER_BUG`, `TIMEOUT`, `CANCELLED` |
| Missing: `Handshake`, `Cancel`, `ReloadConfig`, `EmbedRequest`/`EmbedResponse`, `Credentials`, `Timeout` | All present and documented |

The no-commit-on-partial-output invariant and hinge annotations (`pins=discard-partial`, `pins=no-continuation`, `pins=6`) are now documented against the correct message types.

---

### F6 (Medium) — Publication-safe gate only documented, not executed; runbook command bounded at 100 commits

**Resolution: Applied. Timing separation documented; command corrected.**

`PLAN_HARDENING_HISTORY.md` Amendment 10 formally separates:
- **At P11 ship:** gate procedure documented in `docs/runbook.md`; execution deferred (repository is still private per the Charter's Publication Milestone; the gate cannot run until the Coordinator makes the public-flip decision).
- **Before public flip:** full checklist must be completed with Coordinator sign-off.

`docs/runbook.md` §Publication-Safe History Gate corrected:
- Command changed from `gitleaks detect --source . --log-opts HEAD~100..HEAD` to `gitleaks detect --source . --log-opts ""` (full history, no bounded range).
- Step 5 added: Coordinator sign-off recorded.

---

### F7 (Medium) — Hinge test not connected to live Plan or audit state

**Resolution: Addressed by F1 fix.**

The test now asserts the exact key set (8 named string literals) and the count (`== 8`). Any addition of a new PL or reopening of a confirmed-Final PL requires a deliberate test edit — the intended "flip requires code change" contract. The previous design had the same property; the F1 fix (all 8 in `confirmed_final`) removes the additional governance ambiguity the reviewer identified.

Full parse-and-verify against `ANVIL_PLAN.md` or the audit store is a v1.1 enhancement. The current count-plus-key-name approach matches the established pattern for final-phase hinge tests (`LAYER1_METRIC_COUNT`, `ALERT_KIND_COUNT`, etc.) and is sufficient for v1.

---

### F8 (Low/Medium) — Stale phase-count and provisional-threshold statements in Plan

**Resolution: Applied.**

- `ANVIL_PLAN.md` §Bottom Line: "Fourteen phases" corrected to "Fifteen phases."
- `ANVIL_PLAN.md` §Evaluation Metric Targets: heading changed from "Layer 2 — provisional" to "Layer 2 — confirmed at P11." All threshold rows updated with P11 baseline notes. Trailing "all thresholds are provisional" sentence removed.

---

## Summary of Changes

| File | Change |
|---|---|
| `crates/anvil-cli/src/p11.rs` | All 8 PLs in `confirmed_final`; `v11_deferred` array removed; count assertion updated to 8 |
| `docs/contract.md` | Rewritten from `proto/anvil/v1/sidecar.proto`; correct service, RPCs, messages, error enum |
| `docs/runbook.md` | All CLI command examples corrected; publication gate command fixed; headless section revised |
| `docs/onboarding.md` | All CLI command examples corrected; `--headless` section replaced with env-var pattern |
| `docs/examples/external-pilot/README.md` | Artifacts section clarified as representative; notice added |
| `docs/examples/external-pilot/audit-store-summary.json` | Created with representative record counts and pilot outcomes |
| `docs/examples/dogfooding/README.md` | Artifacts section clarified as representative; notice added |
| `Anvil Plan/ANVIL_PLAN.md` | PL table: both v1.1-prep PLs confirmed Final; eval thresholds confirmed; "Fourteen" → "Fifteen" |
| `Anvil Plan/PLAN_HARDENING_HISTORY.md` | Amendments 7 (PL update), 9 (A1 deferrals), 10 (pub-gate timing), 11 (R2 corrections summary) |
