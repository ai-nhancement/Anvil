# Anvil v1 Runbook — CLI Operational Guide

**Version:** 1.0.1  
**Date:** 2026-05-27  
**Scope:** All six phase-gate operations and project-level commands.

This runbook is for operators (Coordinators) running Anvil v1 on a project. It covers every gate in the workflow from `anvil init` to `anvil ship`. For first-time setup, see `onboarding.md`. For the sidecar contract, see `contract.md`.

---

## Prerequisites

- `anvil` and `anvil-sidecar` binaries on `$PATH` (or `sidecar.binary_path` set in `anvil.toml`).
- API credentials configured via `anvil setup` or `ANVIL_API_KEY_*` environment variables.
- A project directory initialized with `anvil init <path>`.

---

## Project Initialization

```sh
# Create and enter your project directory
mkdir my-project && cd my-project

# Initialize the Anvil project at the current directory (idempotent; safe to re-run)
anvil init .

# Run the interactive Setup Wizard to configure credentials, reviewer pool, and governance
anvil setup .
```

After `anvil setup` completes, `anvil.toml` contains your reviewer pool, provider connections, and project configuration.

---

## Gate 1 — Charter Review: Briefing Sent

The Charter stage pipeline begins with `anvil charter review`.

```sh
# Start a charter review round
anvil charter review --project .
```

The sidecar is spawned automatically on first invocation. The reviewer receives the charter, processes it, and the `ReviewerFindingPacket` audit record is written.

**Audit record created:** `ReviewerFindingPacket`

**What can go wrong:**
- Sidecar not found → check `sidecar.binary_path` in `anvil.toml` or `$PATH`
- API key missing → set `ANVIL_API_KEY_<PROVIDER>` or re-run `anvil setup`
- Charter file missing → ensure your charter markdown is at the configured path

---

## Gate 2 — Charter Review: Findings Received

After the reviewer responds, findings are available via:

```sh
# List all findings from the current review round
anvil charter findings --project .
```

Findings are stored as part of the `ReviewerFindingPacket` audit record. Review them before curation.

**Audit record:** `ReviewerFindingPacket` (already written at Gate 1)

---

## Gate 3 — Charter Review: Findings Curated

The Coordinator reviews each finding and decides: keep, drop, edit, or annotate.

```sh
# Inspect a specific finding packet by record ID
anvil audit show <record-id> --project .

# Curate a finding — composite ID is <packet-uuid>:<finding-id> (e.g. "abc123:F1")
anvil arbiter resolve-finding "<packet-uuid>:F1" \
    --reason "Valid P1 finding — scope too broad" \
    --project .

# Optional: record chosen direction and any contradiction with prior rounds
anvil arbiter resolve-finding "<packet-uuid>:F2" \
    --reason "Refuted: the charter already covers this in §3" \
    --chosen-direction "drop" \
    --project .
```

The composite finding ID `<packet-uuid>:<finding-id>` is assembled from:
- the record UUID returned by `anvil audit list reviewer-finding-packet --project .`
- the finding ID within that packet (e.g. `F1`, `F2`)

Curation produces `ArbiterFindingResolution` records for each finding dispositioned.

**Audit records:** `ArbiterFindingResolution` per finding

---

## Gate 4 — Charter Review: Disposition Rendered

After curation, write the disposition document (a markdown file in `Review Rounds/`) by hand, then verify the Plan contract is satisfied:

```sh
# Verify Required Choices locking state; prints pass/fail — writes no audit record
anvil gate check-plan --project .
```

The gate check confirms that Required Choices are in their expected locked state before advancing. It does not write an audit record; disposition document authoring is a manual step.

**Audit record:** none (manual step; `anvil gate check-plan` is a verification-only command)

---

## Gate 5 — Charter Review: Next-Reviewer-or-Ship Decision

After disposition, either advance to the next reviewer (another rotation slot) or declare convergence:

```sh
# Declare convergence (full-pool clean or human arbiter authority)
# The artifact argument is the artifact name (e.g. "charter.md")
anvil arbiter declare-convergence charter.md \
    --reason "Full pool clean on R3; no remaining P1/P2 findings." \
    --project .
```

**Audit record:** `ConvergenceDeclaration`

---

## Gate 6 — Phase Ship

Once all review gates pass, ship the phase:

```sh
# Ship a build phase (phase ID is a positional argument)
anvil phase ship P5 --project .

# Ship the project (all phases must be in shipped state)
anvil ship --project .
```

**Audit records:** `PhaseDisposition` (shipped), `GateApproval` (phase-ship gate)

---

## Build Stage — Phase Gates

Each build phase (P0, P1, P2, …) goes through its own six-gate loop. The commands are:

```sh
# 1. Build (Coder implements) — phase ID is positional
anvil phase build P<N> --project .

# 2. Review (reviewer returns findings)
anvil phase review P<N> --project .

# 3. Findings (Coordinator inspects)
anvil phase findings P<N> --project .

# 4. (Coder applies fixes)

# 5. Ship the phase when all findings addressed
anvil phase ship P<N> --project .
```

Rollback when a shipped phase must be reopened:

```sh
anvil phase reopen P<N> --reason "Found regression in P7 tests" --project .

# Non-interactive (skips confirmation prompt)
anvil phase reopen P<N> --reason "CI-triggered rollback" --yes --project .
```

---

## Plan Stage

```sh
# Generate the plan via the sidecar
anvil plan invoke --project .

# Review the generated plan
anvil plan review --project .

# Inspect plan findings
anvil plan findings --project .

# Consolidate plan versions (creates PlanConsolidationRecord)
anvil plan consolidate --project .
```

---

## Sidecar Management

```sh
# Check sidecar status
anvil sidecar status --project .

# Stop the sidecar
anvil sidecar stop --project .

# Restart the sidecar
anvil sidecar start --project .
```

The sidecar auto-exits after the configured idle timeout (default: 30 minutes). Logs are at `.anvil/logs/sidecar.log`.

---

## Audit Store Operations

```sh
# List all audit records of a given type (kebab-case or PascalCase accepted)
anvil audit list gate-approval --project .
anvil audit list reviewer-finding-packet --project .

# Show a specific record by ID
anvil audit show <record-id> --project .

# Run integrity check (detects missing or tampered records)
anvil audit integrity --project .

# Show which records back a cross-reference key
anvil audit provenance <cross-ref-key> --project .
```

**Integrity check before every ship.** The ship gate runs integrity automatically; run it manually if you suspect store corruption.

---

## Hinge Tests

```sh
# List all hinge tests and their status
anvil hinge list --project .

# Run cross-language consensus check (exits non-zero if violations exist)
anvil hinge list --strict --project .

# Count all hinge entries
anvil hinge list --count --project .

# Flip a hinge (record a pin value change)
anvil hinge flip <intended-id> --new-value <value> --reason "Justification" --project .
```

---

## Metrics

```sh
# Show current project health metrics
anvil metrics show --project .

# Show metric history
anvil metrics history --project .
```

The six Layer-1 metrics (reviewer precision, inter-reviewer agreement, human minutes per phase, round count distribution, deferred-decision resolution rate, defect escape rate) are computed from audit store records.

---

## Project Status

```sh
anvil status --project .
```

Shows phase ship status, unresolved rollbacks, active sidecar, and current reviewer in the rotation.

---

## Configuration

```sh
# Show current configuration
anvil config show --project .

# Set a configuration value
anvil config set sidecar.idle_timeout_secs 3600 --project .
```

---

## Headless / CI Mode

The sidecar reads API keys from environment variables when no keychain entry is present:

```sh
# Provide API keys via environment (no keychain required)
ANVIL_API_KEY_ANTHROPIC=... anvil charter review --project .

# Phase reopen in CI (skip interactive confirmation with --yes)
anvil phase reopen P<N> --reason "CI rollback" --yes --project .
```

Exit codes: 0 success · 1 user error · 2 gate refused · 3 sidecar error · 4 audit-store integrity failure · 5 invariant violation.

---

## Publication-Safe History Gate

Before the repository becomes public, complete the following steps. This gate is executed when the project is ready to flip public, not necessarily at phase ship time.

1. **Full-history secret scan** (scan the entire git history, not a bounded range):
   ```sh
   gitleaks detect --source . --log-opts ""
   ```
   Zero unresolved hits required, OR every hit acknowledged with a Coordinator audit record (`anvil audit show <record-id>`).

2. **Full-history license scan**: all dependencies must have compatible licenses (Apache 2.0, MIT, BSD, MPL-2.0 acceptable; GPL not acceptable without explicit Coordinator decision).

3. **Coordinator commit-message review**: all commits reviewed for no embedded secrets or PII in messages.

4. Run `anvil audit integrity --project .` to confirm store health before publication.

5. Record gate completion with a Coordinator sign-off note in the project's audit trail.

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `no anvil.toml found` | Project not initialized | `anvil init .` |
| `sidecar not found` | Binary not on PATH | Set `sidecar.binary_path` in `anvil.toml` |
| `timed out waiting for sidecar` | Slow startup or port conflict | Check `.anvil/logs/sidecar.log` |
| `BlockShip: hinge consensus violations` | Cross-language hinge phase mismatch | `anvil hinge list --strict` then fix annotations |
| `anvil audit integrity` fails | Deleted record or index corruption | Restore from git; do not patch store files manually |
| `EmptyReasoning` error | Missing `--reason` argument | Always provide `--reason` for flip, arbiter, and reopen commands |
