# Anvil — P0 Bootstrap Review Briefing (R4)

**Date:** 2026-05-25
**Scope:** R3 finding applied. All tests passing, lint clean, format clean.
**Status:** Ready for approval.

---

## Findings from R3

| Finding | Severity | Fix Applied | Test / Verification |
|---|---|---|---|
| 1. `pub const BINARY_NAME` in binary crate — unnecessary public visibility | Low | Changed to `pub(crate) const BINARY_NAME: &str = "anvil"` | `just test` passes; hinge test `test_cli_entry_point_exists` still passes via `use super::BINARY_NAME` |
| 2. All R2 fixes confirmed present — no regressions | — | No change | — |
| 3. R3 table wording slightly conflated action version with linter version | Advisory | No code change; noted for future review-doc hygiene | — |

---

## Post-Fix Verification

```
just test      → 4 hinge tests pass (2 Rust, 2 Go)
just lint      → 0 issues (cargo clippy + golangci-lint)
just fmt-check → clean (rustfmt + gofmt)
```

---

## Deferred Items (unchanged)

| Item | Reason | Target phase |
|---|---|---|
| `AI-Assisted-By:` / `Derived-From:` trailer CI enforcement | No substantive contributions until P3+; first occurrence is a warning per CONTRIBUTING.md | P3 or P5 |
| SBOM generation and release signing | No release artifact until P9 | P9 |
| `buf` for stricter proto schema control | Appropriate at P3a when full contract is defined | P3a |
