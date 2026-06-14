# Anvil — P0 Bootstrap Review Briefing (R2)

**Date:** 2026-05-25
**Scope:** R1 findings applied. All tests passing, lint clean, format clean.
**Status:** Ready for approval.

---

## Findings from R1

| Finding | Severity | Fix Applied | Test |
|---|---|---|---|
| 1. Rust hinge test asserts crate name instead of binary name | High | Added `pub const BINARY_NAME: &str = "anvil"` at crate root; updated `test_cli_entry_point_exists` to assert `BINARY_NAME == "anvil"`; main uses `BINARY_NAME` for all output | `test_cli_entry_point_exists` now correctly pins the binary name, consistent with Go pattern |
| 2. Unpinned `protoc` in CI (`arduino/setup-protoc@v3` with no version) | Medium | Pinned `version: "25.3"` in both the Rust and Go CI jobs | n/a — CI config change; no runtime test available |
| 3. Go module build reproducibility: no `GOPROXY` / `GONOSUMDB` in CI | Medium | Added job-level `env: GOPROXY: https://proxy.golang.org,direct` and `GONOSUMDB: "off"` to the Go job in `ci.yml` | n/a — CI config change |
| 4. DCO trailer enforcement incomplete vs. CONTRIBUTING.md | Low | Added explanatory comment to the `dco` CI job documenting the planned `AI-Assisted-By:`/`Derived-From:` trailer-grep step, its trigger condition (before P3/P5), and the CONTRIBUTING.md threshold reference | n/a — documentation change |
| 5. CODE_OF_CONDUCT.md stub legally acceptable | — | No change needed; confirmed acceptable | — |
| 6. Workspace inheritance (review point 7 unfounded) | — | Confirmed all 8 crates inherit from `[workspace.package]`; no inconsistency | — |
| 7. windows-shell portability | — | No change; CI is ubuntu-latest, Windows path is local-dev only | — |
| 8. SBOM / signing deferred to P9 | — | Confirmed; current audit + gitleaks jobs appropriate for P0 | — |

---

## Post-Fix Verification

All commands run clean after applying fixes:

```
just test    → 4 hinge tests pass (2 Rust, 2 Go)
just lint    → 0 issues (cargo clippy + golangci-lint)
just fmt-check → clean (rustfmt + gofmt)
```

---

## Deferred Items

| Item | Reason | Target phase |
|---|---|---|
| `AI-Assisted-By:` / `Derived-From:` trailer CI enforcement | No substantive contributions land until P3+; first occurrence triggers warning per CONTRIBUTING.md | P3 or P5 (before first core-crate PR) |
| SBOM generation and release signing | No release artifact exists until P9 | P9 |
| `buf` for protoc management (stricter schema control) | Correct approach at P3a when the full contract is defined; overkill at P0 | P3a |
| `protoc-gen-go` / `protoc-gen-go-grpc` pinned versions in CI | Currently `@latest`; acceptable at P0 while proto is a placeholder; pin at P3a | P3a |
