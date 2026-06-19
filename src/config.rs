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

    /// Web-search backend for evidence-gathering specialists (v0.4.0).
    #[serde(default)]
    pub web_search: WebSearchSettings,

    /// Command-approval policy (which shell commands skip the y/n prompt).
    #[serde(default)]
    pub approvals: ApprovalSettings,
}

/// `[web_search]` block: which backend the `researcher` specialist uses. The API
/// key is never stored here — it comes from a conventional environment variable
/// (`TAVILY_API_KEY` / `BRAVE_API_KEY`) loaded from the global/local `.env`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebSearchSettings {
    /// "tavily" (default) or "brave". `None` → the built-in default at call time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

/// `[approvals]` block: the user's command-approval policy. Commands whose line
/// matches one of these prefixes run without a confirmation prompt; everything
/// else still asks. `None` means "never configured" → the built-in safe read-only
/// default set applies. `Some(list)` (even empty) is taken as the user's explicit
/// choice. Edited via the `/approvals` checklist. Stored globally by default.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApprovalSettings {
    /// Command-line prefixes that auto-approve (e.g. "git diff", "cargo build").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_approve: Option<Vec<String>>,
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

    /// keep_alive duration passed to Ollama (per request) for this provider connection.
    /// Controls how long the *model used in the request* stays loaded after the call finishes.
    /// Special for local Ollama (ignored or harmless for other providers).
    /// Common values:
    ///   "0s" or 0   — unload immediately after the request (saves VRAM when using many models)
    ///   "30s", "2m" — keep the model hot for a short time (good compromise for role switching)
    ///   "5m", "1h"  — longer keep-alive (Ollama default is often 5m)
    /// If not set for a local-ollama provider, Anvil defaults to "30s" at call time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum CredentialRef {
    /// Stored in OS keyring under service "anvil" and the given entry name.
    /// The actual keyring entry is "anvil:provider:<connection_name>"
    #[default]
    Keyring,

    /// Read from this environment variable at call time.
    Env { var_name: String },

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

/// Machine-wide config shared across every repo: `<OS config dir>/anvil/anvil.toml`
/// (e.g. `%APPDATA%\anvil\anvil.toml` on Windows, `~/.config/anvil/anvil.toml` on
/// Linux). This is the default home for provider/model/role setup so it doesn't
/// have to be redone in each project. Credentials already live globally (keyring).
pub fn global_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("anvil").join(CONFIG_FILE))
}

fn read_config_file(path: &Path) -> Option<AnvilConfig> {
    let raw = std::fs::read_to_string(path).ok()?;
    toml::from_str(&raw).ok()
}

/// Overlay a project config onto the global base: providers + model_bindings
/// extend (project keys win on collision); roles override per-field where the
/// project sets them. The result is what the rest of Anvil sees.
fn merge(mut base: AnvilConfig, overlay: AnvilConfig) -> AnvilConfig {
    base.providers.extend(overlay.providers);
    base.model_bindings.extend(overlay.model_bindings);
    if overlay.roles.coder.is_some() {
        base.roles.coder = overlay.roles.coder;
    }
    if overlay.roles.reviewer_a.is_some() {
        base.roles.reviewer_a = overlay.roles.reviewer_a;
    }
    if overlay.roles.reviewer_b.is_some() {
        base.roles.reviewer_b = overlay.roles.reviewer_b;
    }
    if overlay.web_search.provider.is_some() {
        base.web_search.provider = overlay.web_search.provider;
    }
    // A project that sets its own approval list overrides the global one (a risky
    // repo can tighten what the permissive global list allows).
    if overlay.approvals.auto_approve.is_some() {
        base.approvals.auto_approve = overlay.approvals.auto_approve;
    }
    base
}

/// Load the effective config for `root`: the global config as a base, overlaid
/// by the project's `anvil.toml` if present. Either alone is enough; if neither
/// exists the project is not initialized.
pub fn load_config(root: &Path) -> Result<AnvilConfig, ConfigError> {
    let global = global_config_path().as_deref().and_then(read_config_file);
    let project = read_config_file(&config_path(root));
    match (global, project) {
        (Some(g), Some(p)) => Ok(merge(g, p)),
        (Some(g), None) => Ok(g),
        (None, Some(p)) => Ok(p),
        (None, None) => Err(ConfigError::NotInitialized(root.to_path_buf())),
    }
}

/// Write the global, machine-wide config — the default target for setup so it's
/// shared across all repos.
pub fn save_global_config(cfg: &AnvilConfig) -> Result<(), ConfigError> {
    let path = global_config_path().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "no OS config directory")
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
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

/// Best-effort: ensure the repo's `.gitignore` excludes `.anvil/` so per-session
/// state and logs aren't committed. Idempotent (won't duplicate an existing
/// entry) and a no-op outside a git repo (so we don't litter a `.gitignore`
/// where there's no git). Failures are swallowed — this is housekeeping.
pub fn ensure_anvil_gitignored(root: &Path) {
    // `.git` is a dir in a normal repo, a file in worktrees/submodules — both `exists()`.
    if !root.join(".git").exists() {
        return;
    }
    let path = root.join(".gitignore");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let already = existing
        .lines()
        .any(|l| matches!(l.trim(), ".anvil" | ".anvil/" | "/.anvil" | "/.anvil/"));
    if already {
        return;
    }
    let mut out = existing;
    if !out.is_empty() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n'); // blank line separating our block from prior content
    }
    out.push_str("# Anvil session state (local; safe to delete)\n.anvil/\n");
    let _ = std::fs::write(&path, out);
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
            other => {
                return Err(ConfigError::RoleNotConfigured {
                    role: other.to_string(),
                })
            }
        };
        let name = binding_name.ok_or(ConfigError::RoleNotConfigured {
            role: role.to_string(),
        })?;
        self.get_binding(name)
    }

    /// Returns (binding_name, binding, provider) for a role — convenient for calls.
    pub fn resolve_role_full(
        &self,
        role: &str,
    ) -> Result<(&str, &ModelBinding, &ProviderConnection), ConfigError> {
        let name = match role {
            "coder" | "planner" => self.roles.coder.as_deref(),
            "reviewer-a" | "reviewer_a" => self.roles.reviewer_a.as_deref(),
            "reviewer-b" | "reviewer_b" => self.roles.reviewer_b.as_deref(),
            other => {
                return Err(ConfigError::RoleNotConfigured {
                    role: other.to_string(),
                })
            }
        };
        let name = name.ok_or(ConfigError::RoleNotConfigured {
            role: role.to_string(),
        })?;
        let binding = self.get_binding(name)?;
        let provider = self.get_provider(&binding.provider)?;
        Ok((name, binding, provider))
    }

    /// Resolve a reviewer reference that may be **either** a role keyword
    /// ("reviewer_a"/"reviewer_b") or the bound binding name stored in that role.
    /// Callers in the review pipeline pass the stored binding name, so we try the
    /// role keyword first and then fall back to treating the string as a binding
    /// name directly. Returns (binding_name, binding, provider).
    pub fn resolve_role_or_binding<'a>(
        &'a self,
        key: &'a str,
    ) -> Result<(&'a str, &'a ModelBinding, &'a ProviderConnection), ConfigError> {
        if let Ok(full) = self.resolve_role_full(key) {
            return Ok(full);
        }
        let binding = self.get_binding(key)?;
        let provider = self.get_provider(&binding.provider)?;
        Ok((key, binding, provider))
    }
}

/// Path to the per-project local env file that `anvil` can load automatically.
/// We keep it inside .anvil/ so it stays with the rest of the project's anvil artifacts
/// and can be .gitignored easily.
fn local_env_path(root: &Path) -> PathBuf {
    anvil_dir(root).join(".env")
}

/// Loads a very simple KEY=val (or KEY="val") file from `.anvil/.env` (if present)
/// into the current process environment using `std::env::set_var`.
///
/// - Only sets a variable if it is **not** already present in the environment.
///   This respects variables coming from the outer shell, CI system, Docker -e, etc.
/// - Comments (`# ...`) and blank lines are ignored.
/// - This is deliberately minimal (no new dependencies, no full dotenv spec).
/// - Called early by the TUI and all CLI commands that may need credentials.
///
/// This gives a cross-platform, cross-shell "paste the key once during /config and
/// future `anvil` runs in this directory just work" experience without requiring
/// users to edit shell profiles on Windows, Linux, macOS, WSL, etc.
pub fn load_local_env(root: &Path) {
    // The repo-local .env first so it can override, then the GLOBAL .env (next to
    // the global config) as the shared base. set_var-if-absent means the outer
    // shell wins over both, and the local file wins over the global one.
    load_env_file(&local_env_path(root));
    if let Some(g) = global_env_path() {
        load_env_file(&g);
    }
}

/// The shared, machine-wide credential file: `<OS config dir>/anvil/.env`. Lets
/// env-var keys (e.g. ANTHROPIC_API_KEY) be set once for every repo.
pub fn global_env_path() -> Option<PathBuf> {
    global_config_path().and_then(|p| p.parent().map(|d| d.join(".env")))
}

fn load_env_file(path: &Path) {
    if !path.exists() {
        return;
    }
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(eq) = line.find('=') {
            let key = line[..eq].trim();
            if key.is_empty() {
                continue;
            }
            let mut val = line[eq + 1..].trim().to_string();
            // Strip a single pair of matching outer quotes (common when people copy examples)
            if ((val.starts_with('"') && val.ends_with('"'))
                || (val.starts_with('\'') && val.ends_with('\'')))
                && val.len() >= 2
            {
                val = val[1..val.len() - 1].to_string();
            }
            // Only set if the outer environment didn't already provide it
            if std::env::var(key).is_err() {
                std::env::set_var(key, val);
            }
        }
    }
}

/// Persists `key=value` into `.anvil/.env` (creating the directory and file as needed)
/// and also calls `std::env::set_var` so the current process sees it immediately.
///
/// This is used by the interactive "add provider" wizard: when you paste an API key,
/// we store the *name* of the variable in anvil.toml (as CredentialRef::Env) **and**
/// write the actual secret to the local .env file + inject it into the running process.
///
/// Result: after one paste in the TUI, chat/plan/etc. work right away, *and* future
/// invocations of `anvil` from the same project directory will pick up the secret
/// from the file with no further shell configuration on any OS.
pub fn set_local_env_var(root: &Path, key: &str, value: &str) {
    // Make sure the *current* process (the TUI or CLI command) can see it right now.
    std::env::set_var(key, value);

    let dir = match ensure_anvil_dir(root) {
        Ok(d) => d,
        Err(_) => return,
    };
    let path = dir.join(".env");

    // Read existing content so we can update in place instead of always appending duplicates.
    let mut lines: Vec<String> = if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(c) => c.lines().map(|s| s.to_string()).collect(),
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };

    let prefix = format!("{}=", key);
    let mut replaced = false;
    for line in &mut lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with(&prefix) || trimmed.starts_with(&format!("{} =", key)) {
            *line = format!("{}={}", key, value);
            replaced = true;
            break;
        }
    }
    if !replaced {
        if !lines.is_empty() {
            if let Some(last) = lines.last() {
                if !last.trim().is_empty() {
                    lines.push(String::new());
                }
            }
        }
        lines.push(format!("{}={}", key, value));
    }

    let _ = std::fs::write(&path, lines.join("\n") + "\n");

    // Best-effort restrictive permissions on Unix-like systems.
    // On Windows the file is typically only readable by the current user anyway,
    // but we still try.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(&path, perms);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gitignore_added_then_idempotent_and_skipped_without_git() {
        // No .git → no .gitignore written.
        let no_git = tempfile::tempdir().unwrap();
        ensure_anvil_gitignored(no_git.path());
        assert!(!no_git.path().join(".gitignore").exists());

        // With .git → entry appended once, preserving existing content.
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir(repo.path().join(".git")).unwrap();
        std::fs::write(repo.path().join(".gitignore"), "target/\n").unwrap();
        ensure_anvil_gitignored(repo.path());
        let gi = std::fs::read_to_string(repo.path().join(".gitignore")).unwrap();
        assert!(gi.contains("target/"), "{gi}");
        assert!(gi.contains(".anvil/"), "{gi}");

        // Second call is a no-op (no duplicate).
        ensure_anvil_gitignored(repo.path());
        let gi2 = std::fs::read_to_string(repo.path().join(".gitignore")).unwrap();
        assert_eq!(gi, gi2);
        assert_eq!(gi2.matches(".anvil/").count(), 1, "{gi2}");
    }

    /// Mirrors the config the role-assignment wizard produces: the role stores the
    /// binding name, and a model_binding + provider exist under that name.
    fn config_with_reviewer_named(binding: &str) -> AnvilConfig {
        let mut cfg = AnvilConfig::default();
        cfg.providers.insert(
            "local-ollama".to_string(),
            ProviderConnection {
                r#type: "openai_compat".to_string(),
                base_url: Some("http://localhost:11434/v1".to_string()),
                credential: CredentialRef::None,
                extra: Default::default(),
                keep_alive: Some("30s".to_string()),
            },
        );
        cfg.model_bindings.insert(
            binding.to_string(),
            ModelBinding {
                provider: "local-ollama".to_string(),
                model: binding.to_string(),
                note: None,
            },
        );
        cfg.roles.reviewer_a = Some(binding.to_string());
        cfg
    }

    #[test]
    fn resolve_reviewer_by_binding_name() {
        // Regression: /lock-plan passes the bound binding name (e.g. the model tag)
        // rather than the "reviewer_a" keyword. Both must resolve.
        let cfg = config_with_reviewer_named("qwen2.5-coder:32b");

        // The way the review pipeline actually calls it (binding name):
        let (name, binding, provider) = cfg
            .resolve_role_or_binding("qwen2.5-coder:32b")
            .expect("binding-name form should resolve");
        assert_eq!(name, "qwen2.5-coder:32b");
        assert_eq!(binding.model, "qwen2.5-coder:32b");
        assert_eq!(provider.r#type, "openai_compat");

        // The role-keyword form must keep working too.
        assert!(cfg.resolve_role_or_binding("reviewer_a").is_ok());
    }

    #[test]
    fn resolve_reviewer_unknown_fails() {
        let cfg = config_with_reviewer_named("qwen2.5-coder:32b");
        assert!(cfg.resolve_role_or_binding("does-not-exist").is_err());
    }
}
