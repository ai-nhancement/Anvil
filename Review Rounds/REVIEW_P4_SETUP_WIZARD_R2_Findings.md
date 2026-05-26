# Anvil — P4 Setup Wizard R2 Findings

**Source review doc:** `Review Rounds/REVIEW_P4_SETUP_WIZARD_R2.md`  
**Review date:** 2026-05-25  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo build --workspace` — **passes**
- `cargo test --workspace` — **passes**
- `cargo clippy --workspace -- -D warnings` — **passes**
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — **fails**
- `go test ./...` from `C:\Anvil\sidecar` — **passes**
- `cargo fmt --check` — **fails**

Additional note: I accidentally ran `gofmt -w -l .` in `C:\Anvil\sidecar` while checking Go formatting. It changed only import/spacing formatting in four adapter files; I immediately reverted those changes with `git checkout -- ...`. No unintended file edits remain from that command.

---

## 1. High — CI clippy still fails under the repository’s stricter configured command

**Location:**

- `.github/workflows/ci.yml`
- `justfile`
- `crates/anvil-core/src/choices.rs`
- `Review Rounds/REVIEW_P4_SETUP_WIZARD_R2.md`

**Problem:**

R2 validates:

```text
cargo clippy --workspace -- -D warnings
```

That command passes. However, the repository CI and `just lint` use the stricter form:

```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

That command fails:

```text
error: this pattern creates a reference to a reference
   --> crates\anvil-core\src\choices.rs:209:69
    |
209 |                 crate::error::AnvilError::ProvisionalMissingField { ref field, .. }
    |                                                                     ^^^^^^^^^
    |
    = note: `-D clippy::needless-borrow` implied by `-D warnings`

error: this pattern creates a reference to a reference
   --> crates\anvil-core\src\choices.rs:228:69
    |
228 |                 crate::error::AnvilError::ProvisionalMissingField { ref field, .. }
    |                                                                     ^^^^^^^^^
```

The failing code is in test targets, which explains why the narrower clippy command passed.

**Impact:**

- CI will fail even though the R2 review document reports clippy clean.
- P4 cannot be considered fully validation-clean under the repository’s own workflow.

**Suggested fix:**

- Fix the two `ref field` patterns in `crates/anvil-core/src/choices.rs` test code as clippy suggests.
- Update review validation to use the same command as CI:
  ```text
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  ```

---

## 2. High — Rust formatting check fails

**Location:**

- Rust workspace formatting, likely including newly edited P4 files
- `.github/workflows/ci.yml`

**Problem:**

`cargo fmt --check` fails. The command output did not include a detailed diff in this environment, but the exit code is non-zero.

The CI workflow includes a Rust format check, so this is a gate failure independent of build/test/clippy.

**Impact:**

- CI will fail on formatting.
- The R2 verdict’s “P4 is approved for commit” is premature unless formatting is fixed or the CI gate is changed.

**Suggested fix:**

- Run `cargo fmt` and commit the formatting changes.
- Re-run `cargo fmt --check` before approval.

---

## 3. High — `docs/p4-walkthrough.md` does not satisfy the clean-machine/non-author walkthrough requirement

**Location:**

- `docs/p4-walkthrough.md`
- `Anvil Plan/ANVIL_PLAN.md`
- `Review Rounds/REVIEW_P4_SETUP_WIZARD_R2.md`

**Problem:**

The Plan requires:

```text
A non-author reviewer (someone other than the Coder) walks the wizard end-to-end on a clean Windows machine with: no prior Anvil install, no prior sidecar daemon running, no existing ~/.anvil/global-registry.json, and no existing keychain entries for ANVIL_* credentials.
```

The walkthrough says:

```markdown
**Reviewer:** project Coordinator (non-author clean-machine walkthrough required by Plan)
```

This is ambiguous at best. The Plan specifically says “non-author reviewer,” not the Coordinator. If the Coordinator authored or directed the P4 implementation, this does not meet the stated independence requirement.

The walkthrough also does not document all clean-machine preconditions. It states:

```markdown
- No prior `anvil` binary in `$PATH` (clean machine)
- No existing project directory at the test path
- Test environment variables NOT set
```

But it does not show verification of:

- no existing `~/.anvil/global-registry.json`
- no prior sidecar daemon running
- no existing keychain entries for `ANVIL_*` credentials

In fact, the transcript includes an env-var prompt despite the prerequisites saying env vars are not set:

```text
ANVIL_API_KEY_ANTHROPIC is already set — use env var instead of storing in keychain? [Y/n]: n
```

**Impact:**

- The P4 ship-gate walkthrough is not demonstrably satisfied.
- R2’s “Clean Windows 11 walkthrough — PASS” claim is over-stated.
- The walkthrough may be a useful smoke-test note, but it is not the Plan-required independent clean-machine evidence.

**Suggested fix:**

- Have a genuinely non-author reviewer run the walkthrough.
- Add explicit preflight evidence for no prior daemon, no global registry, and no relevant keychain entries.
- Resolve the contradiction between “env vars NOT set” and the transcript showing `ANVIL_API_KEY_ANTHROPIC` set.
- Include timestamps or terminal excerpts per the Plan’s requested structured document.

---

## 4. High — Step 5 connectivity can still be skipped in interactive mode even though P4 acceptance says failing adapters should block

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `docs/p4-walkthrough.md`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

R2 fixes `connectivity_checks()` so failed invokes return `Err`. That is good. But `run_wizard()` still allows an interactive user to continue past any Step 5 failure:

```rust
let sidecar_ok = step5_connectivity(...);
match &sidecar_ok {
    Ok(()) => println!("  Connectivity: OK"),
    Err(e) => {
        println!("  WARNING: connectivity test failed or skipped: {e}");
        println!("  You can re-run Step 5 later with `anvil setup --step 5`.");
        if interactive {
            let proceed = Confirm::new()
                .with_prompt("Continue setup despite connectivity failure?")
                .default(false)
                .interact()
                .unwrap_or(false);
            if !proceed {
                return Err(AnvilError::SetupCancelled);
            }
        }
    }
}
```

The walkthrough explicitly uses this bypass:

```text
anvil-sidecar binary not found — skipping connectivity test.
Continue setup despite connectivity failure? [y/N]: Y
```

But the Plan says:

```text
Failures are explicit: `anvil setup` does not continue past Step 5 with a failing adapter.
```

and acceptance says:

```text
Adapter connectivity test passes for valid configs, fails clearly for invalid.
```

**Impact:**

- A project can be committed even when the sidecar is missing or provider validation failed.
- The walkthrough’s PASS does not validate actual sidecar spawn, health, or provider invoke.
- Wrong API keys may still become committed if the user chooses to continue despite failure.

**Suggested fix:**

- Decide whether Step 5 is advisory or blocking. The Plan currently says blocking for failing adapters.
- If blocking, remove the “continue despite connectivity failure” bypass for configured providers.
- If advisory, amend the Plan acceptance criteria and walkthrough verdict to say Step 5 can be skipped.
- At minimum, distinguish “sidecar binary absent on clean install” from “configured provider failed validation” and only allow bypass for the former if that is intentional.

---

## 5. Medium / High — `anvil setup --step 5` is advertised but no such CLI option exists

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `crates/anvil-cli/src/main.rs`

**Problem:**

On Step 5 failure, the wizard prints:

```rust
println!("  You can re-run Step 5 later with `anvil setup --step 5`.");
```

But the CLI defines `Setup` as:

```rust
Setup {
    #[arg(default_value = ".")]
    path: PathBuf,
}
```

There is no `--step` argument.

**Impact:**

- The user is instructed to run a command that does not exist.
- This is especially harmful because R2’s walkthrough depends on Step 5 being skipped and supposedly rerunnable later.

**Suggested fix:**

- Either implement `anvil setup --step 5` or remove/change the message.
- If partial setup reruns are part of P4 acceptance, implement and test step-level rerun support.

---

## 6. Medium / High — `commit()` can leave partial persistent state if keychain write fails after `project::init()`

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `crates/anvil-core/src/project.rs`

**Problem:**

R2 moves `project::init()` into `commit()`, which fixes pre-confirmation writes. However, `commit()` is still not atomic/transactional.

Order of operations:

```rust
project::init(root)?;
// then keychain writes
for conn in &state.connections {
    entry.set_password(key)?;
}
// then config update
save_config(root, &config)?;
// then audit records
store.append(...)?;
```

If any keychain write fails after `project::init()`, the workspace layout and default `anvil.toml` already exist, but provider config and audit records may not. If `save_config()` or an audit append fails later, keychain entries may already exist while config/audit state is incomplete.

**Impact:**

- The wizard still has partial-state failure modes after final confirmation.
- The R2 statement “Changes are committed atomically” is not accurate.
- This is less severe than R1’s cancellation bug, but it is still a transactional correctness issue.

**Suggested fix:**

- Define the expected atomicity boundary: cancellation-only or all commit failures.
- If full atomicity is required, stage project layout/config/audit writes and commit in a recoverable order, with rollback/cleanup on failure.
- At minimum, preflight keychain availability for all keychain credentials before calling `project::init()` and document non-atomic commit failure behavior.

---

## 7. Medium — Sidecar lifecycle is only partially moved into `anvil-core`

**Location:**

- `crates/anvil-core/src/sidecar.rs`
- `crates/anvil-cli/src/setup.rs`
- `crates/anvil-cli/src/main.rs`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

R2 moves helper functions to `anvil-core`:

- `find_sidecar_binary`
- `wait_for_port_file`
- `is_process_alive`
- `kill_process`

But higher-level lifecycle behavior remains in `anvil-cli`:

- spawning the sidecar process and passing flags
- Step 5 cleanup of PID/port files
- health probing helper `probe_health_sync()`
- `cmd_sidecar_status()` behavior
- `cmd_sidecar_stop()` behavior

The Plan’s locked choice says:

```text
The spawn logic lives in anvil-core (not in CLI-specific code) so the v1.1 App can reuse it without rework.
```

The actual spawn logic remains in `setup.rs`:

```rust
let mut child = std::process::Command::new(&binary)
    .arg("--config")
    .arg(&tmp_config)
    .arg("--workspace")
    .arg(workspace)
    ...
    .spawn()?;
```

**Impact:**

- The future App still cannot reuse full sidecar lifecycle management from `anvil-core`.
- R2’s “Sidecar lifecycle in `anvil-core` — PASS” is overstated.

**Suggested fix:**

- Move full daemon start/probe/stop/status semantics into `anvil-core`, not just low-level helpers.
- Expose a reusable `SidecarManager` or equivalent from `anvil-core`.
- Keep CLI as a thin presentation layer.

---

## 8. Medium — `sidecar status` still lacks global-registry behavior required by the Plan

**Location:**

- `crates/anvil-cli/src/main.rs`
- `sidecar/cmd/anvil-sidecar/main.go`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

P3c R2 appears to add global registry support in the Go sidecar. However, P4 CLI management still only supports:

```rust
enum SidecarCmd {
    Status { project: PathBuf },
    Stop { project: PathBuf },
}
```

The Plan also requires:

- every CLI invocation touching sidecar layer sweeps `~/.anvil/global-registry.json`
- `anvil sidecar status --all`
- `anvil sidecar kill --stale`
- `anvil sidecar kill --workspace <path>`

None of those CLI surfaces are present in P4 R2.

**Impact:**

- P4/P3c daemon management remains incomplete relative to the current Plan.
- Users cannot inspect or clean stale daemons across workspaces from the CLI.

**Suggested fix:**

- Add global registry parsing and stale sweep logic to the shared sidecar lifecycle layer.
- Add `status --all` and `kill` subcommands, or explicitly defer them with a Plan update if they are not P4 scope.

---

## 9. Medium — Walkthrough Step 5 did not validate sidecar spawn, health probe, or provider invoke

**Location:**

- `docs/p4-walkthrough.md`
- `Review Rounds/REVIEW_P4_SETUP_WIZARD_R2.md`

**Problem:**

The walkthrough’s Step 5 says:

```text
anvil-sidecar binary not found — skipping connectivity test.
```

and the final limitation says:

```text
Step 5 connectivity test was skipped because `anvil-sidecar` binary was not on `$PATH`.
Full end-to-end connectivity ... requires the Go sidecar binary to be installed, which is addressed in a subsequent integration phase.
```

But the R2 validation table says:

```text
Step 5 returns Err on provider failure | PASS
Go sidecar --workspace compatibility | PASS
```

Those are code-path inspections or assumptions, not walkthrough validation. The clean-machine walkthrough did not prove the most integration-heavy part of P4.

**Impact:**

- The highest-risk integration point remains unproven by the walkthrough.
- P4’s acceptance criterion “Sidecar spawn + gRPC health probe in Step 5” is not satisfied by the documented run.

**Suggested fix:**

- Install/build `anvil-sidecar` on the walkthrough machine and put it on PATH, or configure `sidecar.binary_path`.
- Run Step 5 through sidecar spawn, port-file detection, Health RPC, handshake/reload, and at least one provider invoke.
- If real vendor invokes are not possible, use a local test provider/mock sidecar and document the limitation clearly.

---

## 10. Medium — Headless no-provider guard is not covered by a behavior test

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `Review Rounds/REVIEW_P4_SETUP_WIZARD_R2.md`

**Problem:**

R2 adds a code guard for headless mode with zero providers:

```rust
if !interactive && connections.is_empty() {
    return Err(AnvilError::SetupCancelled);
}
```

But R2 explicitly defers the behavior-level test:

```text
Behavior-level tests for headless no-providers ... are deferred to a test infrastructure phase.
```

This case does not require a mock sidecar or mock keyring if factored appropriately; it is a pure control-flow path.

**Impact:**

- A previous blocking issue is fixed only by inspection, not regression-tested.
- Future edits could reintroduce silent headless success.

**Suggested fix:**

- Add a behavior-level unit or integration test for headless no-provider failure.
- Consider splitting wizard planning from terminal I/O to make headless behavior easy to test.

---

## 11. Low / Medium — `cmd_sidecar_stop` removes PID/port files after a fixed sleep without verifying exit

**Location:**

- `crates/anvil-cli/src/main.rs`

**Problem:**

R2 improves stale cleanup, but for a live process it now does:

```rust
kill_process(pid);
std::thread::sleep(std::time::Duration::from_millis(500));
...
remove_file(pid_path);
remove_file(port_path);
```

It does not re-check whether the process exited. If the kill signal fails, is ignored, or the process takes longer than 500ms to exit, the CLI removes discovery files while the sidecar may still be running.

**Impact:**

- A live sidecar can become orphaned from the workspace PID/port files.
- Later `status` reports “not running” while a process may still exist.

**Suggested fix:**

- Poll process liveness for a bounded grace period.
- Only remove files once the process exits, or mark as forced/stale if not.
- On Windows, consider taskkill failure status and report errors.

---

## 12. Low — Walkthrough contains several consistency issues that reduce audit value

**Location:**

- `docs/p4-walkthrough.md`

**Problem:**

The walkthrough has multiple internal inconsistencies:

- Prerequisite says env vars are not set, but transcript says `ANVIL_API_KEY_ANTHROPIC is already set`.
- It says “no prior Anvil install,” but Step 0 builds from `C:\Anvil` and adds `target\release` to PATH. This may be acceptable, but it should be described as a source checkout build rather than no install.
- It says `anvil init` creates “8 directories,” but current `LAYOUT_DIRS` contains 18 entries. This may be copied output or outdated expectation.
- The Step 3 selection transcript uses numeric `0`/`1` for `dialoguer::Select`; actual prompts usually render an interactive selection UI rather than accepting/printing those values literally.

**Impact:**

- The walkthrough is less reliable as an audit artifact.
- It is harder for a future reviewer to reproduce exactly what happened.

**Suggested fix:**

- Clean up the walkthrough transcript so prerequisites and transcript agree.
- Include exact terminal excerpts where possible.
- Make expected outputs match current implementation.

---

## Overall Assessment

R2 fixes several important R1 issues: pre-confirmation project initialization has been moved into `commit()`, Step 5 provider failures are propagated from `connectivity_checks()`, headless zero-provider mode now has a guard, the narrow clippy command passes, and P3c sidecar CLI/path compatibility appears to have been addressed.

However, I would **not approve P4 R2 yet** because repository-level validation still fails and the P4 ship-gate walkthrough is not strong enough:

1. CI-equivalent clippy fails under `--all-targets --all-features`.
2. `cargo fmt --check` fails.
3. The walkthrough does not clearly satisfy the Plan’s non-author clean-machine requirement and did not execute Step 5 connectivity.
4. Interactive setup can still bypass Step 5 failures even though the Plan describes failing adapters as blocking.
5. The wizard advertises `anvil setup --step 5`, but the CLI has no `--step` option.

Minimum recommended before approval:

1. Fix `cargo fmt --check` and CI-equivalent clippy.
2. Correct or redo `docs/p4-walkthrough.md` with a non-author reviewer and explicit clean-machine preflight evidence.
3. Decide and align whether Step 5 is blocking or advisory; update code or Plan accordingly.
4. Implement or remove the advertised `anvil setup --step 5` rerun path.
5. Add at least one behavior test for the headless no-provider guard.