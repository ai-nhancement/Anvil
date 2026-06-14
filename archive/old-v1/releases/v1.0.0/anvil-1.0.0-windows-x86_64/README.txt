Anvil v1.0.0 — Windows x86_64
==============================

Contents
--------
  anvil.exe           — Anvil CLI (main binary)
  anvil-sidecar.exe   — AI gateway sidecar (auto-managed by anvil.exe)
  README.txt          — This file

Quick start
-----------
1. Place both binaries in the same directory (or a directory on your PATH).
2. Run: anvil --help
3. Initialize a workspace: anvil init --project <path>
4. The sidecar is started automatically on first use; no manual setup needed.

Provider configuration
----------------------
Copy anvil.toml from a template workspace and configure your AI provider(s).
API keys are referenced by environment variable name — never stored in anvil.toml.
Example:

  [provider_connections.my-claude]
  provider_type = "anthropic"
  endpoint = "https://api.anthropic.com"
  credential_ref = { source = "env_var", var_name = "ANTHROPIC_API_KEY" }

Requirements
------------
  Windows 10/11 (x86_64)
  No additional runtime dependencies (binaries are statically linked)

Verification
------------
  SHA256 checksums: SHA256SUMS.txt
  GPG signature:    SHA256SUMS.txt.asc  (key: keys.openpgp.org, fingerprint in CHECKSUMS.txt)

Anvil v1.0.0 is the first general-availability release.
See CHANGELOG or https://github.com/ai-nhancement/Anvil/releases for details.
