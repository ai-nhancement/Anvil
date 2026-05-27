# Getting Started with Anvil v1

Anvil is a CLI workflow tool that brings structure to AI-assisted software development. It enforces review gates, tracks provenance, uses adversarial cross-vendor AI diversity, and produces an auditable record of every decision.

This guide takes you from nothing to a running project in 10–15 minutes.

---

## Step 1 — Install

Build from source (Rust ≥ 1.80, Go ≥ 1.22 required):

```sh
git clone https://github.com/ai-nhancement/Anvil
cd Anvil

# Build the CLI
cargo build --release -p anvil-cli

# Build the sidecar
cd sidecar && go build -o ../target/release/anvil-sidecar ./cmd/anvil-sidecar

# Add both binaries to your PATH
export PATH="$PWD/../target/release:$PATH"
```

Verify installation:

```sh
anvil --version
anvil-sidecar --version
```

---

## Step 2 — Initialize Your Project

```sh
mkdir my-project && cd my-project
anvil init .
```

The path argument is required. Use `.` to initialize in the current directory. This creates `.anvil/` with the project structure. The directory is safe to inspect; do not edit files inside `.anvil/audit-store/` by hand.

---

## Step 3 — Run Setup

```sh
anvil setup .
```

The interactive wizard asks for:

1. **Project name and description**
2. **Provider connections** — your API credentials for Claude (Coder), GPT (reviewer), Gemini (reviewer). At minimum: two distinct reviewer families different from Claude.
3. **Governance model** — how major decisions are made
4. **Trademark posture** — for open-source projects
5. **Security disclosure contact** — who to contact for vulnerabilities
6. **Single-clean-pass override** — emergency bypass if all reviewers agree but full-pool termination fails
7. **Sidecar binary path** — if `anvil-sidecar` is not on `$PATH`

After the wizard, `anvil.toml` is written and your credentials are stored encrypted in the OS keychain.

For CI and scripted environments, supply API keys via environment variables — the sidecar reads them automatically when no keychain entry is present:

```sh
ANVIL_API_KEY_ANTHROPIC=<key> \
ANVIL_API_KEY_OPENAI=<key> \
ANVIL_API_KEY_GOOGLE=<key> \
anvil charter review --project .
```

---

## Step 4 — Write Your Charter

The Charter is a markdown document describing your project's purpose, principles, and constraints. Place it in your project root:

```sh
# Example minimal charter
cat > charter.md << 'EOF'
# My Project Charter

## Purpose
...

## Core Principles
...

## Constraints
...
EOF
```

See the Anvil Charter (`new_project_charter.md`) as a worked example of the format.

---

## Step 5 — Charter Review

Send your charter to the AI reviewer pool:

```sh
anvil charter review --project .
```

The `anvil-sidecar` starts automatically. The reviewer (your configured non-Claude model family) reads the charter and returns a structured findings packet. This may take 30–90 seconds.

View the findings:

```sh
anvil charter findings --project .
```

---

## Step 6 — Curate Findings

For each finding, decide whether to keep it (and fix the charter) or drop it (with reasoning).

First, get the packet UUID:
```sh
anvil audit list reviewer-finding-packet --project .
```

Then resolve each finding using the composite `<packet-uuid>:<finding-id>` form:
```sh
# Resolve a finding (keep, drop, or annotate — record your reasoning)
anvil arbiter resolve-finding "<packet-uuid>:F1" \
    --reason "Valid concern about scope — will revise §2" \
    --project .

anvil arbiter resolve-finding "<packet-uuid>:F2" \
    --reason "Refuted: the charter already covers this in §3" \
    --chosen-direction "drop" \
    --project .
```

Fix the charter to address kept findings, then run `anvil charter review --project .` again for the next round.

---

## Step 7 — Declare Convergence

When reviewers produce a clean pass (no P1/P2 findings remain), declare convergence:

```sh
anvil arbiter declare-convergence charter.md \
    --reason "Full-pool clean on R3." \
    --project .
```

The first argument is the artifact name (e.g. `charter.md`). Round count is computed automatically from audit records.

---

## Step 8 — Plan Stage

After charter convergence, generate the project plan:

```sh
anvil plan invoke --project .
```

The plan AI generates a phased implementation plan based on your charter. Review, curate, and iterate the same way you did with the charter, using `anvil plan review --project .` and `anvil plan findings --project .`.

---

## Step 9 — Build Stage

For each phase (phase ID is a positional argument, not a flag):

```sh
# Build it (the Coder implements the phase)
anvil phase build P0 --project .

# Review it
anvil phase review P0 --project .

# Ship it when all findings addressed
anvil phase ship P0 --project .
```

---

## Step 10 — Project Ship

When all phases are shipped:

```sh
anvil ship --project .
```

The ship gate checks:
- All phases in shipped state
- No unresolved rollbacks
- Hinge consensus passes (`anvil hinge list --strict`)

If the gate passes, the configured transport actions run (e.g., `git commit`, release upload).

---

## Key Concepts

**Audit store** — every decision is recorded as an immutable append-only audit record in `.anvil/audit-store/`. Never edit these files.

**Hinge tests** — `// hinge_test: pins=X, intended=Y, phase=Z` comments in your test files track deferred decisions. Use `anvil hinge list --project .` to see them.

**Reviewer diversity** — your reviewer pool must have at least two distinct AI model families, neither being Claude (the Coder). This adversarial diversity catches blind spots.

**Provisional Locks** — design decisions captured during setup with explicit revision triggers. Use `anvil audit list provisional-lock --project .` to see them.

**Rotation** — if no reviewer produces a clean pass by round 5, P2/P3 findings become advisory (non-blocking). Round 6+ are advisory by default.

---

## What's Next

- `docs/runbook.md` — all commands and troubleshooting
- `docs/contract.md` — sidecar gRPC contract reference
- `docs/ux-audit.md` — CLI command structure audit
- `new_project_charter.md` — Anvil's own charter (worked example)
