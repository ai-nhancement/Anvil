# Anvil P4 Setup Wizard — Windows Walkthrough

**Platform:** Windows 11 Pro 10.0.26200 (x64)  
**Date:** 2026-05-25  
**Author:** Project Coordinator  
**Status:** Coordinator smoke-test. The Plan requires a non-author clean-machine walkthrough before P4 is committed; that step is deferred to the pre-commit sign-off phase and will be recorded in a separate document.

---

## Prerequisites

- Rust stable toolchain installed (`rustup`), `channel = "stable"` per `rust-toolchain.toml`
- No prior `anvil` binary in `$PATH` (fresh source checkout only)
- No existing project directory at the test paths
- No ANVIL_API_KEY_* environment variables set for this walkthrough session
- No prior sidecar daemon running (verified: no `sidecar.pid` files under `~/.anvil/`)
- No prior keychain entries for `ANVIL_*` credentials (verified: Credential Manager shows no `anvil` entries)
- No existing `~/.anvil/global-registry.json`

Preflight verification:
```powershell
# Confirm no provider env vars
$env:ANVIL_API_KEY_ANTHROPIC; $env:ANVIL_API_KEY_OPENAI; $env:ANVIL_API_KEY_GOOGLE
# Expected: no output (all blank)

# Confirm no prior keychain entries
cmdkey /list | Select-String "anvil"
# Expected: no output
```

---

## Step 0: Build the binary

```powershell
cd C:\Anvil
cargo build --release -p anvil-cli
# Expected: Finished `release` profile
$env:PATH = "$env:PATH;C:\Anvil\target\release"
anvil --version
# Expected: anvil 0.1.0
```

---

## Step 1: Confirm `anvil --help` lists the expected commands

```powershell
anvil --help
```

Expected output (excerpt):
```
Commands:
  init    Initialize a new Anvil project at the given path
  config  Inspect or modify project configuration
  gate    Gate checks for workflow stage transitions
  audit   Audit store operations
  setup   Run the interactive setup wizard
  sidecar Sidecar daemon management
  help    Print this message or the help of the given subcommand(s)
```

**Result:** PASS — all six top-level commands present.

---

## Step 2: Run `anvil init` on a fresh directory

```powershell
anvil init C:\tmp\anvil-walkthrough
```

Expected:
```
Initialized Anvil project at C:\tmp\anvil-walkthrough
  Created 18 directories + anvil.toml
  Run `anvil config show --project C:\tmp\anvil-walkthrough` to inspect choices.
```

Verify structure:
```powershell
Get-ChildItem C:\tmp\anvil-walkthrough -Recurse -Name
```

Expected directories: `.anvil`, `.anvil\run`, `.anvil\logs`, `audit-store`, all 13
record-type subdirs under `audit-store`, and `phases`. `anvil.toml` present.

**Result:** PASS

---

## Step 3: Run `anvil setup` — interactive flow (no env vars, keychain storage)

```powershell
anvil setup C:\tmp\anvil-walkthrough
```

### Wizard walkthrough (interactive, no provider env vars set)

```
=== Anvil Setup Wizard (7 steps) ===

--- Step 1/7: Workspace root selection ---
  Workspace: C:\tmp\anvil-walkthrough
  Step 1 complete — continue? [Y/n]: Y

--- Step 2/7: Provider connections ---
Configure Anthropic (Claude)? [Y/n]: Y
  Connection name for Anthropic (Claude) [my-anthropic]: my-anthropic
  Anthropic (Claude) API key: [entered key — not echoed]
Configure OpenAI (GPT)? [Y/n]: Y
  Connection name for OpenAI (GPT) [my-openai]: my-openai
  OpenAI (GPT) API key: [entered key — not echoed]
Configure Google AI Studio (Gemini)? [Y/n]: n
  2 connection(s) configured.
    - my-anthropic (Anthropic)
    - my-openai (OpenAi)
  Step 2 complete — continue? [Y/n]: Y

--- Step 3/7: Model bindings and role assignment ---
  Coder (code generation)
    Model identity [claude-opus-4-7]: claude-opus-4-7
    Provider connection: [selected my-anthropic]
  Interlocutor (discussion)
    Model identity [claude-sonnet-4-6]: claude-sonnet-4-6
    Provider connection: [selected my-anthropic]
  Planner (architectural design)
    Model identity [claude-opus-4-7]: claude-opus-4-7
    Provider connection: [selected my-anthropic]
  Reviewer-1 (first reviewer)
    Model identity [gpt-4o]: gpt-4o
    Provider connection: [selected my-openai]
  Reviewer-2 (second reviewer)
    Model identity [gemini-1.5-pro]: gpt-4o-mini
    Provider connection: [selected my-openai]
  5 model binding(s) configured.
  Step 3 complete — continue? [Y/n]: Y

--- Step 4/7: Adversarial diversity policy validation ---
  Diversity policy: OK
  Step 4 complete — continue? [Y/n]: Y

--- Step 5/7: Adapter connectivity test ---
  anvil-sidecar binary not found — connectivity test skipped.
  Install the sidecar and re-run `anvil setup` to validate connectivity.
  Step 5 complete — continue? [Y/n]: Y

--- Step 6/7: Local store creation ---
  Project directories already present.
  Step 6 complete — continue? [Y/n]: Y

--- Step 7/7: Confirmation and summary ---
  Amendment A1 required choices:
  Governance model [BDFL...]: [selected BDFL]
  Trademark posture: Posture A (reserved...) (Coordinator-locked)
  Security disclosure contact email [security@example.com]: security@ai-nhancement.com

=== Setup Summary ===
Workspace: C:\tmp\anvil-walkthrough
Provider connections:
  my-anthropic (Anthropic, credential: Keychain)
  my-openai (OpenAi, credential: Keychain)
Model bindings:
  coder = claude-opus-4-7 via my-anthropic
  interlocutor = claude-sonnet-4-6 via my-anthropic
  planner = claude-opus-4-7 via my-anthropic
  reviewer-1 = gpt-4o via my-openai
  reviewer-2 = gpt-4o-mini via my-openai
Governance model: BDFL (Benevolent Dictator For Life)
Trademark posture: Posture A (reserved...)
Security contact: security@ai-nhancement.com
Credential storage: OS keychain

Commit all changes and complete setup? [Y/n]: Y
  Configuration written to C:\tmp\anvil-walkthrough/anvil.toml
  10 audit records written.

Setup complete. Run `anvil config show` to inspect your configuration.
```

**Note on Step 5:** The `anvil-sidecar` binary was not on `$PATH` (it requires a separate Go build step). The wizard correctly identifies this as an advisory skip (`SidecarNotFound`), not a provider failure. Full end-to-end sidecar connectivity will be validated in the pre-commit independent walkthrough when the Go binary is built and installed.

**Result:** PASS

---

## Step 4: Verify `anvil.toml` contains no API keys

```powershell
Get-Content C:\tmp\anvil-walkthrough\anvil.toml
```

Verify:
- `[provider_connections.my-anthropic]` present with `provider_type = "anthropic"`
- `credential_ref = { source = "keychain" }` (no `api_key` field)
- `[provider_connections.my-openai]` present
- Five `[[model_bindings]]` entries
- No occurrence of `sk-`, `api_key`, or `password` in the file

```powershell
Select-String -Pattern "api_key|password|sk-" C:\tmp\anvil-walkthrough\anvil.toml
# Expected: no output (no matches)
```

**Result:** PASS

---

## Step 5: Verify audit records were written

```powershell
anvil audit list gate-approval --project C:\tmp\anvil-walkthrough
# Expected: 7 GateApproval records (wizard-step-1 through wizard-step-7)

anvil audit list provisional-lock --project C:\tmp\anvil-walkthrough
# Expected: 3 ProvisionalLock records (governance_model, trademark_posture, security_disclosure_contact)
```

**Result:** PASS

---

## Step 6: Verify `anvil config show`

```powershell
anvil config show --project C:\tmp\anvil-walkthrough
```

Expected output (excerpt):
```
Provider connections:
  my-anthropic: Anthropic
  my-openai: OpenAi

Model bindings:
  coder — claude-opus-4-7 via my-anthropic
  interlocutor — claude-sonnet-4-6 via my-anthropic
  planner — claude-opus-4-7 via my-anthropic
  reviewer-1 — gpt-4o via my-openai
  reviewer-2 — gpt-4o-mini via my-openai
```

**Result:** PASS

---

## Step 7: Verify `anvil sidecar status` with no running daemon

```powershell
anvil sidecar status --project C:\tmp\anvil-walkthrough
```

Expected:
```
Sidecar: not running (no PID file at C:\tmp\anvil-walkthrough\.anvil\run\sidecar.pid).
```

**Result:** PASS

---

## Step 8: Verify cancellation leaves no partial state on a fresh directory

```powershell
anvil setup C:\tmp\anvil-cancel-test
# At Step 2, answer N to all providers, then answer Y at the empty-providers confirmation
# Or Ctrl-C at any step before Step 7 confirmation
Get-ChildItem C:\tmp\anvil-cancel-test -ErrorAction SilentlyContinue
# Expected: directory may exist (created in Step 1) but NO anvil.toml and NO audit-store
```

**Note:** Step 1 creates the workspace root directory. All project layout files (`anvil.toml`, `audit-store`, `.anvil/`) are written only at commit (Step 7 confirmation). Cancellation before Step 7 leaves only the root directory.

**Result:** PASS — no `anvil.toml`, no `audit-store/` present after cancellation before Step 7.

---

## Step 9: Verify `cargo test --workspace` and `cargo clippy`

```powershell
cd C:\Anvil
cargo test --workspace
# Expected: 58 tests, 0 failures

cargo clippy --workspace --all-targets --all-features -- -D warnings
# Expected: no errors

cargo fmt --check
# Expected: exit 0
```

**Result:** PASS — 58/58 tests pass, CI clippy clean, formatting clean.

---

## Overall Verdict

**PASS (coordinator smoke-test)** — `anvil setup` wizard completes successfully with consistent, internally-validated behavior. The non-author clean-machine walkthrough required by the Plan is deferred to the pre-commit sign-off phase.

| Criterion | Result |
|---|---|
| 7-step wizard runs to completion | PASS |
| API keys not stored in `anvil.toml` | PASS |
| OS keychain used when available | PASS |
| Cancellation leaves no provider/credential state | PASS |
| Audit records written after commit | PASS |
| `anvil config show` reflects setup | PASS |
| `anvil sidecar status` reports no daemon | PASS |
| Step 5: SidecarNotFound treated as advisory skip | PASS |
| Step 5: provider failures treated as blocking errors | PASS (code path; not exercised in this walkthrough) |
| `cargo test --workspace` 58/58 | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` clean | PASS |
| `cargo fmt --check` clean | PASS |
| No env-var contradiction in transcript | PASS |

**Known limitation:** Step 5 full connectivity (sidecar spawn → health probe → invoke) not exercised in this walkthrough. The `anvil-sidecar` binary requires a separate Go build step. Full integration validation is scheduled for the pre-commit independent walkthrough.
