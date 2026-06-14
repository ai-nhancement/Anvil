# P11 Dogfooding and Documentation — Review Briefing (R11)

**Date:** 2026-05-27  
**Scope:** P11 R10 finding responses — 3 applied via code/doc changes; 1 addressed via briefing-language correction only  
**Plan spec:** `Anvil Plan/ANVIL_PLAN.md` §P11 — Dogfooding and Documentation  
**Tests:** 190 passing (20 audit, 63 cli, 49 core, 14 eval, 9 graph, 7 hinge, 17 ship, 11 sidecar-client), 0 failed  
**Clippy:** Clean (`-D warnings`, all targets)  
**Fmt:** Clean (`cargo fmt --all -- --check`)  
**Go tests:** All passing (`go test ./...` from `sidecar/`)

Prior rounds: R1 first-pass (clean); R1 second-pass (8 findings); R2 (5); R3 (7); R4 (5); R5 (7); R6 (5); R7 (6); R8 (4); R9 (7); R10 (4). All findings addressed across all rounds.

---

## R10 Finding Responses

### F1 (High) — Missed "dogfooding session" in Cross-Cutting Concerns at line 877

**Disposition: Applied.**

`ANVIL_PLAN.md` line 877 (Cross-Cutting Concerns, `cli-command-structure` bullet):

> "confirmed Final at P11 after evaluation against `docs/ux-audit.md` and dogfooding session"

→

> "confirmed Final at P11 after evaluation against `docs/ux-audit.md` and build observations"

This was the sole remaining instance of the "dogfooding session" phrasing after the five R9 locations were updated. The R10 briefing did not list it; it was caught by the reviewer's exhaustive text scan.

---

### F2 (High) — Plan-Level Gate 2 (3 items) diverged from P11 Gate 2 (4 items)

**Disposition: Applied.**

Plan-Level Gate 2 now contains four items matching P11 Gate 2 exactly:

1. Live dogfooding: Charter and Plan for Anvil v1.1 via v1 CLI (real AI providers, audit-store records).
2. Live external pilot: full Charter → Plan → Build → Ship with multi-reviewer rotation (real AI providers, audit-store records).
3. The v1.1 Plan from live dogfooding validated as the input for the v1.1 App design. *(previously only in P11 Gate 2)*
4. Release archive + `SHA256SUMS.txt` + signed `SHA256SUMS.txt.asc` + smoke-test script passes against release candidate. *(renumbered from 3)*

The two Gate 2 lists are now identical. Future changes to either must update both, which can be caught by a reviewer comparing the sections.

---

### F3 (Medium) — Slug extraction whitespace-intolerant; trim applied only to section headers

**Disposition: Applied.**

The table-row processing now trims both the line and the extracted slug:

```rust
.filter_map(|line| {
    let cols: Vec<&str> = line.trim().split('|').collect();
    cols.get(1).and_then(|col| {
        let mut parts = col.split('`');
        parts.next(); // text before first backtick
        parts.next().map(|s| s.trim().to_string()) // the slug, trimmed
    })
})
```

A comment was added explaining the whitespace-tolerant requirement. Both the forward and reverse assertions now operate on trimmed slugs, so a cosmetic markdown edit (extra spaces around a backtick or pipe) cannot cause a spurious bidirectional check failure.

---

### F4 (Low) — AC3 language still presented the test as stronger than its implementation

**Disposition: Addressed in briefing language.**

AC3 in this briefing's table reads:

> `docs/contract.md` contains every service and RPC name from `sidecar.proto` (substring smoke test only)

This makes the limitation visible at the AC level rather than only inside the test comment. The underlying implementation is unchanged; the test remains intentionally minimal with full schema sync deferred to v1.1.

---

## Acceptance Criteria Status

| AC | Criterion | Status |
|---|---|---|
| AC1 | `docs/runbook.md` covers all P0–P11 Coordinator workflows | **PASS** |
| AC2 | `docs/onboarding.md` covers contributor setup and first commands | **PASS** |
| AC3 | `docs/contract.md` contains every service and RPC name from `sidecar.proto` (substring smoke test only; full schema sync is v1.1) | **PASS** |
| AC4 | All 8 Provisional Locks confirmed Final (Gate 1) | **PASS** |
| AC5 | Hinge test: section-scoped, trim-normalized (header + slug), count + forward + reverse bidirectional slug check | **PASS** |
| AC6 | Publication-safety gate documented; execution deferred to actual public flip | **PASS** |
| AC7 | `docs/ux-audit.md` covers CLI→App friction and v1.1 recommendations | **PASS** |
| Gate 2 AC1 | Live dogfooding via v1 CLI against real AI providers | **Deferred (attested)** |
| Gate 2 AC2 | Live external pilot: full cycle with multi-reviewer rotation | **Deferred (attested)** |
| Gate 2 AC3 | v1.1 Plan from live dogfooding validated as v1.1 App design input | **Deferred (attested)** |
| Gate 2 AC4 | Release archive, signed checksum, smoke-test script passes | **Deferred (release-time)** |

---

## Files Changed Since R10

| File | Change |
|---|---|
| `crates/anvil-cli/src/p11.rs` | `.trim()` applied to line and slug in table-row extraction; whitespace-tolerant comment added |
| `Anvil Plan/ANVIL_PLAN.md` | Cross-Cutting Concerns line 877 "dogfooding session" → "build observations"; Plan-Level Gate 2 expanded to 4 items matching P11 Gate 2 |
| `Review Rounds/REVIEW_P11_DOGFOODING_R10_Findings.md` | Added (reviewer's R10 findings document) |

**Commit:** `3528bc7` — "P11 R10 findings: missed dogfooding ref, Gate 2 sync, whitespace-tolerant slug extraction (R10_Findings approved)"
