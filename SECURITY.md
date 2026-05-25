# Security Policy

## Supported versions

During the pre-release period (P0–P11), only the latest commit on `main` is supported.
Once v1 ships, the current release and the previous minor release receive security fixes.

## Reporting a vulnerability

Please **do not** file a public GitHub issue for security vulnerabilities.

Report privately to: **john@ai-nhancement.com**

Include in your report:
- Description of the issue
- Steps to reproduce or proof-of-concept
- Affected versions or commits
- Your assessment of severity and impact

You will receive an acknowledgment within 3 business days.
The coordinated disclosure window is **90 days** from the acknowledgment date.
We will work with you on timing if a patch needs more time; we ask that you honour the window in return.

## Threat model (v1 scope)

Anvil v1 is a local-first, single-user CLI tool. The relevant threat surface is:

- **Credential storage.** API keys are stored in the OS keychain (Windows Credential Manager, macOS Keychain, Linux Secret Service). Anvil never writes keys to disk in plaintext.
- **Audit store integrity.** The audit store is append-only at the application layer. Anvil detects missing records via an index-vs-disk completeness check. It does not defend against an attacker who can modify both the record file and the index entry simultaneously.
- **Sidecar communication.** The sidecar communicates over loopback gRPC only. No remote sidecar endpoint is supported in v1.
- **Supply chain.** Dependencies are scanned via `cargo audit` (Rust) and `govulncheck` (Go) in CI. An SBOM (CycloneDX) is published with every release.

Out of scope for v1: multi-user isolation, network-exposed sidecar, cryptographic tamper-proofing of audit records.

## Vulnerability triage roles

| Role | Responsibility |
|---|---|
| **BDFL (John Canady Jr.)** | Final triage authority, patch approval, coordinated disclosure coordination |
| **Maintainers** | Initial acknowledgment, reproduction, severity assessment |

## GPG key

Release artifacts are signed with the project GPG key. Key fingerprint and public key will be published on the GitHub repository and Releases page at v1 ship.
