# Anvil — P0 Bootstrap Review Briefing (R3)

**Date:** 2026-05-25
**Scope:** R2 findings applied. All tests passing, lint clean, format clean.
**Status:** Ready for approval.

---

## Findings from R2

| Finding | Severity | Fix Applied | Test / Verification |
|---|---|---|---|
| 1. `gofmt -l .` exits 0 on violations — Go format check silently passes | Critical | `justfile` `fmt-check` recipe changed to `test -z "$(gofmt -l .)"` — exits non-zero if any file is unformatted | Verified: `just fmt-check` passes clean; a hand-tested unformatted file causes exit 1 |
| 2. Go format check absent from CI | Critical | Added `Format check (Go)` step to Go CI job: `test -z "$(gofmt -l .)"` in `sidecar/` working directory | CI config change — logic mirrors the fixed justfile recipe |
| 3. `golangci-lint-action` version: `latest` | High | Pinned to `v2.12.2` (matches local dev install) | CI config change |
| 4. `govulncheck` installed via `@latest` | High | Pinned to `v1.3.0` | CI config change |
| 5. `protoc-gen-go` installed via `@latest` | High | Pinned to `v1.36.11` (matches `google.golang.org/protobuf` in go.mod) | CI config change |
| 6. `protoc-gen-go-grpc` installed via `@latest` | High | Pinned to `v1.6.2` (matches grpc version in go.mod) | CI config change |
| 7. CI inlines protoc command — can diverge from `just gen` | Medium | Split `gen` recipe into `gen-go` (protoc only) and `gen-rust` (cargo build only); `gen` calls both. Go CI job installs `just` (`extractions/setup-just@v2`) and runs `just gen-go` | Single source of truth for the protoc invocation; Go job no longer has an inline copy |
| 8. Rust hinge test checks `contains("stable")` — matches comment lines | Low | Updated `test_rust_toolchain_version_floor` to check `l.trim() == r#"channel = "stable""#` — exact key/value line match | `test_rust_toolchain_version_floor` passes; would fail if channel were changed to `beta` or `nightly` or if the line were comment-only |
| 9. Proto plugin binaries re-downloaded every CI run | Advisory | Added `actions/cache` keyed on exact plugin versions before the install step | Cache hit on repeat runs; key encodes both versions so a version bump invalidates correctly |

---

## Post-Fix Verification

```
just test      → 4 hinge tests pass (2 Rust, 2 Go)
just lint      → 0 issues (cargo clippy + golangci-lint)
just fmt-check → clean (rustfmt + gofmt, exits non-zero on violations)
```

---

## Deferred Items (unchanged from R2)

| Item | Reason | Target phase |
|---|---|---|
| `AI-Assisted-By:` / `Derived-From:` trailer CI enforcement | No substantive contributions land until P3+; first occurrence triggers warning per CONTRIBUTING.md | P3 or P5 |
| SBOM generation and release signing | No release artifact until P9 | P9 |
| `buf` for stricter proto schema control | Appropriate at P3a when full contract is defined | P3a |
