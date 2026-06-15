//! Configuration: providers, model bindings, and role assignments.
//!
//! anvil.toml lives at the project root. Credentials are never stored in it.
//! They come from OS keyring (preferred) or named environment variables.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const ANVIL_DIR: &str = ".anvil";
pub const CONFIG_FILE: &str = "anvil.toml";
pub const STATE_FILE: &str = "state.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnvilConfig {
    /// Role bindings — these are the named model_bindings you will actually use.
    #[serde(default)]
    pub roles: Roles,

    /// Named provider connections (how we reach the model: direct, Azure, Ollama, gateway, etc.)
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderConnection>,

    /// Model bindings: a logical name + which provider + exact model id string.
    #[serde(default)]
    pub model_bindings: BTreeMap<String, ModelBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Roles {
    /// Primary model — used for all coding, planning, and chat
    pub coder: Option<String>,

    /// First reviewer (must be different provider/family from coder)
    pub reviewer_a: Option<String>,

    /// Second reviewer (different from reviewer_a and coder)
    pub reviewer_b: Option<String>,

    /// Deprecated — ignored, kept only for smooth migration of old configs
    #[serde(default, skip_serializing)]
    pub planner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConnection {
    /// "openai_compat" | "anthropic" | "google" | "aws_bedrock" | "azure_openai" | ...
    /// "openai_compat" is the universal path for Ollama, Groq, Together, Fireworks, OpenRouter,
    /// vLLM, LocalAI, Azure, AWS (via gateway), Gradient, Vertex, etc.
    pub r#type: String,

    /// For openai_compat and some others: the base URL (no trailing slash).
    /// Examples:
    ///   http://localhost:11434/v1          (Ollama)
    ///   https://api.groq.com/openai/v1
    ///   https://api.together.xyz/v1
    ///   https://<your-resource>.openai.azure.com/openai/deployments/<deployment>
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// How to obtain the credential at runtime.
    #[serde(default)]
    pub credential: CredentialRef,

    /// Extra headers or provider-specific hints (rarely needed).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum CredentialRef {
    /// Stored in OS keyring under service "anvil" and the given entry name.
    /// The actual keyring entry is "anvil:provider:<connection_name>"
    #[default]
    Keyring,

    /// Read from this environment variable at call time.
    Env {
        var_name: String,
    },

    /// No credential / secret required (common for local Ollama at http://localhost:11434/v1,
    /// many self-hosted openai-compat servers, vLLM without auth, etc.).
    /// A harmless placeholder is still supplied at call time so all code paths stay uniform.
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelBinding {
    /// Which provider connection to use (key into providers map)
    pub provider: String,

    /// The exact model identifier the provider expects (e.g. "llama3.1:70b", "claude-3-5-sonnet-20241022", "gpt-4o")
    pub model: String,

    /// Optional short human note (e.g. "fast local", "strong at reviews")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("project not initialized (no anvil.toml at {0})")]
    NotInitialized(PathBuf),

    #[error("failed to read config: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse anvil.toml: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("failed to serialize anvil.toml: {0}")]
    Serialize(#[from] toml::ser::Error),

    #[error("unknown model binding: {0}")]
    UnknownBinding(String),

    #[error("unknown provider connection: {0}")]
    UnknownProvider(String),

    #[error("role '{role}' is not configured")]
    RoleNotConfigured { role: String },
}

pub fn config_path(root: &Path) -> PathBuf {
    root.join(CONFIG_FILE)
}

pub fn anvil_dir(root: &Path) -> PathBuf {
    root.join(ANVIL_DIR)
}

pub fn state_path(root: &Path) -> PathBuf {
    anvil_dir(root).join(STATE_FILE)
}

pub fn load_config(root: &Path) -> Result<AnvilConfig, ConfigError> {
    let path = config_path(root);
    if !path.exists() {
        return Err(ConfigError::NotInitialized(root.to_path_buf()));
    }
    let raw = std::fs::read_to_string(&path)?;
    let cfg: AnvilConfig = toml::from_str(&raw)?;
    Ok(cfg)
}

pub fn save_config(root: &Path, cfg: &AnvilConfig) -> Result<(), ConfigError> {
    std::fs::create_dir_all(root)?;
    let path = config_path(root);
    let serialized = toml::to_string_pretty(cfg)?;
    std::fs::write(path, serialized)?;
    Ok(())
}

pub fn ensure_anvil_dir(root: &Path) -> Result<PathBuf, ConfigError> {
    let dir = anvil_dir(root);
    std::fs::create_dir_all(&dir)?;
    // touch a .gitkeep so the dir is tracked if user wants
    let keep = dir.join(".gitkeep");
    if !keep.exists() {
        let _ = std::fs::write(&keep, "# Anvil runtime state (reviews, snapshots, etc.)\n");
    }
    Ok(dir)
}

impl AnvilConfig {
    pub fn get_binding(&self, name: &str) -> Result<&ModelBinding, ConfigError> {
        self.model_bindings
            .get(name)
            .ok_or_else(|| ConfigError::UnknownBinding(name.to_string()))
    }

    pub fn get_provider(&self, name: &str) -> Result<&ProviderConnection, ConfigError> {
        self.providers
            .get(name)
            .ok_or_else(|| ConfigError::UnknownProvider(name.to_string()))
    }

    #[allow(dead_code)]
    pub fn resolve_role(&self, role: &str) -> Result<&ModelBinding, ConfigError> {
        let binding_name = match role {
            "coder" | "planner" => self.roles.coder.as_deref(),
            "reviewer-a" | "reviewer_a" => self.roles.reviewer_a.as_deref(),
            "reviewer-b" | "reviewer_b" => self.roles.reviewer_b.as_deref(),
            other => return Err(ConfigError::RoleNotConfigured { role: other.to_string() }),
        };
        let name = binding_name.ok_or(ConfigError::RoleNotConfigured { role: role.to_string() })?;
        self.get_binding(name)
    }

    /// Returns (binding_name, binding, provider) for a role — convenient for calls.
    pub fn resolve_role_full(&self, role: &str) -> Result<(&str, &ModelBinding, &ProviderConnection), ConfigError> {
        let name = match role {
            "coder" | "planner" => self.roles.coder.as_deref(),
            "reviewer-a" | "reviewer_a" => self.roles.reviewer_a.as_deref(),
            "reviewer-b" | "reviewer_b" => self.roles.reviewer_b.as_deref(),
            other => return Err(ConfigError::RoleNotConfigured { role: other.to_string() }),
        };
        let name = name.ok_or(ConfigError::RoleNotConfigured { role: role.to_string() })?;
        let binding = self.get_binding(name)?;
        let provider = self.get_provider(&binding.provider)?;
        Ok((name, binding, provider))
    }
}
