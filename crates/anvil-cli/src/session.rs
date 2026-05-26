//! Sidecar session management for P5+ CLI pipeline commands.
//!
//! Provides helpers to start the sidecar daemon on demand, retrieve runtime credentials,
//! connect to the sidecar via gRPC, and execute the handshake / config-reload sequence.

use std::path::Path;
use std::time::Duration;

use anvil_core::{
    config::{AnvilConfig, CredentialRef},
    error::AnvilError,
    sidecar as sidecar_util,
};
use anvil_sidecar_client::client::AnvilSidecarClient;

use crate::setup::{
    keychain_entry_name, provider_type_sidecar_str, sidecar_config_epoch, with_tokio,
    KEYRING_SERVICE,
};

// ── Sidecar config JSON ────────────────────────────────────────────────────────

/// Builds the sidecar provider-config JSON from the loaded `AnvilConfig`.
/// API keys are NOT included — they are passed per-request in `InvokeRequest.credentials`.
#[must_use]
pub(crate) fn build_sidecar_config_json(config: &AnvilConfig) -> String {
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

    let conns: Vec<SidecarConn<'_>> = config
        .provider_connections
        .iter()
        .map(|(name, conn)| SidecarConn {
            id: name.as_str(),
            provider: provider_type_sidecar_str(&conn.provider_type),
            endpoint: conn.endpoint.as_deref(),
        })
        .collect();

    serde_json::to_string(&SidecarConfig {
        version: 1,
        connections: conns,
    })
    .expect("sidecar config serialization must not fail")
}

// ── Credential retrieval ───────────────────────────────────────────────────────

/// Retrieves the API key for `conn_name` from the OS keychain or environment variable.
///
/// # Errors
///
/// Returns [`AnvilError::CredentialError`] if the key cannot be retrieved.
pub(crate) fn retrieve_api_key(
    conn_name: &str,
    cred_ref: &CredentialRef,
) -> Result<String, AnvilError> {
    match cred_ref {
        CredentialRef::Keychain => {
            let entry_name = keychain_entry_name(conn_name);
            let entry = keyring::Entry::new(KEYRING_SERVICE, &entry_name).map_err(|e| {
                AnvilError::CredentialError {
                    name: conn_name.to_owned(),
                    reason: format!("keychain entry error: {e}"),
                }
            })?;
            entry
                .get_password()
                .map_err(|e| AnvilError::CredentialError {
                    name: conn_name.to_owned(),
                    reason: format!("keychain read error: {e}"),
                })
        }
        CredentialRef::EnvVar { var_name } => {
            std::env::var(var_name).map_err(|_| AnvilError::CredentialError {
                name: conn_name.to_owned(),
                reason: format!("environment variable {var_name} is not set"),
            })
        }
    }
}

// ── Sidecar lifecycle ──────────────────────────────────────────────────────────

/// Ensures the sidecar daemon is running for `project_root`.
///
/// If the daemon is already alive (PID file exists, process is live), returns the
/// existing port. Otherwise starts a new daemon with the config derived from
/// `config.provider_connections` and returns the assigned port.
///
/// # Errors
///
/// Returns [`AnvilError::SidecarNotFound`] if the binary cannot be located,
/// [`AnvilError::SidecarStartTimeout`] if the port file is not written in time,
/// or [`AnvilError::Io`] on process-spawn failure.
pub(crate) fn ensure_sidecar_running(
    project_root: &Path,
    config: &AnvilConfig,
) -> Result<u16, AnvilError> {
    let pid_path = project_root.join(".anvil/run/sidecar.pid");
    let port_path = project_root.join(".anvil/run/sidecar.port");

    // Check if already running.
    if pid_path.exists() {
        if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                if sidecar_util::is_process_alive(pid) {
                    if let Ok(port_str) = std::fs::read_to_string(&port_path) {
                        if let Ok(port) = port_str.trim().parse::<u16>() {
                            return Ok(port);
                        }
                    }
                }
            }
        }
    }

    // Not running — spawn.
    let binary = sidecar_util::find_sidecar_binary(config.sidecar.binary_path.as_deref())?;

    let config_json = build_sidecar_config_json(config);
    let config_path = project_root.join(".anvil/sidecar-config.json");
    std::fs::write(&config_path, config_json.as_bytes())?;

    std::process::Command::new(&binary)
        .arg("--config")
        .arg(&config_path)
        .arg("--workspace")
        .arg(project_root)
        .arg("--idle-timeout")
        .arg(format!("{}s", config.sidecar.idle_timeout_secs))
        .spawn()?;

    sidecar_util::wait_for_port_file(project_root, Duration::from_secs(15))
}

// ── gRPC connection + handshake ────────────────────────────────────────────────

/// Connects to the sidecar at `port`, performs the Handshake RPC, and if a config-epoch
/// mismatch is detected, calls `reload_config`. Returns a `Ready` client.
///
/// # Errors
///
/// Returns [`AnvilError::Io`] on transport or handshake failure.
pub(crate) fn connect_and_handshake(
    port: u16,
    config: &AnvilConfig,
) -> Result<AnvilSidecarClient, AnvilError> {
    let config_json = build_sidecar_config_json(config);
    let epoch = sidecar_config_epoch(&config_json);

    with_tokio(async move {
        let addr = format!("http://127.0.0.1:{port}");
        let mut client = AnvilSidecarClient::connect(addr)
            .await
            .map_err(|e| AnvilError::Io(std::io::Error::other(format!("sidecar connect: {e}"))))?;

        client.handshake(epoch.clone()).await.map_err(|e| {
            AnvilError::Io(std::io::Error::other(format!("sidecar handshake: {e}")))
        })?;

        if client.needs_config_reload() {
            let reload_req = anvil_sidecar_client::proto::ReloadConfigRequest {
                new_config_epoch: epoch,
                new_provider_config: config_json.into_bytes(),
            };
            let reload = client.reload_config(reload_req).await.map_err(|e| {
                AnvilError::Io(std::io::Error::other(format!("sidecar reload_config: {e}")))
            })?;
            if !reload.success {
                return Err(AnvilError::Io(std::io::Error::other(format!(
                    "sidecar reload_config rejected: {:?}",
                    reload.error
                ))));
            }
        }

        Ok(client)
    })
}

/// Looks up a model binding by role name (e.g., `"interlocutor"`, `"reviewer-1"`).
///
/// # Errors
///
/// Returns [`AnvilError::ModelBindingMissing`] if no binding with that name exists.
pub(crate) fn find_model_binding<'a>(
    config: &'a AnvilConfig,
    role: &str,
) -> Result<&'a anvil_core::config::ModelBinding, AnvilError> {
    config
        .model_bindings
        .iter()
        .find(|b| b.name == role)
        .ok_or_else(|| AnvilError::ModelBindingMissing(role.to_owned()))
}
