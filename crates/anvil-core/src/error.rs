use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AnvilError {
    #[error("config parse error in '{path}': {source}")]
    ConfigParse {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },

    #[error("config serialize error: {0}")]
    ConfigSerialize(#[from] toml::ser::Error),

    #[error("provisional choice '{key}' is missing required field '{field}'")]
    ProvisionalMissingField { key: String, field: &'static str },

    #[error("charter file '{0}' is empty — a valid charter is required")]
    EmptyCharter(PathBuf),

    #[error("no anvil.toml found in {0} — run `anvil init` first")]
    NotInitialized(PathBuf),

    #[error("unknown config key '{0}' — valid keys: sidecar.idle_timeout_secs, sidecar.binary_path")]
    UnknownConfigKey(String),

    #[error("invalid value for '{key}': {reason}")]
    InvalidConfigValue { key: String, reason: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
