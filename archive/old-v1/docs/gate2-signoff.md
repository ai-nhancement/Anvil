# Gate 2 Sign-Off: Anvil v1

**Date:** 2026-05-28
**Release:** v1.0.0
**Authors:** AI-nhancement development team (dogfooded via Anvil v1 CLI)

---

## Acceptance Criteria Summary

### AC1: Live Dogfooding — Anvil v1.1 Charter + Plan via v1 CLI

**Status: COMPLETE**

The Anvil v1.1 App (Tauri + React desktop shell) charter and plan were produced using
the Anvil v1.0.0 CLI against live AI providers (DigitalOcean Serverless Inference,
OpenAI-compatible gateway at https://inference.do-ai.run).

Provider configuration:
- Coder/Planner/Interlocutor: anthropic-claude-opus-4.6 (Gradient)
- Reviewer-1: openai-gpt-5.5 (Gradient)
- Reviewer-2: deepseek-v4-pro (Gradient)

Artifacts produced:
- `C:\Anvil-v11-dogfood\charter.md` — Anvil v1.1 App charter (convergence declared)
- `C:\Anvil-v11-dogfood\.anvil\plan_contract.json` — 12-phase plan contract
- Plan reviewed (R1) with 14 findings captured; convergence declared

Audit trail: `C:\Anvil-v11-dogfood\audit-store\`

---

### AC2: Live External Pilot — Leaflog Houseplant CLI

**Status: COMPLETE**

The Leaflog project (houseplant watering tracker CLI in Rust) was developed through a full
Charter -> Plan -> Build -> Ship cycle using Anvil v1.0.0 with multi-reviewer rotation.

Pilot configuration:
- Project root: `C:\Leaflog-pilot\`
- Reviewer pool: reviewer-1 (GPT-5.5) + reviewer-2 (DeepSeek-v4-pro)
- single_clean_pass_override: true (full-pool-clean architecturally impossible with sequential
  rotation; one clean pass sufficient)

Phases shipped (all 7):
- P0: Bootstrap (Rust CLI skeleton)
- P1: Database Layer (SQLite schema + migrations)
- P2: Plant Management Commands (add, list, delete)
- P3: Watering Operations (water, log subcommands)
- P4: Reminders (remind command)
- P5: Export (CSV/JSON export)
- P6: Status & Polish (full integration)

Each phase completed: 2 build rounds, 2 review rounds, findings curated, blocking findings
resolved via arbiter, shipped.

Audit trail: `C:\Leaflog-pilot\audit-store\`

---

### AC3: v1.1 Plan Validated as v1.1 App Design Input

**Status: COMPLETE**

The 12-phase Anvil v1.1 App plan (produced in AC1) has been:
1. Reviewed by reviewer-1 (GPT-5.5) with 14 findings captured
2. All findings curated: 1 advisory accepted, 13 material findings locked pending plan
   (standard disposition for plan-level architectural findings)
3. Convergence declared on both charter.md and plan.md

The plan contract at `C:\Anvil-v11-dogfood\.anvil\plan_contract.json` is the authoritative
design input for Anvil v1.1 App development. Key architectural decisions captured:

- 12-phase development arc (P0 Bootstrap through P11 Packaging)
- Tauri IPC boundary design (P1)
- Navigation shell and routing (P2)
- Dashboard, Charter, Plan, Phase, Audit Log panels (P3-P7)
- Hinge registry and metrics dashboard (P8)
- Setup wizard (P9)
- End-to-end validation and CLI coexistence (P10)
- Distributable packaging for macOS, Windows, Linux (P11)

Reviewer findings (F1-F13, material severity) are tracked as design input constraints that
must be resolved during Anvil v1.1 App development.

---

### AC4: Release Engineering

**Status: COMPLETE**

Release artifacts at `C:\Anvil\releases\v1.0.0\`:

| File | Description |
|---|---|
| `anvil-1.0.0-windows-x86_64.zip` | Windows x86_64 release archive |
| `anvil-1.0.0-windows-x86_64/anvil.exe` | Anvil CLI binary (Rust, release build) |
| `anvil-1.0.0-windows-x86_64/anvil-sidecar.exe` | AI gateway sidecar (Go, release build) |
| `anvil-1.0.0-windows-x86_64/README.txt` | Installation and verification instructions |
| `SHA256SUMS.txt` | SHA256 checksums for all release files |
| `SHA256SUMS.txt.asc` | GPG signature placeholder (pending release key setup) |
| `smoke-test.ps1` | Automated smoke-test script |

Smoke-test results (11/11 passed):
- Archive integrity (SHA256 checksum verification)
- Binary presence in archive
- `anvil --help` exits 0
- `anvil --version` outputs "1.0.0"
- `anvil phase --help`, `anvil charter --help`, `anvil plan --help` exit 0
- `anvil-sidecar.exe` is a valid Windows PE (MZ header)

Build commands used:
  cargo build --release -p anvil-cli
  (sidecar) go build -ldflags="-s -w" -o anvil-sidecar.exe ./cmd/anvil-sidecar/

GPG note: GPG signing requires a dedicated release key. The SHA256SUMS.txt.asc placeholder
is included to track this as a pending step before public distribution.

Linux and macOS builds: not included in v1.0.0 (Windows-only pilot release). Cross-platform
builds are tracked as AC items for the public v1.0 release.

---

## Gate 2 Decision

All four acceptance criteria are SATISFIED. Gate 2 is hereby signed off.

Anvil v1.0.0 is approved for internal release and Anvil v1.1 App development may proceed
using the validated Plan as design input.

Open items (non-blocking for Gate 2):
1. GPG signing key setup for public release signature
2. Linux/macOS build targets for cross-platform distribution
3. v1.1 App development (12 phases, tracked in C:\Anvil-v11-dogfood\)
