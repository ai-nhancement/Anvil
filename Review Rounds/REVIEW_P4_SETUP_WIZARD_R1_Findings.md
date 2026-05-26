# Anvil — P4 Setup Wizard R1 Findings

**Source review doc:** `Review Rounds/REVIEW_P4_SETUP_WIZARD_R1.md`  
**Review date:** 2026-05-25  
**Reviewer instruction:** Critical review only. Do not write code. Findings are ordered most critical first.

## Validation Performed

- `cargo build -p anvil-cli` — **passes**
- `cargo test --workspace` — **passes**
- `cargo test -p anvil-cli -- --nocapture` — **passes**
- `cargo clippy --workspace -- -D warnings` — **fails**

Clippy failure:

```text
error: this method could have a `#[must_use]` attribute
  --> crates\anvil-core\src\diversity.rs:18:12
   |
18 |     pub fn display_name(&self) -> &str {
   |            ^^^^^^^^^^^^
   |
   = note: `-D clippy::must-use-candidate` implied by `-D warnings`
```

The P4 implementation has a useful wizard skeleton and several helpful hinge tests, but several acceptance-criterion claims in the review document are inaccurate. The most important issues are transactional safety, sidecar integration mismatch, weak Step 5 validation, and the absence of the required clean-machine walkthrough.

---

## 1. Critical — Wizard is not transactional; Step 1 and Step 6 write to disk before final Step 7 confirmation

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `crates/anvil-core/src/project.rs`
- `Review Rounds/REVIEW_P4_SETUP_WIZARD_R1.md`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The review doc claims:

> Wizard is transactional — no partial state on cancel. `commit()` called only on Step 7 confirmation.

But `run_wizard()` writes to disk long before Step 7.

Step 1 calls `step1_workspace()`, which calls `project::init(&abs)`:

```rust
let workspace_root = step1_workspace(&path)?;
```

`project::init()` creates directories, placeholder files, `audit-store/_index.json`, and `anvil.toml`:

```rust
std::fs::create_dir_all(root)?;
for dir in LAYOUT_DIRS {
    std::fs::create_dir_all(root.join(dir))?;
}
std::fs::write(&index_path, b"{\"records\":[]}\n")?;
...
save_config(root, &config)?;
```

Step 6 also creates directories before Step 7 confirmation:

```rust
step6_store(&workspace_root)?;
```

The hinge test does not exercise this path. It only constructs a `WizardState` and does not call `run_wizard()`, `step1_workspace()`, or `step6_store()`.

**Impact:**

- Cancelling after Step 1 leaves `.anvil/`, `audit-store/`, placeholder files, and `anvil.toml` behind.
- Cancelling after Step 6 can leave additional directories behind.
- This directly violates P4 acceptance criterion 2: “Cancellation at any step leaves no partial state in the workspace.”
- The review doc’s PASS claim is materially incorrect.

**Suggested fix:**

- Stage all intended filesystem changes in memory or in a temporary directory until Step 7 confirmation.
- Or explicitly split “project initialization” from “wizard transaction” and update the Plan/acceptance criteria, but that would weaken the stated requirement.
- Replace the current hinge test with one that runs realistic cancellation paths and asserts the workspace is unchanged.
- At minimum, test cancellation immediately after Step 1 and after Step 6.

---

## 2. Critical — Step 5 sidecar spawn path is incompatible with the current Go sidecar CLI

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `sidecar/cmd/anvil-sidecar/main.go`
- `Review Rounds/REVIEW_P3C_GO_SIDECAR_R1_Findings.md`

**Problem:**

P4 Step 5 spawns the sidecar with:

```rust
std::process::Command::new(&binary)
    .arg("--config")
    .arg(&tmp_config)
    .arg("--workspace")
    .arg(workspace)
```

The current Go sidecar `main.go` defines these flags:

```go
showVersion := flag.Bool("version", false, ...)
port := flag.Int("port", 0, ...)
configPath := flag.String("config", "", ...)
idleTimeout := flag.Duration("idle-timeout", 0, ...)
logLevel := flag.String("log-level", "info", ...)
```

There is no `--workspace` flag. Go’s standard `flag` package exits on unknown flags. Therefore, when the sidecar binary is found, the Step 5 spawn path will likely start a process that immediately exits with an unknown flag error.

**Impact:**

- Step 5 cannot successfully start the current sidecar binary.
- The wizard will wait for `.anvil/run/sidecar.port` until timeout.
- The review doc’s “Sidecar spawn + gRPC health probe in Step 5 — PASS” claim is not supported by the actual integration.

**Suggested fix:**

- Align the Go sidecar CLI and Rust wizard contract.
- Either add `--workspace` support to the Go sidecar or remove the argument from P4 spawn logic.
- Add an integration test that runs the actual sidecar binary or a faithful test double with the same CLI arguments.

---

## 3. Critical — Step 5 waits for `.anvil/run/sidecar.port`, but the current Go sidecar writes port files under `~/.anvil/<pid>.port`

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `sidecar/cmd/anvil-sidecar/main.go`
- `sidecar/internal/daemon/daemon.go`

**Problem:**

P4 waits for:

```rust
workspace.join(".anvil/run/sidecar.port")
```

But the current P3c sidecar writes:

```go
anvilDir := filepath.Join(homeDir, ".anvil")
pidPath := filepath.Join(anvilDir, fmt.Sprintf("%d.pid", pid))
portPath := filepath.Join(anvilDir, fmt.Sprintf("%d.port", pid))
```

So even if the sidecar starts successfully, P4 waits for a file the sidecar does not write.

**Impact:**

- Step 5 startup detection times out.
- `anvil sidecar status` and `stop` read `.anvil/run/sidecar.pid`, but the current sidecar does not populate that path.
- P4 and P3c are not yet integrated at the daemon lifecycle boundary.

**Suggested fix:**

- Resolve the P3c daemon lifecycle mismatch first.
- Ensure the sidecar writes exactly `.anvil/run/sidecar.pid` and `.anvil/run/sidecar.port` for the workspace, or update all Rust callers and Plan docs consistently.
- Add a cross-component test for sidecar spawn → port file → health probe.

---

## 4. High — Step 5 provider validation always returns success even when individual provider invokes fail

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `Review Rounds/REVIEW_P4_SETUP_WIZARD_R1.md`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The Plan says API key validation should actually invoke the provider through the sidecar and wrong keys should be detected inline. The implementation logs failures but does not propagate them.

Examples:

```rust
match client.handshake(epoch.clone()).await {
    Ok(_) => {}
    Err(e) => {
        println!("  {}: handshake failed: {e}", conn.name);
        continue;
    }
}
```

```rust
match client.reload_config(reload_req).await {
    Ok(r) if r.success => {}
    Ok(r) => {
        println!("  {}: reload_config failed: {:?}", conn.name, r.error);
        continue;
    }
    Err(e) => {
        println!("  {}: reload_config error: {e}", conn.name);
        continue;
    }
}
```

```rust
match client.invoke(req).await {
    Ok(_) => println!("  {}: OK", conn.name),
    Err(e) => println!("  {}: FAILED — {e}", conn.name),
}
```

After all of this, the function still returns:

```rust
Ok(())
```

**Impact:**

- A wrong API key can print `FAILED` but still make Step 5 return success.
- The wizard can proceed as if connectivity passed.
- This violates acceptance criteria:
  - “API key validation actually invokes the provider through the sidecar; wrong key detected inline.”
  - “Adapter connectivity test passes for valid configs, fails clearly for invalid.”

**Suggested fix:**

- Accumulate per-connection failures and return an error if any configured provider fails validation.
- Treat skipped validation due to missing key as a distinct warning or failure based on mode.
- Add tests using a mock sidecar that returns unary `AnvilError` for bad credentials.

---

## 5. High — Non-interactive/headless mode can commit setup with zero provider connections and no explicit user confirmation

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

In non-interactive mode, providers are configured only if the matching env var is present:

```rust
let configure = if !interactive {
    has_env
} else { ... };
```

If no `ANVIL_API_KEY_*` vars are present, `connections` is empty. The wizard then:

- skips diversity enforcement because required roles are missing
- skips Step 5 connectivity because there are no connections
- skips all confirmations because `interactive == false`
- commits an `anvil.toml` with no provider connections and no model bindings

The review doc treats this as acceptable:

```text
step5_connectivity returns Ok(()) when connections is empty
```

But P4 setup is supposed to configure provider connections and validate credentials. Headless mode is supposed to use env vars, not silently succeed without them.

**Impact:**

- CI/headless setup can produce a “successful” but unusable project.
- The diversity floor is bypassed because there are no reviewer bindings.
- The user receives success output even though provider setup did not occur.

**Suggested fix:**

- In headless mode, require at least the minimum provider env vars needed for the configured roles, or require an explicit `--allow-no-providers` / `--skip-providers` flag.
- Return a clear error when no provider connections are configured in headless mode.
- Add a test: headless with no env vars must not commit a completed setup.

---

## 6. High — Sidecar spawn logic lives in `anvil-cli`, contrary to the locked sidecar lifecycle choice

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `crates/anvil-cli/src/main.rs`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The locked sidecar lifecycle choice says:

```text
The spawn logic lives in anvil-core (not in CLI-specific code) so the v1.1 App can reuse it without rework.
```

Current P4 implementation puts spawn, port-file waiting, process cleanup, status, and stop behavior in `anvil-cli`:

- `step5_connectivity()`
- `find_sidecar_binary()`
- `wait_for_port_file()`
- `cmd_sidecar_status()`
- `cmd_sidecar_stop()`
- platform-specific `is_process_alive()` / `kill_process()`

**Impact:**

- The future App cannot reuse sidecar lifecycle management through `anvil-core`.
- CLI and App are likely to grow duplicate implementations.
- This violates an explicit app-compatibility design decision.

**Suggested fix:**

- Move sidecar lifecycle management into `anvil-core` or a shared crate.
- Keep CLI functions as thin wrappers around shared lifecycle APIs.
- Add tests at the shared-library layer rather than only CLI-level smoke checks.

---

## 7. High — Clean Windows machine walkthrough is marked deferred even though the Plan says it is a P4 ship gate

**Location:**

- `Review Rounds/REVIEW_P4_SETUP_WIZARD_R1.md`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The review doc says:

```text
Clean-machine walkthrough by non-author reviewer — DEFERRED
```

and later:

```text
This does not block the P4 commit; it is a documentation artifact that can be added in a follow-up.
```

But the Plan says:

```text
A clean walkthrough is a P4 ship gate — the phase does not ship until at least one reviewer-walkthrough document exists for the primary platform.
```

**Impact:**

- The review doc’s verdict contradicts the Plan.
- P4 cannot be considered complete or shippable until `docs/p4-walkthrough.md` exists and records the non-author clean Windows walkthrough.

**Suggested fix:**

- Treat the walkthrough as blocking for P4 approval.
- Add `docs/p4-walkthrough.md` from a non-author reviewer on a clean Windows machine.
- If this is intentionally deferred, amend the Plan explicitly before approval.

---

## 8. Medium / High — `cargo clippy --workspace -- -D warnings` fails

**Location:**

- `crates/anvil-core/src/diversity.rs`

**Problem:**

Clippy fails under the workspace lint policy:

```text
error: this method could have a `#[must_use]` attribute
  --> crates\anvil-core\src\diversity.rs:18:12
```

The method is:

```rust
pub fn display_name(&self) -> &str {
```

The R1 validation section does not mention clippy, but prior phases have used `cargo clippy --workspace -- -D warnings` as a quality gate.

**Impact:**

- CI or local quality gates that include clippy will fail.
- The implementation is not warning-clean under the configured workspace lint strictness.

**Suggested fix:**

- Add `#[must_use]` to `display_name()` or otherwise address the lint.
- Include clippy in P4 validation before declaring PASS.

---

## 9. Medium — Interactive mode prefers prompting for keychain credentials even when an env var is already present

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `Anvil Plan/PLAN_HARDENING_HISTORY.md`

**Problem:**

The Plan hardening history says env vars should be detected before the wizard step, and if set, the interactive prompt is skipped for that vendor.

Current behavior uses an env var without prompting only when either:

```rust
has_env && (!interactive || credential_mode == CredentialMode::EnvVarOnly)
```

In interactive mode with a working keychain and an env var already present, the wizard still prompts for an API key and chooses keychain storage.

**Impact:**

- Users who intentionally supplied `ANVIL_API_KEY_*` still get prompted.
- This contradicts the documented env-var bypass behavior.
- It may accidentally move a session-only env-var workflow into persistent keychain storage.

**Suggested fix:**

- If an env var is present, offer a clear choice or default to using it without prompting as the Plan states.
- Add a test for interactive-mode env-var bypass logic separately from terminal prompts.

---

## 10. Medium — `anvil sidecar status` does not probe the Health RPC despite the sidecar lifecycle contract

**Location:**

- `crates/anvil-cli/src/main.rs`
- `crates/anvil-sidecar-client/src/client.rs`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The locked sidecar lifecycle says subsequent CLI invocations read PID/port files and probe the `Health` RPC.

`cmd_sidecar_status()` only checks whether the PID appears alive:

```rust
if is_process_alive(pid) {
    print!("Sidecar: running (PID {pid}");
```

It parses the port file but does not connect to the sidecar and call `probe_health()`.

**Impact:**

- `status` can report “running” for a process that is not a healthy sidecar.
- A stale or wrong process with the same PID can be misreported.
- The new `probe_health()` API is not used by `status`, despite its doc comment mentioning that use case.

**Suggested fix:**

- Connect to `127.0.0.1:<port>` and call `probe_health()` when a port file exists.
- Report separately: PID alive, port present, health healthy/unhealthy.
- Add tests around stale PID, missing port, and health failure cases.

---

## 11. Medium — `anvil sidecar stop` does not remove stale PID/port files

**Location:**

- `crates/anvil-cli/src/main.rs`

**Problem:**

If `cmd_sidecar_status()` detects a stale PID file, it tells the user:

```rust
Run `anvil sidecar stop` to clean up, or delete the file manually.
```

But `cmd_sidecar_stop()` does not remove the PID file or port file. It reads the PID and sends a kill signal:

```rust
kill_process(pid);
println!("Stop signal sent to sidecar (PID {pid}).");
```

It leaves `.anvil/run/sidecar.pid` and `.anvil/run/sidecar.port` in place.

**Impact:**

- The recommended cleanup command does not actually clean up stale files.
- Subsequent `status` calls can continue to report stale state.

**Suggested fix:**

- If the process is not alive, remove stale PID/port files.
- After sending stop, wait briefly and then remove files if the process exits.
- Add tests for stale cleanup behavior.

---

## 12. Medium — `sidecar.binary_path` configuration is ignored by Step 5

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `crates/anvil-core/src/config.rs`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The Plan says sidecar binary location can come from `$PATH` or `sidecar.binary_path` in `anvil.toml`.

`find_sidecar_binary()` supports a configured path parameter:

```rust
fn find_sidecar_binary(configured: Option<&Path>) -> Result<PathBuf, AnvilError>
```

But Step 5 always calls it with `None`:

```rust
let binary = find_sidecar_binary(None)?;
```

**Impact:**

- Users who configured `sidecar.binary_path` cannot use it during setup connectivity checks.
- Clean-machine setup is more likely to fail unless the binary is already on PATH.

**Suggested fix:**

- Load existing config after Step 1 and pass `config.sidecar.binary_path.as_deref()` into `find_sidecar_binary()`.
- Add a test for configured sidecar binary path resolution.

---

## 13. Medium — The workspace lock is only a directory, not an actual concurrency lock

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `crates/anvil-core/src/project.rs`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The hinge test is named:

```rust
test_workspace_lock_enforced
```

but it only asserts that `.anvil/run` exists in `LAYOUT_DIRS`:

```rust
assert!(LAYOUT_DIRS.contains(&".anvil/run"))
```

There is no actual file lock or process lock implementation shown.

**Impact:**

- Two `anvil setup` processes can likely write the same workspace concurrently.
- The hinge test name overstates what is enforced.
- This does not satisfy the Plan’s same-workspace concurrency protection requirement.

**Suggested fix:**

- Implement an actual workspace lock file under `.anvil/run` using an OS-level exclusive lock or create-new semantics.
- Acquire it around setup and other write commands.
- Replace the current hinge with a real concurrency/exclusive-lock test.

---

## 14. Medium — Amendment A1 choices are written only as audit records, not locked into `anvil.toml` Required Choices

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `crates/anvil-core/src/config.rs`
- `crates/anvil-core/src/choices.rs`

**Problem:**

The review doc says Amendment A1 choices are “captured and locked.” The implementation writes `ProvisionalLock` audit records:

```rust
ProvisionalLock::new("governance_model".to_owned(), ...)
ProvisionalLock::new("trademark_posture".to_owned(), ...)
ProvisionalLock::new("security_disclosure_contact".to_owned(), ...)
```

But it does not update `config.choices` or otherwise lock these values in `anvil.toml`.

**Impact:**

- The choices may be visible in the audit store but not enforced by the config gate.
- `anvil config show` may not reflect them as locked Required Choices.
- The “captured and locked” claim may be stronger than the implementation.

**Suggested fix:**

- Add these choices to the Required Choices schema if they are truly project-level required choices.
- Update `anvil.toml` and audit records together during commit.
- Add tests that `load_config()` after setup contains the A1 choices with locked state.

---

## 15. Medium — Hinge tests are shallow and miss the actual behaviors claimed in the review

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `crates/anvil-core/src/diversity.rs`

**Problem:**

Several hinge tests pin constants or structural facts but not behavior:

- `test_wizard_cancellation_leaves_no_partial_state` does not run cancellation paths.
- `test_api_keys_env_var_bypass_works_headless` only checks env-var constant names.
- `test_workspace_lock_enforced` only checks directory existence.
- `test_api_keys_encrypted_at_rest` serializes one `ProviderConnection`, but does not run `commit()` or inspect a real `anvil.toml` after setup.

**Impact:**

- The tests pass while major acceptance criteria are unmet.
- The review doc overstates the degree of behavioral validation.

**Suggested fix:**

- Add behavior-level tests for setup cancellation, headless setup, keychain/env-var config serialization, sidecar path resolution, and status/stop behavior.
- Use mocks/fakes for keyring and sidecar where direct integration is too expensive.

---

## 16. Low / Medium — Operational hinge pins still use exact equality despite the Plan’s brittle-pin convention

**Location:**

- `crates/anvil-cli/src/setup.rs`
- `Anvil Plan/ANVIL_PLAN.md`

**Problem:**

The Plan’s hinge convention says operational pins like wizard step count should become minimum-style assertions rather than exact equality.

Current test uses exact equality:

```rust
assert_eq!(WIZARD_STEPS, 7, "wizard must have exactly 7 steps");
```

**Impact:**

- Legitimate additions to the wizard require synchronized test/prose edits.
- This contradicts the Plan’s own brittle-pin guidance.

**Suggested fix:**

- If P4 is intentionally exactly seven steps for v1, document why this is a constitutional/exact pin.
- Otherwise convert to a minimum-style hinge once P10b machinery exists, and avoid wording that forbids future wizard growth.

---

## 17. Low — Unused dependencies were added to `anvil-cli`

**Location:**

- `crates/anvil-cli/Cargo.toml`

**Problem:**

The review doc says these were added:

```text
chrono, serde, serde_json, sha2, toml, uuid, tokio, keyring, dialoguer, anvil-sidecar-client
```

Some are clearly used, but `chrono` does not appear to be used in `anvil-cli/src/main.rs` or `setup.rs`.

**Impact:**

- Minor dependency bloat.
- Increases compile surface and supply-chain footprint unnecessarily.

**Suggested fix:**

- Remove unused dependencies or move them to crates that actually use them.
- Use `cargo machete` or equivalent in a later cleanup pass if available.

---

## Overall Assessment

I would **not approve P4 R1 as complete**. The implementation compiles and tests pass, but the tests do not cover the most important acceptance criteria. The review doc’s PASS verdict is premature.

Blocking or near-blocking issues:

1. The wizard is not transactional; it writes project state before Step 7 confirmation.
2. Step 5 is incompatible with the current Go sidecar CLI (`--workspace`) and port-file behavior.
3. Step 5 reports success even when provider validation fails.
4. Headless setup can silently commit with zero providers.
5. Sidecar lifecycle logic lives in `anvil-cli`, contrary to the locked `anvil-core` reuse decision.
6. The required clean Windows walkthrough is missing even though the Plan calls it a P4 ship gate.
7. `cargo clippy --workspace -- -D warnings` fails.

Minimum recommended before approval:

1. Make setup genuinely transactional or amend the Plan’s cancellation guarantee.
2. Align P4 sidecar spawn/status behavior with the actual P3c sidecar lifecycle.
3. Make Step 5 return failure on failed provider validation.
4. Require explicit no-provider mode for headless setup, or fail when no env vars are present.
5. Move shared sidecar lifecycle logic into `anvil-core`.
6. Add the clean Windows walkthrough artifact.
7. Fix clippy and add behavior-level tests for the claimed hinge properties.