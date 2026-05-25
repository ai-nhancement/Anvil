# Anvil — P0 Bootstrap Review Briefing (R1)

**Date:** 2026-05-25
**Scope:** Stand up Rust workspace, Go module, protobuf placeholder, build orchestration, governance/legal files, and CI configuration.
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P0 — Bootstrap; `Anvil Plan/CHARTER_AMENDMENT_A1.md` §P0 impact
**Tests:** `crates/anvil-cli/src/main.rs` (2 Rust hinge tests), `sidecar/cmd/anvil-sidecar/main_test.go` (2 Go hinge tests) — all 4 passing
**Status:** All acceptance criteria met. No runtime consumers changed — purely additive scaffold.

---

## What Was Built

| File | Action | Purpose |
|---|---|---|
| `Cargo.toml` | Created | Workspace root; 8 member crates; workspace-level lints (`unsafe_code = forbid`, clippy all+pedantic warn) |
| `rust-toolchain.toml` | Created | Pins stable channel; hinge-tested as floor ≥1.80 |
| `.cargo/config.toml` | Created | `-D warnings` rustflags to make clippy denials uniform |
| `rustfmt.toml` | Created | `edition = "2021"` |
| `justfile` | Created | Orchestrates build, test, gen, lint, fmt, fmt-check, dev-sidecar; `windows-shell` set for Git sh on Windows |
| `.golangci.yml` | Created | golangci-lint v2 config; enables errcheck, govet, ineffassign, staticcheck, unused; excludes `internal/contract/` (generated code) |
| `crates/anvil-cli/` | Created | Binary crate `anvil`; `--version` / `-V` flag; 2 hinge tests |
| `crates/anvil-core/` | Created | Stub lib (Vault library) |
| `crates/anvil-audit/` | Created | Stub lib (audit store) |
| `crates/anvil-graph/` | Created | Stub lib (provenance graph) |
| `crates/anvil-sidecar-client/` | Created | Stub gRPC client lib; `build.rs` using `tonic_build::compile_protos`; `include_proto!` behind `#[allow(clippy::all, clippy::pedantic)]` |
| `crates/anvil-eval/` | Created | Stub lib (evaluation metrics) |
| `crates/anvil-hinge/` | Created | Stub lib (hinge-test framework) |
| `crates/anvil-ship/` | Created | Stub lib (ship + rollback) |
| `proto/anvil/v1/sidecar.proto` | Created | `package anvil.v1`; `Sidecar` service with `Ping` RPC; `go_package` option pointing to `internal/contract` |
| `sidecar/go.mod` | Created | Module `github.com/ai-nhancement/Anvil/sidecar`, go 1.22, grpc v1.71.1, protobuf v1.36.6 |
| `sidecar/cmd/anvil-sidecar/main.go` | Created | Binary `anvil-sidecar`; `--version` flag; exits 1 with usage hint otherwise |
| `sidecar/cmd/anvil-sidecar/main_test.go` | Created | 2 Go hinge tests; `fmt.Sscanf` return checked (errcheck-clean) |
| `sidecar/internal/adapters/adapters.go` | Created | Stub package |
| `sidecar/internal/contract/contract.go` | Created | Stub package |
| `sidecar/internal/errors/errors.go` | Created | Stub package |
| `sidecar/internal/server/server.go` | Created | Stub package |
| `LICENSE` | Created | Apache 2.0; Copyright 2026 John Canady Jr. |
| `NOTICE` | Created | Attribution file |
| `CONTRIBUTING.md` | Created | DCO sign-off; `AI-Assisted-By:` trailer (required ≥20 lines or new file in core crates); `Derived-From:` trailer |
| `CODE_OF_CONDUCT.md` | Created | Contributor Covenant 2.1 stub with URL reference; enforcement contact: john@ai-nhancement.com |
| `SECURITY.md` | Created | Supported versions; private disclosure to john@ai-nhancement.com; 90-day window; triage roles; GPG key note |
| `GOVERNANCE.md` | Created | BDFL (John Canady Jr.); maintainer admission/removal; conflict of interest; succession (60-day window); emergency + adversarial emergency freeze |
| `README.md` | Created | References Charter and Plan files; prerequisites; just commands |
| `.gitignore` | Created | `target/`, Go `vendor/`, Anvil runtime dirs |
| `.github/workflows/ci.yml` | Created | Rust build/test/clippy/cargo-audit; Go build/test/golangci-lint/govulncheck; DCO check (dco-org/dco-check); gitleaks secret scan |
| `tests/hinge/.gitkeep` | Created | Placeholder for top-level hinge tests (P10b) |
| `tests/integration/.gitkeep` | Created | Placeholder for end-to-end tests |
| `docs/.gitkeep` | Created | Placeholder for runbook and docs |

---

## Architecture Decisions

**1. `windows-shell` in justfile.**
`just` defaults to `/bin/sh`; Git's sh is installed at `C:\Program Files\Git\bin\sh.exe` on the dev machine. Without this setting every `just` invocation fails on Windows before running any recipe. Set at the top of the justfile so the first-use experience is correct on Windows without requiring any manual shell export.

**2. `tonic_build::compile_protos` (not the deprecated `compile`).**
`tonic-build 0.12` marks `Builder::compile` as deprecated with `#[deprecated]`. With `-D warnings` in `.cargo/config.toml` this becomes a compile error, not a warning. `compile_protos` is the stable replacement and the call site is identical in structure.

**3. `#[allow(clippy::all, clippy::pedantic)]` on the `proto` module.**
Tonic generates code that triggers clippy pedantic lints (missing_errors_doc, etc.). `#[allow(clippy::all)]` alone does NOT suppress the `pedantic` group — it is a separate lint group. Both must be explicitly allowed on the generated-code wrapper module. The allow attribute is scoped to the `proto` mod block only.

**4. golangci-lint v2 config with `version: "2"` header.**
golangci-lint v2.x requires `version: "2"` at the top of `.golangci.yml`; without it the tool refuses to run. Additionally, `gosimple` was merged into `staticcheck` in v2 and is no longer a valid standalone linter name. The enable list uses `staticcheck` only.

**5. golangci-lint `exclude-rules` for `internal/contract/`.**
`protoc`-generated files live in `sidecar/internal/contract/`. They import grpc/protobuf packages and emit style issues that are not ours to fix. The path-based exclude-rules entry suppresses linter output from generated code. Typecheck errors (missing imports) are fixed at the module level via `go get` / `go mod tidy`.

**6. `binaryName` constant for the sidecar hinge test.**
`TestSidecarEntryPointExists` pins the binary name `anvil-sidecar` by asserting against an exported `binaryName` package constant rather than parsing `os.Args[0]`. This avoids test-environment path sensitivity and makes the intent explicit: the constant is the single source of truth for the binary identity.

**7. Amendment A1 governance files in P0.**
Amendment A1 front-loads legal and community files into P0 so every subsequent commit is already under a known license + contribution model. CI enforces DCO on PR commits from the start, preventing debt accumulation.

---

## P0 Success Criteria

| Criterion | Status |
|---|---|
| 1. Fresh-clone build works given Rust ≥1.80 and Go ≥1.22 | **Pass** — `just build` exits 0; both toolchain floor constraints hinge-tested |
| 2. `anvil --version` and `anvil-sidecar --version` print correctly | **Pass** — `anvil 0.1.0` / `anvil-sidecar 0.1.0` |
| 3. `cargo test` and `go test ./...` both succeed | **Pass** — all tests pass, 0 failures |
| 4. All 4 hinge tests pass | **Pass** — `test_rust_toolchain_version_floor`, `test_cli_entry_point_exists`, `TestGoToolchainVersionFloor`, `TestSidecarEntryPointExists` |
| 5. `just gen` regenerates Rust + Go bindings without warnings | **Pass** — `protoc` generates Go bindings; `cargo build -p anvil-sidecar-client` compiles clean |
| 6. README references Charter and Plan files | **Pass** — README.md references `Anvil Plan/new_project_charter.md` and `Anvil Plan/ANVIL_PLAN.md` |

---

## What to Review

1. **Amendment A1 completeness.** CONTRIBUTING.md defines the `AI-Assisted-By:` and `Derived-From:` trailers, but the CI DCO check (`dco-org/dco-check`) only validates standard DCO sign-off. The plan notes trailer enforcement becomes *blocking* on the second update (warning on first). Is the current CI step sufficient for P0, or should a separate trailer-validation job be added now before any commits land?

2. **CODE_OF_CONDUCT.md as a stub.** The file points to the Contributor Covenant 2.1 URL rather than embedding the full text. This is intentional (content-filter constraint during authoring). Review that the URL reference is legally sufficient and that the enforcement contact `john@ai-nhancement.com` is the right long-term address.

3. **`go.sum` reproducibility.** `go mod tidy` resolved dependencies at the time of setup; the go.sum is generated but not yet pinned to a specific Go module proxy mirror. Downstream CI must fetch the same modules. Confirm the CI workflow's Go setup step either caches or pins the module download to avoid non-reproducible builds.

4. **protoc version pinning.** `just gen` calls the system `protoc` binary without a version check. If a developer has a different protoc version, generated bindings may differ slightly. Consider adding a `protoc --version` assertion (or a hinge test in P3a) to catch version drift before it becomes a binary compatibility issue.

5. **`windows-shell` portability.** The justfile hardcodes `C:/Program Files/Git/bin/sh.exe`. On Linux/macOS CI this line is ignored (just uses the platform default). On Windows CI (GitHub Actions) the path should be correct for standard runners. Verify the CI workflow does not need an explicit `windows-shell` override for GitHub-hosted Windows runners.

6. **SBOM and release signing absent in CI.** Amendment A1 requires SBOM generation and release signing. The current CI workflow has the audit and secret-scan jobs but does not yet generate an SBOM or sign artifacts. These are appropriate to add at P9 (Ship + Rollback) when there is actually a release artifact to sign. Confirm this deferral is acceptable.

7. **Stub crates have no `[package]` version inheritance.** Each stub crate should inherit `version`, `edition`, `license`, and `repository` from `[workspace.package]` using `<field>.workspace = true`. Currently only some crates may explicitly declare these; inconsistency could cause a future publish error. Audit the `Cargo.toml` files in all 8 crates.

8. **`anvil-sidecar-client` `Cargo.toml` pin specificity.** `tonic = { version = "0.12" }` and `prost = "0.13"` use caret ranges; `tonic-build = "0.12"` similarly. At P0 these are fine. At P3b when the client is fully implemented, tighter version pins should align with whatever grpc version the sidecar's Go module uses. Flag this as a cross-language version alignment concern for P3a/P3b.

---

## Test Coverage Summary

| Test | Kind | File | Hinge? | Phase |
|---|---|---|---|---|
| `test_rust_toolchain_version_floor` | Unit | `crates/anvil-cli/src/main.rs` | Yes — pins=1.80, intended=stable-floor | P0 |
| `test_cli_entry_point_exists` | Unit | `crates/anvil-cli/src/main.rs` | Yes — pins=anvil, intended=binary-entry-point | P0 |
| `TestGoToolchainVersionFloor` | Unit | `sidecar/cmd/anvil-sidecar/main_test.go` | Yes — pins=1.22, intended=go-stable-floor | P0 |
| `TestSidecarEntryPointExists` | Unit | `sidecar/cmd/anvil-sidecar/main_test.go` | Yes — pins=anvil-sidecar, intended=binary-entry-point | P0 |

All 4 hinge tests are ordinary unit tests with the structured comment annotation above the function. P10b will auto-discover them via the comment convention; until then they run as part of the normal `cargo test` / `go test` suites.

No integration tests at P0 — the integration test directory placeholder is in place for P5+.
