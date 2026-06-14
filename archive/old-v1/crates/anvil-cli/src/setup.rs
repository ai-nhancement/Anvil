//! `anvil setup` — interactive seven-step project wizard.
//!
//! Each step accumulates state in [`WizardState`] without writing to disk.
//! Changes are committed atomically (written to disk + audit records) only after
//! the user confirms in Step 7. Cancelling at any point leaves no partial state:
//! Step 1 creates only the workspace root directory; full project layout
//! (`project::init`) runs inside `commit()`.

use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anvil_audit::{
    records::{GateApproval, ProvisionalLock},
    AuditStore,
};
use anvil_core::{
    config::{
        load_config, save_config, AnvilConfig, CredentialRef, ModelBinding, ProviderConnection,
        ProviderType,
    },
    diversity::validate_diversity,
    error::AnvilError,
    project,
};
use dialoguer::{Confirm, Input, Password, Select};

/// Number of wizard steps — pinned by [`test_wizard_step_count`].
pub const WIZARD_STEPS: usize = 7;

/// Environment variable names for headless/CI credential supply.
pub const ENV_ANTHROPIC: &str = "ANVIL_API_KEY_ANTHROPIC";
pub const ENV_OPENAI: &str = "ANVIL_API_KEY_OPENAI";
pub const ENV_GOOGLE: &str = "ANVIL_API_KEY_GOOGLE";

/// Default model IDs used for the Step 5 connectivity test.
const DEFAULT_MODEL_ANTHROPIC: &str = "claude-haiku-4-5-20251001";
const DEFAULT_MODEL_OPENAI: &str = "gpt-4o-mini";
const DEFAULT_MODEL_GOOGLE: &str = "gemini-1.5-flash";

/// Keyring service name (Windows Credential Manager / macOS Keychain).
pub(crate) const KEYRING_SERVICE: &str = "anvil";

// ── Role names ────────────────────────────────────────────────────────────────

pub const ROLE_CODER: &str = "coder";
pub const ROLE_INTERLOCUTOR: &str = "interlocutor";
pub const ROLE_PLANNER: &str = "planner";
pub const ROLE_REVIEWER_1: &str = "reviewer-1";
pub const ROLE_REVIEWER_2: &str = "reviewer-2";

// ── Credential mode ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialMode {
    /// OS keychain is available and in use.
    Keychain,
    /// Keychain unavailable; all credentials must come from environment variables.
    EnvVarOnly,
}

// ── Per-connection credential (held in memory during wizard) ──────────────────

#[derive(Debug, Clone)]
pub struct WizardCredential {
    /// The actual API key — held in memory, written to keychain at commit.
    /// `None` when the credential comes from an env var (key is never extracted).
    pub api_key: Option<String>,
    /// Which env var name to record if not using keychain.
    pub env_var_name: Option<String>,
    /// How this credential will be stored in `anvil.toml`.
    pub credential_ref: CredentialRef,
}

// ── Connection entry during wizard ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WizardConnection {
    pub name: String,
    pub provider_type: ProviderType,
    pub endpoint: Option<String>,
    pub credential: WizardCredential,
    /// Model ID used in Step 5 connectivity test.
    pub test_model: String,
}

// ── Wizard state (transactional; committed only at end of Step 7) ─────────────

pub struct WizardState {
    pub workspace_root: PathBuf,
    pub connections: Vec<WizardConnection>,
    pub model_bindings: Vec<ModelBinding>,
    pub credential_mode: CredentialMode,
    // Amendment A1 fields
    pub governance_model: String,
    pub trademark_posture: String,
    pub security_disclosure_contact: String,
}

// ── Main wizard entry point ───────────────────────────────────────────────────

/// Run the interactive setup wizard. Returns `Ok(())` when setup completes
/// successfully (changes committed) or the user explicitly skips.
/// Returns `Err(AnvilError::SetupCancelled)` if the user aborts mid-wizard.
#[allow(clippy::too_many_lines)]
pub fn run_wizard(path: &Path) -> Result<(), AnvilError> {
    let interactive = std::io::stdin().is_terminal();

    println!();
    println!("=== Anvil Setup Wizard ({WIZARD_STEPS} steps) ===");
    if !interactive {
        println!("  Non-interactive mode: credentials from environment variables.");
    }
    println!();

    // ── Step 1 ────────────────────────────────────────────────────────────────
    print_step(1, "Workspace root selection");
    let workspace_root = step1_workspace(path)?;
    println!("  Workspace: {}", workspace_root.display());
    confirm_step(1, interactive)?;

    // ── Step 2 ────────────────────────────────────────────────────────────────
    print_step(2, "Provider connections");
    let (connections, credential_mode) = step2_providers(interactive);

    // In headless mode, require at least one provider connection.
    if !interactive && connections.is_empty() {
        eprintln!("error: headless setup configured no provider connections.");
        eprintln!("  Set at least one of: {ENV_ANTHROPIC}, {ENV_OPENAI}, {ENV_GOOGLE}");
        return Err(AnvilError::SetupCancelled);
    }

    if connections.is_empty() {
        println!("  No provider connections configured.");
        println!("  You can add connections later by re-running `anvil setup`.");
    } else {
        println!("  {} connection(s) configured.", connections.len());
        for c in &connections {
            println!("    - {} ({:?})", c.name, c.provider_type);
        }
    }
    confirm_step(2, interactive)?;

    // ── Step 3 ────────────────────────────────────────────────────────────────
    print_step(3, "Model bindings and role assignment");
    let model_bindings = step3_model_bindings(&connections, interactive);
    println!("  {} model binding(s) configured.", model_bindings.len());
    for b in &model_bindings {
        println!(
            "    - {} = {} via {}",
            b.name, b.model_identity, b.provider_connection
        );
    }
    confirm_step(3, interactive)?;

    // ── Step 4 ────────────────────────────────────────────────────────────────
    print_step(4, "Adversarial diversity policy validation");
    step4_diversity(&model_bindings)?;
    println!("  Diversity policy: OK");
    confirm_step(4, interactive)?;

    // ── Step 5 ────────────────────────────────────────────────────────────────
    print_step(5, "Adapter connectivity test");
    // Load existing config (if any) for the configured sidecar binary path.
    let binary_path = load_config(&workspace_root)
        .ok()
        .and_then(|c| c.sidecar.binary_path);
    let sidecar_ok = step5_connectivity(
        &workspace_root,
        &connections,
        binary_path.as_deref(),
        interactive,
    );
    match &sidecar_ok {
        Ok(()) => println!("  Connectivity: OK"),
        // Binary not found: advisory skip. Sidecar may not be installed yet on a clean machine.
        Err(AnvilError::SidecarNotFound) => {
            println!("  anvil-sidecar binary not found — connectivity test skipped.");
            println!("  Install the sidecar and re-run `anvil setup` to validate connectivity.");
        }
        // Any other failure (spawn timeout, provider invoke error): hard failure.
        // The Plan specifies: "Failures are explicit: anvil setup does not continue past
        // Step 5 with a failing adapter."
        Err(e) => {
            eprintln!("  ERROR: connectivity test failed: {e}");
            eprintln!("  Fix the provider configuration and re-run `anvil setup`.");
            if interactive {
                let proceed = Confirm::new()
                    .with_prompt(
                        "Override: continue setup despite provider failure? [NOT RECOMMENDED]",
                    )
                    .default(false)
                    .interact()
                    .unwrap_or(false);
                if !proceed {
                    return Err(AnvilError::SetupCancelled);
                }
            } else {
                return Err(AnvilError::SetupCancelled);
            }
        }
    }
    confirm_step(5, interactive)?;

    // ── Step 6 ────────────────────────────────────────────────────────────────
    print_step(6, "Local store creation");
    step6_store(&workspace_root);
    confirm_step(6, interactive)?;

    // ── Step 7 ────────────────────────────────────────────────────────────────
    print_step(7, "Confirmation and summary");
    let (governance_model, trademark_posture, security_disclosure_contact) =
        step7_amendment_a1(interactive);

    let state = WizardState {
        workspace_root: workspace_root.clone(),
        connections,
        model_bindings,
        credential_mode,
        governance_model,
        trademark_posture,
        security_disclosure_contact,
    };

    print_summary(&state);

    if interactive {
        let confirmed = Confirm::new()
            .with_prompt("Commit all changes and complete setup?")
            .default(true)
            .interact()
            .unwrap_or(false);
        if !confirmed {
            println!("Setup cancelled. No changes were written.");
            return Err(AnvilError::SetupCancelled);
        }
    }

    commit(&state)?;
    println!();
    println!("Setup complete. Run `anvil config show` to inspect your configuration.");
    println!("Run `anvil gate check-plan` when you're ready to start the Plan stage.");
    Ok(())
}

// ── Step implementations ──────────────────────────────────────────────────────

/// Resolves the workspace path without initializing the project layout.
///
/// Full project initialization (`project::init`) runs inside `commit()` so that
/// cancelling after this step leaves no partial project state behind.
fn step1_workspace(path: &Path) -> Result<PathBuf, AnvilError> {
    std::fs::create_dir_all(path)?;
    Ok(path.canonicalize()?)
}

#[allow(clippy::too_many_lines)]
fn step2_providers(interactive: bool) -> (Vec<WizardConnection>, CredentialMode) {
    let credential_mode = detect_credential_mode();
    if credential_mode == CredentialMode::EnvVarOnly {
        println!();
        println!("  WARNING: OS keychain is unavailable on this system.");
        println!("  API keys will NOT be stored persistently.");
        println!("  Supply credentials per-session via environment variables:");
        println!("    {ENV_ANTHROPIC}, {ENV_OPENAI}, {ENV_GOOGLE}");
        println!();
    }

    let providers: &[(&str, ProviderType, &str, &str)] = &[
        (
            "Anthropic (Claude)",
            ProviderType::Anthropic,
            ENV_ANTHROPIC,
            DEFAULT_MODEL_ANTHROPIC,
        ),
        (
            "OpenAI (GPT)",
            ProviderType::OpenAi,
            ENV_OPENAI,
            DEFAULT_MODEL_OPENAI,
        ),
        (
            "Google AI Studio (Gemini)",
            ProviderType::Google,
            ENV_GOOGLE,
            DEFAULT_MODEL_GOOGLE,
        ),
    ];

    let mut connections = Vec::new();

    for (display_name, provider_type, env_var, default_model) in providers {
        let env_key = std::env::var(env_var).ok();
        let has_env = env_key.is_some();

        let configure = if interactive {
            Confirm::new()
                .with_prompt(format!("Configure {display_name}?"))
                .default(true)
                .interact()
                .unwrap_or(false)
        } else {
            has_env
        };

        if !configure {
            continue;
        }

        let conn_name: String = if interactive {
            Input::new()
                .with_prompt(format!("  Connection name for {display_name}"))
                .default(format!("my-{}", provider_type_slug(provider_type)))
                .interact_text()
                .unwrap_or_else(|_| format!("my-{}", provider_type_slug(provider_type)))
        } else {
            format!("my-{}", provider_type_slug(provider_type))
        };

        let maybe_credential: Option<WizardCredential> =
            if credential_mode == CredentialMode::EnvVarOnly {
                if has_env {
                    println!("  Using {env_var} from environment.");
                    Some(WizardCredential {
                        api_key: None,
                        env_var_name: Some((*env_var).to_owned()),
                        credential_ref: CredentialRef::EnvVar {
                            var_name: (*env_var).to_owned(),
                        },
                    })
                } else {
                    // Prompt for which env var name holds the key.
                    println!(
                    "  Keychain unavailable. Record which environment variable holds your API key."
                );
                    let var_name: String = if interactive {
                        Input::new()
                            .with_prompt(format!(
                                "  Environment variable name for {display_name} key"
                            ))
                            .default((*env_var).to_owned())
                            .interact_text()
                            .unwrap_or_else(|_| (*env_var).to_owned())
                    } else {
                        (*env_var).to_owned()
                    };
                    Some(WizardCredential {
                        api_key: None,
                        env_var_name: Some(var_name.clone()),
                        credential_ref: CredentialRef::EnvVar { var_name },
                    })
                }
            } else if has_env {
                // Env var is present. In interactive mode, offer to use it (avoids keychain prompting).
                let use_env = if interactive {
                    Confirm::new()
                        .with_prompt(format!(
                        "  {env_var} is already set — use env var instead of storing in keychain?"
                    ))
                        .default(true)
                        .interact()
                        .unwrap_or(true)
                } else {
                    true // Non-interactive: always use env var when present.
                };

                if use_env {
                    println!("  Using {env_var} from environment.");
                    Some(WizardCredential {
                        api_key: None,
                        env_var_name: Some((*env_var).to_owned()),
                        credential_ref: CredentialRef::EnvVar {
                            var_name: (*env_var).to_owned(),
                        },
                    })
                } else {
                    // User prefers keychain storage.
                    prompt_keychain_key(display_name, interactive)
                }
            } else {
                // No env var present; prompt for the key to store in keychain.
                prompt_keychain_key(display_name, interactive)
            };

        let Some(credential) = maybe_credential else {
            continue;
        };

        connections.push(WizardConnection {
            name: conn_name,
            provider_type: provider_type.clone(),
            endpoint: None,
            credential,
            test_model: (*default_model).to_owned(),
        });
    }

    (connections, credential_mode)
}

/// Prompt for an API key and return a `Keychain` credential, or `None` to skip.
fn prompt_keychain_key(display_name: &str, interactive: bool) -> Option<WizardCredential> {
    let key: String = if interactive {
        Password::new()
            .with_prompt(format!("  {display_name} API key"))
            .interact()
            .unwrap_or_default()
    } else {
        String::new()
    };
    if key.is_empty() {
        println!("  No key entered — skipping {display_name}.");
        None
    } else {
        Some(WizardCredential {
            api_key: Some(key),
            env_var_name: None,
            credential_ref: CredentialRef::Keychain,
        })
    }
}

fn step3_model_bindings(connections: &[WizardConnection], interactive: bool) -> Vec<ModelBinding> {
    if connections.is_empty() {
        return Vec::new();
    }

    let conn_names: Vec<&str> = connections.iter().map(|c| c.name.as_str()).collect();

    let default_models: &[(&str, &str, &str)] = &[
        (ROLE_CODER, "claude-opus-4-7", "Coder (code generation)"),
        (
            ROLE_INTERLOCUTOR,
            "claude-sonnet-4-6",
            "Interlocutor (discussion)",
        ),
        (
            ROLE_PLANNER,
            "claude-opus-4-7",
            "Planner (architectural design)",
        ),
        (ROLE_REVIEWER_1, "gpt-4o", "Reviewer-1 (first reviewer)"),
        (
            ROLE_REVIEWER_2,
            "gemini-1.5-pro",
            "Reviewer-2 (second reviewer)",
        ),
    ];

    let mut bindings = Vec::new();

    for (role, default_model, description) in default_models {
        if !interactive {
            if let Some(conn) = connections.first() {
                bindings.push(ModelBinding {
                    name: (*role).to_owned(),
                    model_identity: (*default_model).to_owned(),
                    provider_connection: conn.name.clone(),
                });
            }
            continue;
        }

        println!("  {description}");
        let model_id: String = Input::new()
            .with_prompt("    Model identity")
            .default((*default_model).to_owned())
            .interact_text()
            .unwrap_or_else(|_| (*default_model).to_owned());

        let conn_idx = if conn_names.len() == 1 {
            0
        } else {
            Select::new()
                .with_prompt("    Provider connection")
                .items(&conn_names)
                .default(0)
                .interact()
                .unwrap_or(0)
        };

        bindings.push(ModelBinding {
            name: (*role).to_owned(),
            model_identity: model_id,
            provider_connection: connections[conn_idx].name.clone(),
        });
    }

    bindings
}

fn step4_diversity(bindings: &[ModelBinding]) -> Result<(), AnvilError> {
    let find_model = |role: &str| -> String {
        bindings
            .iter()
            .find(|b| b.name == role)
            .map(|b| b.model_identity.clone())
            .unwrap_or_default()
    };

    let coder = find_model(ROLE_CODER);
    let r1 = find_model(ROLE_REVIEWER_1);
    let r2 = find_model(ROLE_REVIEWER_2);

    if coder.is_empty() || r1.is_empty() || r2.is_empty() {
        return Ok(());
    }

    let violations = validate_diversity(&coder, &r1, &r2);
    if violations.is_empty() {
        return Ok(());
    }

    let msgs: Vec<String> = violations.iter().map(|v| format!("  - {v}")).collect();
    Err(AnvilError::DiversityViolation(msgs.join("\n")))
}

fn step5_connectivity(
    workspace: &Path,
    connections: &[WizardConnection],
    binary_path: Option<&Path>,
    _interactive: bool,
) -> Result<(), AnvilError> {
    if connections.is_empty() {
        println!("  No connections configured — skipping connectivity test.");
        return Ok(());
    }

    let binary = anvil_core::sidecar::find_sidecar_binary(binary_path)?;
    println!("  Found sidecar binary: {}", binary.display());

    // Write temp config to system temp dir, not workspace, to avoid partial-state on cancel.
    let tmp_config = write_temp_provider_config(connections)?;

    println!("  Starting sidecar daemon...");
    let mut child = std::process::Command::new(&binary)
        .arg("--config")
        .arg(&tmp_config)
        .arg("--workspace")
        .arg(workspace)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    let port = match anvil_core::sidecar::wait_for_port_file(workspace, Duration::from_secs(10)) {
        Ok(p) => p,
        Err(e) => {
            let _ = child.kill();
            let _ = std::fs::remove_file(&tmp_config);
            return Err(e);
        }
    };
    println!("  Sidecar ready on port {port}.");

    let result = with_tokio(connectivity_checks(port, connections));

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(&tmp_config);
    let _ = std::fs::remove_file(workspace.join(".anvil/run/sidecar.pid"));
    let _ = std::fs::remove_file(workspace.join(".anvil/run/sidecar.port"));

    result
}

async fn connectivity_checks(
    port: u16,
    connections: &[WizardConnection],
) -> Result<(), AnvilError> {
    use anvil_sidecar_client::client::AnvilSidecarClient;

    let addr = format!("http://127.0.0.1:{port}");
    let mut client = AnvilSidecarClient::connect(addr)
        .await
        .map_err(|e| AnvilError::Io(std::io::Error::other(format!("gRPC connect: {e}"))))?;

    client
        .probe_health()
        .await
        .map_err(|e| AnvilError::Io(std::io::Error::other(format!("Health: {e}"))))?;
    println!("  Health: OK");

    // Find connections that have a resolvable API key for testing.
    let testable: Vec<(&WizardConnection, String)> = connections
        .iter()
        .filter_map(|conn| {
            let key = resolve_api_key(conn);
            if key.is_empty() {
                None
            } else {
                Some((conn, key))
            }
        })
        .collect();

    if testable.is_empty() {
        println!("  No keys available for invoke test — health probe only.");
        return Ok(());
    }

    // One handshake + reload covers all connections (they share the provider config).
    let config_json = sidecar_config_json(connections);
    let epoch = sidecar_config_epoch(&config_json);

    client
        .handshake(epoch.clone())
        .await
        .map_err(|e| AnvilError::Io(std::io::Error::other(format!("handshake: {e}"))))?;

    let reload_req = anvil_sidecar_client::proto::ReloadConfigRequest {
        new_config_epoch: epoch,
        new_provider_config: config_json.into_bytes(),
    };
    let reload = client
        .reload_config(reload_req)
        .await
        .map_err(|e| AnvilError::Io(std::io::Error::other(format!("reload_config: {e}"))))?;
    if !reload.success {
        return Err(AnvilError::Io(std::io::Error::other(format!(
            "reload_config rejected: {:?}",
            reload.error
        ))));
    }

    // Invoke each configured connection individually; collect failures.
    let mut failures: Vec<String> = Vec::new();
    for (conn, api_key) in &testable {
        let req = anvil_sidecar_client::proto::InvokeRequest {
            idempotency_key: String::new(),
            model_id: conn.test_model.clone(),
            provider_connection_id: conn.name.clone(),
            credentials: Some(anvil_sidecar_client::proto::Credentials {
                credential: Some(
                    anvil_sidecar_client::proto::credentials::Credential::ApiKey(api_key.clone()),
                ),
            }),
            timeout: Some(anvil_sidecar_client::proto::Timeout { millis: 30_000 }),
            payload: Some(anvil_sidecar_client::proto::invoke_request::Payload::Chat(
                anvil_sidecar_client::proto::ChatRequest {
                    system_prompt: String::new(),
                    messages: vec![anvil_sidecar_client::proto::Message {
                        role: "user".into(),
                        content: "Say hello in one word.".into(),
                    }],
                    max_tokens: Some(5),
                    temperature: None,
                },
            )),
        };

        match client.invoke(req).await {
            Ok(_) => println!("  {}: OK", conn.name),
            Err(e) => {
                println!("  {}: FAILED — {e}", conn.name);
                failures.push(conn.name.clone());
            }
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(AnvilError::Io(std::io::Error::other(format!(
            "connectivity failed for: {}",
            failures.join(", ")
        ))))
    }
}

/// Report directories that will be created at commit time (no disk writes here).
fn step6_store(workspace: &Path) {
    let to_create = anvil_core::project::layout_dirs()
        .iter()
        .filter(|d| !workspace.join(d).exists())
        .count();
    if to_create > 0 {
        println!("  {to_create} director(y/ies) will be initialized at commit.");
    } else {
        println!("  Project directories already present.");
    }
}

fn step7_amendment_a1(interactive: bool) -> (String, String, String) {
    println!("  Amendment A1 required choices:");

    let governance = if interactive {
        let options = &[
            "BDFL (Benevolent Dictator For Life)",
            "Core committer committee",
            "Other",
        ];
        let idx = Select::new()
            .with_prompt("  Governance model")
            .items(options)
            .default(0)
            .interact()
            .unwrap_or(0);
        options[idx].to_owned()
    } else {
        "BDFL (Benevolent Dictator For Life)".to_owned()
    };

    let trademark = "Posture A (reserved; no third-party use without permission)".to_owned();
    println!("  Trademark posture: {trademark} (Coordinator-locked)");

    let security_contact = if interactive {
        Input::new()
            .with_prompt("  Security disclosure contact email")
            .default("security@example.com".to_owned())
            .interact_text()
            .unwrap_or_else(|_| "security@example.com".to_owned())
    } else {
        "security@example.com".to_owned()
    };

    (governance, trademark, security_contact)
}

// ── Commit ────────────────────────────────────────────────────────────────────

fn commit(state: &WizardState) -> Result<(), AnvilError> {
    let root = &state.workspace_root;

    // 1. Initialize project layout (idempotent): creates dirs + default anvil.toml.
    //    This is the first filesystem write — nothing above commit() touches the layout.
    project::init(root)?;

    // 2. Write API keys to keychain.
    for conn in &state.connections {
        if let (Some(ref key), CredentialRef::Keychain) =
            (&conn.credential.api_key, &conn.credential.credential_ref)
        {
            let entry_name = keychain_entry_name(&conn.name);
            let entry = keyring::Entry::new(KEYRING_SERVICE, &entry_name)
                .map_err(|e| AnvilError::KeychainUnavailable(e.to_string()))?;
            entry
                .set_password(key)
                .map_err(|e| AnvilError::KeychainUnavailable(e.to_string()))?;
        }
    }

    // 3. Update anvil.toml with provider connections and model bindings.
    let mut config = load_config(root).unwrap_or_else(|_| AnvilConfig::default_locked());
    for conn in &state.connections {
        config.provider_connections.insert(
            conn.name.clone(),
            ProviderConnection {
                provider_type: conn.provider_type.clone(),
                endpoint: conn.endpoint.clone(),
                credential_ref: conn.credential.credential_ref.clone(),
            },
        );
    }
    config.model_bindings.clone_from(&state.model_bindings);

    // Populate reviewer_pool from bindings whose names start with "reviewer-" (F3).
    let reviewer_pool: Vec<String> = state
        .model_bindings
        .iter()
        .filter(|b| b.name.starts_with("reviewer-"))
        .map(|b| b.name.clone())
        .collect();
    if !reviewer_pool.is_empty() {
        config.reviewer_pool = reviewer_pool;
    }

    save_config(root, &config)?;

    // 4. Write audit records.
    let store = AuditStore::open(root)?;

    for step in 1..=WIZARD_STEPS {
        let record = GateApproval::new(format!("wizard-step-{step}"), "user".to_owned(), vec![]);
        store.append(&record)?;
    }

    store.append(&ProvisionalLock::new(
        "governance_model".to_owned(),
        state.governance_model.clone(),
        vec![],
    ))?;
    store.append(&ProvisionalLock::new(
        "trademark_posture".to_owned(),
        state.trademark_posture.clone(),
        vec![],
    ))?;
    store.append(&ProvisionalLock::new(
        "security_disclosure_contact".to_owned(),
        state.security_disclosure_contact.clone(),
        vec![],
    ))?;

    println!("  Configuration written to {}/anvil.toml", root.display());
    println!("  {} audit records written.", WIZARD_STEPS + 3);

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn print_step(n: usize, name: &str) {
    println!("--- Step {n}/{WIZARD_STEPS}: {name} ---");
}

fn confirm_step(step: usize, interactive: bool) -> Result<(), AnvilError> {
    if !interactive {
        return Ok(());
    }
    let ok = Confirm::new()
        .with_prompt(format!("  Step {step} complete — continue?"))
        .default(true)
        .interact()
        .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err(AnvilError::SetupCancelled)
    }
}

fn print_summary(state: &WizardState) {
    println!();
    println!("=== Setup Summary ===");
    println!("Workspace: {}", state.workspace_root.display());
    println!("Provider connections:");
    for c in &state.connections {
        println!(
            "  {} ({:?}, credential: {:?})",
            c.name, c.provider_type, c.credential.credential_ref
        );
    }
    println!("Model bindings:");
    for b in &state.model_bindings {
        println!(
            "  {} = {} via {}",
            b.name, b.model_identity, b.provider_connection
        );
    }
    println!("Governance model: {}", state.governance_model);
    println!("Trademark posture: {}", state.trademark_posture);
    println!("Security contact: {}", state.security_disclosure_contact);
    println!(
        "Credential storage: {}",
        if state.credential_mode == CredentialMode::Keychain {
            "OS keychain"
        } else {
            "environment variables"
        }
    );
    println!();
}

fn detect_credential_mode() -> CredentialMode {
    match keyring::Entry::new(KEYRING_SERVICE, "__probe__") {
        Ok(entry) => {
            let available = entry.set_password("probe").is_ok();
            let _ = entry.delete_credential();
            if available {
                CredentialMode::Keychain
            } else {
                CredentialMode::EnvVarOnly
            }
        }
        Err(_) => CredentialMode::EnvVarOnly,
    }
}

pub(crate) fn keychain_entry_name(conn_name: &str) -> String {
    format!("provider-{conn_name}")
}

fn provider_type_slug(pt: &ProviderType) -> &'static str {
    match pt {
        ProviderType::Anthropic => "anthropic",
        ProviderType::OpenAi => "openai",
        ProviderType::Google => "google",
        ProviderType::AwsBedrock => "bedrock",
        ProviderType::AzureOpenAi => "azure-openai",
        ProviderType::GoogleVertexAi => "vertex",
        ProviderType::Other(_) => "custom",
    }
}

/// Write the temp provider config to the system temp directory (not the workspace)
/// so Step 5 does not touch the workspace before commit.
fn write_temp_provider_config(connections: &[WizardConnection]) -> Result<PathBuf, AnvilError> {
    let json = sidecar_config_json(connections);
    let path = std::env::temp_dir().join(format!("anvil-setup-{}.json", uuid::Uuid::new_v4()));
    std::fs::write(&path, json.as_bytes())?;
    Ok(path)
}

fn sidecar_config_json(connections: &[WizardConnection]) -> String {
    #[derive(serde::Serialize)]
    struct SidecarConfig<'a> {
        version: u32,
        connections: Vec<SidecarConn<'a>>,
    }
    #[derive(serde::Serialize)]
    struct SidecarConn<'a> {
        id: &'a str,
        provider: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        endpoint: Option<&'a str>,
    }

    let conns: Vec<SidecarConn<'_>> = connections
        .iter()
        .map(|c| SidecarConn {
            id: &c.name,
            provider: provider_type_sidecar_str(&c.provider_type),
            endpoint: c.endpoint.as_deref(),
        })
        .collect();

    serde_json::to_string(&SidecarConfig {
        version: 1,
        connections: conns,
    })
    .expect("sidecar config serialization must not fail")
}

pub(crate) fn provider_type_sidecar_str(pt: &ProviderType) -> &'static str {
    match pt {
        ProviderType::Anthropic => "anthropic",
        ProviderType::OpenAi => "openai",
        ProviderType::Google => "google_ai_studio",
        ProviderType::AwsBedrock => "aws_bedrock",
        ProviderType::AzureOpenAi => "azure_openai",
        ProviderType::GoogleVertexAi => "google_vertex_ai",
        ProviderType::Other(_) => "unknown",
    }
}

pub(crate) fn sidecar_config_epoch(json: &str) -> String {
    use std::fmt::Write as _;
    let digest = sha2::Sha256::digest(json.as_bytes());
    let mut hex = String::with_capacity(64);
    for b in &digest {
        write!(hex, "{b:02x}").unwrap();
    }
    hex
}

/// Retrieve the API key for a connection: from in-memory field or env var.
fn resolve_api_key(conn: &WizardConnection) -> String {
    if let Some(k) = &conn.credential.api_key {
        k.clone()
    } else {
        let var = conn.credential.env_var_name.as_deref().unwrap_or_default();
        std::env::var(var).unwrap_or_default()
    }
}

/// Run a future on a single-threaded Tokio runtime (for use in sync CLI context).
pub(crate) fn with_tokio<F: std::future::Future>(f: F) -> F::Output {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    let rt = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime build must succeed")
    });
    rt.block_on(f)
}

use sha2::Digest as _;

// ── Hinge tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_core::config::{CredentialRef, ProviderConnection, ProviderType};

    // hinge_test: pins=7, intended=wizard-step-count, phase=P4
    // Exact-equality pin is intentional for v1: the 7-step structure is a user-facing
    // contract described in the Plan and walkthrough document. Adding or removing a step
    // is a deliberate contract change, not an incidental refactor.
    #[test]
    fn test_wizard_step_count() {
        assert_eq!(WIZARD_STEPS, 7, "wizard must have exactly 7 steps");
    }

    // hinge_test: pins=api-keys-not-in-config, intended=no-plaintext-api-keys-on-disk, phase=P4
    #[test]
    fn test_api_keys_encrypted_at_rest() {
        let conn = ProviderConnection {
            provider_type: ProviderType::Anthropic,
            endpoint: None,
            credential_ref: CredentialRef::Keychain,
        };
        let serialized = toml::to_string_pretty(&conn).expect("must serialize");
        assert!(
            !serialized.contains("api_key"),
            "api_key must not appear in serialized ProviderConnection: {serialized}"
        );
        assert!(
            !serialized.contains("password"),
            "password must not appear in serialized ProviderConnection: {serialized}"
        );
        let conn_env = ProviderConnection {
            provider_type: ProviderType::OpenAi,
            endpoint: None,
            credential_ref: CredentialRef::EnvVar {
                var_name: "ANVIL_API_KEY_OPENAI".to_owned(),
            },
        };
        let ser_env = toml::to_string_pretty(&conn_env).expect("must serialize");
        assert!(
            !ser_env.contains("sk-"),
            "API key prefix must not appear in serialized config: {ser_env}"
        );
    }

    // hinge_test: pins=wizard-cancellation-leaves-no-partial-state, intended=transactional-wizard, phase=P4
    // This test exercises the real behavior: step1_workspace() must NOT call project::init().
    // Full layout creation runs only inside commit(). Cancelled setup leaves no anvil.toml.
    #[test]
    fn test_wizard_cancellation_leaves_no_partial_state() {
        let tmp = std::env::temp_dir().join(format!("anvil-cancel-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();

        let resolved = step1_workspace(&tmp).expect("step1 must resolve path");

        assert!(
            !resolved.join("anvil.toml").exists(),
            "anvil.toml must not exist after step1 only — project::init() belongs in commit()"
        );
        assert!(
            !resolved.join("audit-store").exists(),
            "audit-store must not exist after step1 only"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    // hinge_test: pins=env-var-names, intended=api-keys-env-var-bypass-works-headless, phase=P4
    #[test]
    fn test_api_keys_env_var_bypass_works_headless() {
        assert_eq!(ENV_ANTHROPIC, "ANVIL_API_KEY_ANTHROPIC");
        assert_eq!(ENV_OPENAI, "ANVIL_API_KEY_OPENAI");
        assert_eq!(ENV_GOOGLE, "ANVIL_API_KEY_GOOGLE");
    }

    // hinge_test: pins=workspace-runtime-dir, intended=sidecar-runtime-files-location, phase=P4
    // Pins that .anvil/run is in the project layout for sidecar PID/port files.
    // Concurrent-write protection via OS-level exclusive lock is a future requirement.
    #[test]
    fn test_workspace_runtime_dir_in_layout() {
        let dirs = anvil_core::project::layout_dirs();
        assert!(
            dirs.iter().any(|d| d == ".anvil/run"),
            ".anvil/run must be in the project layout (sidecar runtime files)"
        );
        assert!(
            dirs.iter().any(|d| d == ".anvil"),
            ".anvil must be in the project layout"
        );
    }

    // hinge_test: pins=headless-no-provider-guard, intended=headless-requires-provider, phase=P4
    // Behavior test: headless mode (non-terminal stdin) with no ANVIL_API_KEY_* env vars set
    // must return Err(SetupCancelled) before reaching commit().
    // Skips if any provider env var is present in this environment (dev machine with real keys).
    #[test]
    fn test_headless_no_provider_guard() {
        if std::env::var(ENV_ANTHROPIC).is_ok()
            || std::env::var(ENV_OPENAI).is_ok()
            || std::env::var(ENV_GOOGLE).is_ok()
        {
            return; // env vars present — skip to avoid false failure on dev machines
        }
        let tmp =
            std::env::temp_dir().join(format!("anvil-headless-guard-{}", uuid::Uuid::new_v4()));
        // run_wizard with non-terminal stdin (always true in cargo test) and no provider env vars.
        // step2_providers(false) returns empty; the guard fires and returns SetupCancelled.
        let result = run_wizard(&tmp);
        let _ = std::fs::remove_dir_all(&tmp);
        assert!(
            matches!(result, Err(AnvilError::SetupCancelled)),
            "headless setup with no provider env vars must return Err(SetupCancelled), got: {result:?}"
        );
    }
}
