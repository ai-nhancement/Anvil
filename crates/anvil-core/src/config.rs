use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::choices::{default_choices, Choice};
use crate::error::AnvilError;

/// Top-level `anvil.toml` configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnvilConfig {
    /// Required-Choices schema (17 entries in v1).
    pub choices: BTreeMap<String, Choice>,
    #[serde(default)]
    pub sidecar: SidecarConfig,
    /// Named provider connections (configured via `anvil setup` in P4).
    #[serde(default)]
    pub provider_connections: BTreeMap<String, ProviderConnection>,
    /// Model bindings: (`model_identity`, `provider_connection`) tuples.
    #[serde(default)]
    pub model_bindings: Vec<ModelBinding>,
}

impl AnvilConfig {
    /// Returns a default config with all choices in their plan-locked states.
    #[must_use]
    pub fn default_locked() -> Self {
        Self {
            choices: default_choices(),
            sidecar: SidecarConfig::default(),
            provider_connections: BTreeMap::new(),
            model_bindings: Vec::new(),
        }
    }

    /// Validates all `Provisional` choices have non-empty `hypothesis` and `revision_trigger`.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::ProvisionalMissingField`] for the first offending choice.
    pub fn validate(&self) -> Result<(), AnvilError> {
        for (key, choice) in &self.choices {
            choice.validate(key)?;
        }
        Ok(())
    }
}

/// Sidecar daemon configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SidecarConfig {
    /// Idle timeout in seconds before the daemon auto-exits. Default: 1800 (30 min).
    #[serde(default = "SidecarConfig::default_idle_timeout_secs")]
    pub idle_timeout_secs: u32,
    /// Path to the `anvil-sidecar` binary. Defaults to `$PATH` lookup if absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_path: Option<PathBuf>,
}

impl Default for SidecarConfig {
    fn default() -> Self {
        Self {
            idle_timeout_secs: 1800,
            binary_path: None,
        }
    }
}

impl SidecarConfig {
    fn default_idle_timeout_secs() -> u32 {
        1800
    }
}

/// How the runtime credential (API key or token) for a provider connection is sourced.
///
/// API keys are NEVER stored in `anvil.toml`. This enum records how to retrieve them
/// at invocation time (from the OS keychain or from a named environment variable).
/// The keychain entry name follows the pattern `"anvil:provider-{connection_name}"`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum CredentialRef {
    /// Key is stored in the OS keychain (Windows Credential Manager / macOS Keychain / Linux Secret Service).
    #[default]
    Keychain,
    /// Key must be supplied via the named environment variable at runtime.
    EnvVar { var_name: String },
}

/// A named provider connection entry in `anvil.toml`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderConnection {
    pub provider_type: ProviderType,
    /// Optional custom endpoint override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// How the API key / credential is sourced at runtime.
    /// Keys are never stored in anvil.toml.
    #[serde(default)]
    pub credential_ref: CredentialRef,
}

/// Supported provider types for model access.
///
/// Serialized strings match the Go sidecar's `ProviderType` constants so that
/// the Vault-generated provider config JSON is accepted directly by the sidecar.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProviderType {
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "google_ai_studio")]
    Google,
    #[serde(rename = "aws_bedrock")]
    AwsBedrock,
    #[serde(rename = "azure_openai")]
    AzureOpenAi,
    #[serde(rename = "google_vertex_ai")]
    GoogleVertexAi,
    #[serde(untagged)]
    Other(String),
}

/// A model binding: ties a model identity to a provider connection.
///
/// Role assignments (Coder, Planner, Reviewer) reference model bindings by name.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelBinding {
    /// Logical name for this binding (referenced by role assignments).
    pub name: String,
    /// Model identity string (e.g., `claude-sonnet-4-6`, `gpt-5`).
    pub model_identity: String,
    /// Name of the provider connection to route through.
    pub provider_connection: String,
}

/// Loads and validates `anvil.toml` from `<root>/anvil.toml`.
///
/// # Errors
///
/// Returns [`AnvilError::NotInitialized`] if no `anvil.toml` is found, [`AnvilError::Io`]
/// on read failure, [`AnvilError::ConfigParse`] if the TOML is malformed, or
/// [`AnvilError::ProvisionalMissingField`] if a Provisional choice lacks required metadata.
pub fn load_config(root: &Path) -> Result<AnvilConfig, AnvilError> {
    let path = root.join("anvil.toml");
    if !path.exists() {
        return Err(AnvilError::NotInitialized(root.to_path_buf()));
    }
    let raw = std::fs::read_to_string(&path)?;
    let config: AnvilConfig = toml::from_str(&raw).map_err(|source| AnvilError::ConfigParse {
        path: path.clone(),
        source: Box::new(source),
    })?;
    config.validate()?;
    Ok(config)
}

/// Serializes `config` and writes it to `<root>/anvil.toml`.
///
/// # Errors
///
/// Returns [`AnvilError::ProvisionalMissingField`] if validation fails, [`AnvilError::ConfigSerialize`]
/// on TOML serialization failure, or [`AnvilError::Io`] on write failure.
pub fn save_config(root: &Path, config: &AnvilConfig) -> Result<(), AnvilError> {
    config.validate()?;
    let serialized = toml::to_string_pretty(config)?;
    let path = root.join("anvil.toml");
    std::fs::write(&path, serialized)?;
    Ok(())
}

/// Returns the list of unlocked choice keys.
///
/// An empty return value means the gate passes and Plan stage is unblocked.
#[must_use]
pub fn check_plan_stage_gate(config: &AnvilConfig) -> Vec<String> {
    config
        .choices
        .iter()
        .filter(|(_, choice)| !choice.is_locked())
        .map(|(key, _)| key.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_type_other_roundtrip() {
        let toml_input = r#"
provider_type = "my-custom-provider"
"#;
        let conn: ProviderConnection = toml::from_str(toml_input).expect("should deserialize");
        assert_eq!(
            conn.provider_type,
            ProviderType::Other("my-custom-provider".to_owned())
        );
        let serialized = toml::to_string_pretty(&conn).expect("should serialize");
        let conn2: ProviderConnection = toml::from_str(&serialized).expect("should round-trip");
        assert_eq!(conn.provider_type, conn2.provider_type);
    }
}
