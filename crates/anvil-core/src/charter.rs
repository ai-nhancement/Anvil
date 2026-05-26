//! Charter file utilities.
//!
//! [`CharterPacket`] and related types live in [`crate::pipeline`] (P5).
//! This module handles the file-level `charter.md` operations.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::AnvilError;

/// Metadata extracted from a `charter.md` file.
#[derive(Debug, Clone)]
pub struct CharterMetadata {
    /// Hex-encoded SHA-256 of the raw file bytes.
    pub content_hash: String,
    /// Raw byte length of the file.
    pub byte_len: usize,
}

/// Reads the file at `path`, computes its SHA-256 hash, and returns metadata.
///
/// # Errors
///
/// Returns [`AnvilError::Io`] if the file cannot be read, or
/// [`AnvilError::EmptyCharter`] if the file is zero bytes.
pub fn load_charter(path: &Path) -> Result<CharterMetadata, AnvilError> {
    let bytes = std::fs::read(path)?;
    if bytes.is_empty() {
        return Err(AnvilError::EmptyCharter(path.to_path_buf()));
    }
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hash = format!("{:x}", hasher.finalize());
    Ok(CharterMetadata {
        content_hash: hash,
        byte_len: bytes.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_charter_rejects_empty_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("anvil_test_empty_charter.md");
        std::fs::write(&path, b"").unwrap();
        let err = load_charter(&path).unwrap_err();
        assert!(
            matches!(err, AnvilError::EmptyCharter(_)),
            "expected EmptyCharter, got: {err}"
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_charter_returns_hash_for_nonempty_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("anvil_test_nonempty_charter.md");
        std::fs::write(&path, b"# Charter\n").unwrap();
        let meta = load_charter(&path).expect("should succeed");
        assert!(meta.byte_len > 0);
        assert_eq!(meta.content_hash.len(), 64); // SHA-256 hex is always 64 chars
        std::fs::remove_file(&path).ok();
    }
}
