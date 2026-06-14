use std::path::Path;

use serde::{Deserialize, Serialize};

use anvil_core::error::AnvilError;

/// A single entry in `audit-store/_index.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: String,
    pub record_type: String,
    /// Path relative to the project root (e.g. `audit-store/gate-approval/uuid.json`).
    pub file_path: String,
}

/// In-memory representation of `audit-store/_index.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditIndex {
    pub records: Vec<IndexEntry>,
}

impl AuditIndex {
    /// Reads and deserializes the index from `path`.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::Io`] if the file cannot be read, or
    /// [`AnvilError::IndexCorrupted`] if the bytes are not valid UTF-8 or valid JSON.
    pub fn load(path: &Path) -> Result<Self, AnvilError> {
        let bytes = std::fs::read(path)?;
        let s = std::str::from_utf8(&bytes).map_err(|_| AnvilError::IndexCorrupted {
            path: path.to_path_buf(),
            reason: "index file is not valid UTF-8".to_owned(),
        })?;
        serde_json::from_str(s).map_err(|e| AnvilError::IndexCorrupted {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })
    }

    /// Appends `entry` to the in-memory records list.
    pub fn append_entry(&mut self, entry: IndexEntry) {
        self.records.push(entry);
    }

    /// Serializes `index` and atomically replaces `path` via a unique-named `.tmp` + rename.
    ///
    /// Each call generates a fresh UUID-based temp filename
    /// (`_index.json.<uuid>.tmp`) so concurrent writers never share a temp path.
    /// The temp file is left on disk only if the rename fails; `AuditStore::open`
    /// cleans up all `_index.json.*.tmp` orphans on next startup.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::Json`] on serialization failure or [`AnvilError::Io`] on write failure.
    ///
    /// # Panics
    ///
    /// Panics if `path` has no parent directory, which cannot happen for paths produced
    /// by `AuditStore` (all index paths are `<audit-root>/_index.json`).
    pub fn save_atomic(path: &Path, index: &Self) -> Result<(), AnvilError> {
        let serialized = serde_json::to_string_pretty(index)?;
        let tmp_name = format!("_index.json.{}.tmp", uuid::Uuid::new_v4());
        let tmp_path = path
            .parent()
            .expect("index path must have a parent directory")
            .join(tmp_name);
        std::fs::write(&tmp_path, serialized.as_bytes())?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }
}
