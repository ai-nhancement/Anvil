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

    #[error(
        "unknown config key '{0}' — valid keys: sidecar.idle_timeout_secs, sidecar.binary_path"
    )]
    UnknownConfigKey(String),

    #[error("invalid value for '{key}': {reason}")]
    InvalidConfigValue { key: String, reason: String },

    #[error("record already exists: {id}")]
    RecordAlreadyExists { id: String },

    #[error("record not found: {id}")]
    RecordNotFound { id: String },

    #[error("audit index corrupted at {path}: {reason}")]
    IndexCorrupted { path: PathBuf, reason: String },

    #[error("invalid record type: '{0}'")]
    InvalidRecordType(String),

    #[error("invalid cross-reference key '{0}' — expected format: path:section:version (no colons in fields)")]
    InvalidCrossRefKey(String),

    #[error(
        "invalid record id '{0}' — ids must not contain path separators or control characters"
    )]
    InvalidRecordId(String),

    #[error("utf-8 violation in record '{id}': {source}")]
    RecordUtf8Error {
        id: String,
        #[source]
        source: std::str::Utf8Error,
    },

    #[error("adversarial diversity policy violated: {0}")]
    DiversityViolation(String),

    #[error(
        "anvil-sidecar binary not found — set sidecar.binary_path in anvil.toml or add it to $PATH"
    )]
    SidecarNotFound,

    #[error("timed out waiting for sidecar to become ready (checked .anvil/run/sidecar.port)")]
    SidecarStartTimeout,

    #[error("setup cancelled")]
    SetupCancelled,

    #[error("OS keychain unavailable: {0}")]
    KeychainUnavailable(String),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
