# Anvil — P9 Ship + Rollback R1 Findings

**Source review doc:** `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R1.md`  
**Review date:** 2026-05-26  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo fmt --all -- --check` — **Fail** (multiple formatting diffs in `main.rs`, `phase.rs`, `ship.rs`, `rollback.rs`, `transport.rs`, `lib.rs`)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **Fail** (one error in `crates/anvil-ship/src/rollback.rs:164`)
- `cargo test --workspace` — **Pass** (152 tests)

The R1 document claims "Tests: 152 passing … clippy clean, fmt clean" but the current tree does not match that state.

---

## 1. High — Validation not clean; R1 claims contradicted by actual state

**Location:**

- `crates/anvil-ship/src/rollback.rs:164` (clippy `redundant_closure`)
- Multiple files with `cargo fmt` diffs
- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R1.md` (reproducibility section)

**Problem:**

The self-review states clean fmt/clippy and 152 tests. The tree currently has one clippy error and many formatting violations. The hinge tests and new `anvil-ship` crate have not been run through the full CI-equivalent command before the R1 disposition.

**Impact:**

- The "P9 R1 complete" bottom-line claim is premature.
- A future reviewer or CI run will immediately fail on the new crate.

**Suggested fix:**

- Run `cargo fmt --all` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` locally and commit the resulting clean state before declaring R1 complete.

---

## 2. High — `run_phase_reopen` lacks non-interactive `--yes` / `--reason` path (explicitly called out in the review doc)

**Location:**

- `crates/anvil-cli/src/phase.rs:936` (`run_phase_reopen`)
- `Review Rounds/REVIEW_P9_SHIP_ROLLBACK_R1.md` (question 5)

**Problem:**

The Plan cross-cutting requirement ("every gate must support a non-interactive `--yes` path") is violated. `dialoguer::Confirm` with `default(false)` blocks in CI / scripted environments. No `--yes` or `--reason` flag exists on `phase reopen`.

**Impact:**

- P9 cannot be used in automated release pipelines or headless Coordinator workflows.
- The review document itself flags this as a potential High/Medium gap.

**Suggested fix:**

- Add `--yes` / `-y` and optional `--reason <text>` flags to `PhaseCmd::Reopen`.
- When `--yes` is present, skip the Confirm prompt and use the provided (or default) reason.

---

## 3. Medium — Timestamp comparison direction `rollback_at > ship_at` is correct but fragile under sub-second races (review question 1)

**Location:**

- `crates/anvil-ship/src/ship.rs:120` (`is_phase_currently_shipped`)

**Problem:**

The predicate `rollback_at > ship_at` (strictly greater) is used. While chrono sub-second precision makes same-millisecond collision vanishingly unlikely in real workflows, the comparison direction is the only thing preventing a ship/rollback pair created in the same instant from being misclassified.

**Impact:**

- Low practical risk, but the predicate is not robust against any future change that coarsens timestamp granularity.
- The review document explicitly asks for confirmation of the direction.

**Suggested fix:**

- Document the invariant: "RollbackEvent.created_at is always strictly after the corresponding GateApproval.created_at because the rollback command runs after the ship gate is written." Add a comment at the comparison site.

---

## 4. Low — Redundant sequential calls to `check_all_phases_shipped` then `check_unresolved_rollbacks` (review question 3)

**Location:**

- `crates/anvil-cli/src/ship.rs:70`

**Problem:**

`check_unresolved_rollbacks` is a strict subset of the phases that would already fail `check_all_phases_shipped`. The second check is only reached if the first passes, making its error path unreachable in the current call order.

**Impact:**

- Minor dead code in the error surface.
- The distinction in error messages ("unresolved rollbacks" vs. "not all phases shipped") may still be valuable for diagnostics.

**Suggested fix:**

- Either merge the two checks into a single richer readiness report or reverse the call order so the more specific "unresolved rollback" message can be surfaced when appropriate.

---

## 5. Low — `cmd /C` quoting for complex transport commands on Windows not tested (review question 4)

**Location:**

- `crates/anvil-ship/src/transport.rs:60` (`run_shell_command` on Windows)

**Problem:**

The test only uses the portable no-op `cd .`. Commands containing quotes or spaces (e.g., `git commit -m "message with spaces"`) may be mishandled by the current `cmd /C` invocation.

**Impact:**

- Low for the current test suite; potential runtime failure for realistic transport actions on Windows.

**Suggested fix:**

- Add a Windows-specific test that exercises a command with embedded quotes and asserts correct exit status.

---

## Overall Assessment

P9 R1 introduces a substantial new crate (`anvil-ship`) with clean architectural decisions (timestamp recency, epoch-based rotation reset, `TransportAction` placement to avoid cycles). The 17 new tests and three hinge tests cover the core rollback plan, immutability, and rotation reset semantics.

However, the self-review is inconsistent with the current tree:

- fmt and clippy are not clean.
- The non-interactive path gap (question 5) is a real Plan violation for a ship/rollback gate.

The four review questions in the document are reasonable; only the timestamp direction (Q1) and redundant checks (Q3) have minor robustness notes. The `--yes` gap (Q5) is the only item that should be treated as High for v1.

P9 should not be considered R1-complete until `cargo fmt` + full clippy pass and a non-interactive path for `phase reopen` are present. Once those two items are resolved, the rollback and project-ship machinery is ready for independent review.