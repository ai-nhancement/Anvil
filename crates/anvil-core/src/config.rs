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
    /// Ordered list of reviewer binding names for round-robin rotation (P6).
    /// Empty means fall back to `["reviewer-1"]` (single-reviewer mode).
    #[serde(default)]
    pub reviewer_pool: Vec<String>,
    /// If `true`, a single clean reviewer pass satisfies the full-pool clean convergence
    /// condition even when the pool has more than one member (P6).
    #[serde(default)]
    pub single_clean_pass_override: bool,
    /// Ordered list of transport actions executed by `anvil ship` (P9).
    /// An empty list is valid — no external commands are run on ship.
    #[serde(default)]
    pub transport_actions: Vec<TransportAction>,
    /// Layer-2 numeric targets for the six Layer-1 evaluation metrics (P10a).
    #[serde(default)]
    pub metric_targets: MetricTargets,
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
            reviewer_pool: Vec::new(),
            single_clean_pass_override: false,
            transport_actions: Vec::new(),
            metric_targets: MetricTargets::default(),
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
        self.metric_targets.validate()?;
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

/// Kind of transport action supported by the Ship gate (P9).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    /// Run a shell command in the project root directory.
    #[default]
    Shell,
}

/// One configured transport action executed by `anvil ship` (P9).
///
/// Actions are executed in declaration order. The first failure aborts the sequence.
/// An empty `transport_actions` list is valid — `anvil ship` succeeds without running
/// any external command (gates are still enforced).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransportAction {
    /// Execution strategy (only `shell` in v1).
    #[serde(default)]
    pub kind: TransportKind,
    /// Command string passed to the system shell.
    pub command: String,
    /// Human-readable label shown during execution. Defaults to `command` if absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Layer-2 numeric targets for the six Layer-1 evaluation metrics (P10a).
///
/// All fields have defaults; an absent `[metric_targets]` section in `anvil.toml`
/// uses the values shown below. Override any field to tighten or relax a project's
/// specific targets.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricTargets {
    /// Minimum acceptable reviewer precision (0.0–1.0). Default: 0.70.
    #[serde(default = "MetricTargets::default_precision_min")]
    pub precision_min: f64,
    /// Maximum acceptable cross-reviewer agreement score (0.0–1.0). Default: 0.90.
    #[serde(default = "MetricTargets::default_agreement_max")]
    pub agreement_max: f64,
    /// Maximum acceptable average human minutes per shipped phase. Default: 120.0.
    #[serde(default = "MetricTargets::default_human_minutes_max")]
    pub human_minutes_max: f64,
    /// Maximum acceptable average review rounds per phase. Default: 5.0.
    #[serde(default = "MetricTargets::default_round_count_max")]
    pub round_count_max: f64,
    /// Maximum acceptable defect escape rate (0.0–1.0). Default: 0.10.
    #[serde(default = "MetricTargets::default_escape_rate_max")]
    pub escape_rate_max: f64,
}

impl Default for MetricTargets {
    fn default() -> Self {
        Self {
            precision_min: Self::default_precision_min(),
            agreement_max: Self::default_agreement_max(),
            human_minutes_max: Self::default_human_minutes_max(),
            round_count_max: Self::default_round_count_max(),
            escape_rate_max: Self::default_escape_rate_max(),
        }
    }
}

impl MetricTargets {
    fn default_precision_min() -> f64 {
        0.70
    }
    fn default_agreement_max() -> f64 {
        0.90
    }
    fn default_human_minutes_max() -> f64 {
        120.0
    }
    fn default_round_count_max() -> f64 {
        5.0
    }
    fn default_escape_rate_max() -> f64 {
        0.10
    }

    /// Validates that all threshold values are finite and within their acceptable ranges.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::InvalidConfigValue`] for the first field that fails.
    pub fn validate(&self) -> Result<(), AnvilError> {
        for (key, val) in [
            ("metric_targets.precision_min", self.precision_min),
            ("metric_targets.agreement_max", self.agreement_max),
            ("metric_targets.escape_rate_max", self.escape_rate_max),
        ] {
            if !val.is_finite() || !(0.0..=1.0).contains(&val) {
                return Err(AnvilError::InvalidConfigValue {
                    key: key.to_owned(),
                    reason: format!("must be a finite value between 0.0 and 1.0, got {val}"),
                });
            }
        }
        for (key, val) in [
            ("metric_targets.human_minutes_max", self.human_minutes_max),
            ("metric_targets.round_count_max", self.round_count_max),
        ] {
            if !val.is_finite() || val < 0.0 {
                return Err(AnvilError::InvalidConfigValue {
                    key: key.to_owned(),
                    reason: format!("must be a finite non-negative value, got {val}"),
                });
            }
        }
        Ok(())
    }
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
