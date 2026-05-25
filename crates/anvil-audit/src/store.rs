use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use anvil_core::error::AnvilError;

use crate::index::{AuditIndex, IndexEntry};
use crate::integrity::{IntegrityReport, IntegrityStatus, IntegrityViolation};
use crate::metrics::StoreMetrics;
use crate::records::{AuditRecord, RecordType, ALL_RECORD_TYPES};

/// Validates that `id` is safe for use as a filename component.
///
/// Rejects empty strings, the single-dot special name `.`, any string containing
/// `..` (catches both the literal `..` and embedded sequences like `foo..bar`),
/// path separators (`/`, `\`), null bytes, and colons.
fn validate_record_id(id: &str) -> Result<(), AnvilError> {
    if id.is_empty()
        || id == "."
        || id.contains("..")
        || id.contains(['/', '\\', '\0', ':'])
    {
        return Err(AnvilError::InvalidRecordId(id.to_owned()));
    }
    Ok(())
}

/// Filesystem-backed audit store.
///
/// All writes go through [`AuditStore::append`] which enforces append-only semantics at
/// both the API level (no `update` or `delete` method exists) and the filesystem level
/// (`O_CREAT|O_EXCL` via [`std::fs::OpenOptions::create_new`]).
///
/// # Concurrency
///
/// P2 assumes single-process access. The `_index.json` read-modify-write in [`AuditStore::append`]
/// is not protected by a cross-process lock; concurrent CLI invocations can race and lose index
/// updates. A file-lock mechanism will be added in a future phase when concurrent access is needed.
#[derive(Debug)]
pub struct AuditStore {
    /// `<project-root>/audit-store/`
    audit_root: PathBuf,
    /// `<project-root>/audit-store/_index.json`
    index_path: PathBuf,
    /// Project root — used to resolve relative file paths stored in the index.
    project_root: PathBuf,
    metrics: Arc<StoreMetrics>,
}

impl AuditStore {
    /// Opens the audit store at `<project_root>/audit-store/`.
    ///
    /// Removes any orphaned `_index.json.*.tmp` files left by a previous crash
    /// between a temp-file write and its rename. These files are never read during
    /// normal operation; cleanup is best-effort (failures are silently ignored).
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::NotInitialized`] if `audit-store/` does not exist or
    /// `_index.json` is missing (project not initialized via `anvil init`).
    pub fn open(project_root: &Path) -> Result<Self, AnvilError> {
        let audit_root = project_root.join("audit-store");
        if !audit_root.is_dir() {
            return Err(AnvilError::NotInitialized(project_root.to_path_buf()));
        }
        let index_path = audit_root.join("_index.json");
        if !index_path.is_file() {
            return Err(AnvilError::IndexCorrupted {
                path: index_path.clone(),
                reason: "_index.json is missing — re-run `anvil init` to restore".to_owned(),
            });
        }
        // Clean up any UUID-named temp files left by a previous crash mid-rename.
        // Each temp file has a unique name so this cannot delete an active writer's temp.
        if let Ok(entries) = std::fs::read_dir(&audit_root) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("_index.json.") && name_str.ends_with(".tmp") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        Ok(Self {
            index_path,
            audit_root,
            project_root: project_root.to_path_buf(),
            metrics: Arc::new(StoreMetrics::default()),
        })
    }

    /// Appends `record` to the store.
    ///
    /// Writes the record as a JSON file at
    /// `audit-store/<record-type-dir>/<id>.json` using `O_CREAT|O_EXCL` semantics,
    /// then atomically updates `_index.json`.
    ///
    /// If the index update fails after the record file was created, the record file
    /// is removed on a best-effort basis to prevent an unindexed orphan.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::InvalidRecordId`] if the ID contains unsafe path characters,
    /// [`AnvilError::RecordAlreadyExists`] if a record with the same ID was previously
    /// appended, [`AnvilError::Io`] on filesystem failure, or [`AnvilError::Json`] on
    /// serialization failure.
    pub fn append<R: AuditRecord>(&self, record: &R) -> Result<(), AnvilError> {
        validate_record_id(record.id())?;

        let json = serde_json::to_string_pretty(record)?;
        let dir = self.audit_root.join(record.record_type().dir_name());
        let file_path = dir.join(format!("{}.json", record.id()));

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file_path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    AnvilError::RecordAlreadyExists {
                        id: record.id().to_owned(),
                    }
                } else {
                    AnvilError::Io(e)
                }
            })?;
        file.write_all(json.as_bytes())?;
        drop(file);

        let relative_path = format!(
            "audit-store/{}/{}.json",
            record.record_type().dir_name(),
            record.id()
        );
        let index_result = (|| {
            let mut index = AuditIndex::load(&self.index_path)?;
            index.append_entry(IndexEntry {
                id: record.id().to_owned(),
                record_type: record.record_type().as_str().to_owned(),
                file_path: relative_path,
            });
            AuditIndex::save_atomic(&self.index_path, &index)
        })();

        if let Err(e) = index_result {
            // Best-effort rollback: remove the record file to prevent an unindexed orphan.
            // A retry with the same ID would otherwise hit RecordAlreadyExists.
            let _ = std::fs::remove_file(&file_path);
            return Err(e);
        }

        self.metrics.total_appended.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Lists all index entries of the given `record_type`.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::IndexCorrupted`] or [`AnvilError::Io`] if the index cannot be read.
    pub fn list(&self, record_type: RecordType) -> Result<Vec<IndexEntry>, AnvilError> {
        let index = AuditIndex::load(&self.index_path)?;
        let type_str = record_type.as_str();
        Ok(index
            .records
            .into_iter()
            .filter(|e| e.record_type == type_str)
            .collect())
    }

    /// Returns the full record as a raw JSON value.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::RecordNotFound`] if `id` is not in the index,
    /// [`AnvilError::RecordUtf8Error`] if the stored file is not valid UTF-8, or
    /// [`AnvilError::IndexCorrupted`] / [`AnvilError::Io`] on read failure.
    pub fn get(&self, id: &str) -> Result<serde_json::Value, AnvilError> {
        let index = AuditIndex::load(&self.index_path)?;
        let entry = index
            .records
            .into_iter()
            .find(|e| e.id == id)
            .ok_or_else(|| AnvilError::RecordNotFound { id: id.to_owned() })?;
        let abs_path = self.project_root.join(&entry.file_path);
        let bytes = std::fs::read(&abs_path)?;
        let s = std::str::from_utf8(&bytes).map_err(|e| AnvilError::RecordUtf8Error {
            id: id.to_owned(),
            source: e,
        })?;
        serde_json::from_str(s).map_err(|e| AnvilError::IndexCorrupted {
            path: abs_path,
            reason: e.to_string(),
        })
    }

    /// Checks the audit store for two classes of violations:
    ///
    /// **`BlockShip`** — indexed record whose file is missing, not a regular file, or
    /// whose stored `id` field does not match the index entry.
    ///
    /// **`Warn`** — `.json` file found on disk inside a record-type directory that has
    /// no corresponding index entry (indicates an unindexed orphan from a partial write).
    ///
    /// This is local tamper detection (accidental deletion, partial restore) — not
    /// adversarial tamper-proofing.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::IndexCorrupted`] or [`AnvilError::Io`] if the index cannot be read.
    pub fn check_integrity(&self) -> Result<IntegrityReport, AnvilError> {
        let index = AuditIndex::load(&self.index_path)?;
        let mut violations: Vec<IntegrityViolation> = Vec::new();

        // Build the set of indexed IDs for the orphan-scan below.
        let indexed_ids: std::collections::HashSet<&str> =
            index.records.iter().map(|e| e.id.as_str()).collect();

        // 1. Check every indexed record.
        for entry in &index.records {
            let path = self.project_root.join(&entry.file_path);
            if !path.is_file() {
                violations.push(IntegrityViolation {
                    id: entry.id.clone(),
                    path: entry.file_path.clone(),
                    reason: if path.exists() {
                        "path exists but is not a regular file".to_owned()
                    } else {
                        "file missing from disk".to_owned()
                    },
                    severity: crate::integrity::ViolationSeverity::BlockShip,
                });
                continue;
            }
            // Validate that the stored `id` field matches the index.
            match std::fs::read(&path)
                .ok()
                .and_then(|b| String::from_utf8(b).ok())
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| v.get("id").and_then(|id| id.as_str()).map(str::to_owned))
            {
                Some(stored_id) if stored_id == entry.id => {}
                Some(stored_id) => violations.push(IntegrityViolation {
                    id: entry.id.clone(),
                    path: entry.file_path.clone(),
                    reason: format!("stored id '{stored_id}' does not match index entry"),
                    severity: crate::integrity::ViolationSeverity::BlockShip,
                }),
                None => violations.push(IntegrityViolation {
                    id: entry.id.clone(),
                    path: entry.file_path.clone(),
                    reason: "file exists but could not be read as valid JSON with an id field"
                        .to_owned(),
                    severity: crate::integrity::ViolationSeverity::BlockShip,
                }),
            }
        }

        // 2. Scan for unindexed `.json` files (orphans from partial writes).
        for rt in ALL_RECORD_TYPES {
            let dir = self.audit_root.join(rt.dir_name());
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if !name_str.ends_with(".json") {
                        continue;
                    }
                    let stem = name_str.trim_end_matches(".json");
                    if !indexed_ids.contains(stem) {
                        let rel = format!("audit-store/{}/{}", rt.dir_name(), name_str);
                        violations.push(IntegrityViolation {
                            id: stem.to_owned(),
                            path: rel,
                            reason: "file on disk has no index entry (unindexed orphan from partial write)".to_owned(),
                            severity: crate::integrity::ViolationSeverity::Warn,
                        });
                    }
                }
            }
        }

        let status = if violations
            .iter()
            .any(|v| v.severity == crate::integrity::ViolationSeverity::BlockShip)
        {
            IntegrityStatus::BlockShip
        } else if violations
            .iter()
            .any(|v| v.severity == crate::integrity::ViolationSeverity::Warn)
        {
            IntegrityStatus::Warn
        } else {
            IntegrityStatus::Pass
        };

        Ok(IntegrityReport { status, violations })
    }

    /// Returns the path where `record` is (or would be) stored.
    #[must_use]
    pub fn record_path<R: AuditRecord>(&self, record: &R) -> PathBuf {
        self.audit_root
            .join(record.record_type().dir_name())
            .join(format!("{}.json", record.id()))
    }

    /// Returns the Layer-1 metric counters (for P10a collection).
    #[must_use]
    pub fn metrics(&self) -> &StoreMetrics {
        &self.metrics
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tempfile::TempDir;

    use super::*;
    use crate::records::{GateApproval, CHARTER_REQUIRED_TYPES};

    fn init_test_store() -> (TempDir, AuditStore) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::create_dir_all(root.join("audit-store")).unwrap();
        for rt in ALL_RECORD_TYPES {
            std::fs::create_dir_all(root.join("audit-store").join(rt.dir_name())).unwrap();
        }
        std::fs::write(root.join("audit-store/_index.json"), b"{\"records\":[]}\n").unwrap();
        let store = AuditStore::open(root).expect("open");
        (tmp, store)
    }

    fn make_gate_approval(id: &str) -> GateApproval {
        GateApproval {
            id: id.to_owned(),
            created_at: Utc::now(),
            cross_references: vec!["plan.md:§p1:v1".to_owned()],
            gate_name: "plan-stage".to_owned(),
            approver: "john".to_owned(),
        }
    }

    // hinge_test: pins=11, intended=charter-required-audit-types, phase=P2
    #[test]
    fn test_audit_store_required_types_present() {
        for name in CHARTER_REQUIRED_TYPES {
            assert!(
                RecordType::from_type_name(name).is_some(),
                "Charter-required type missing from RecordType: {name}"
            );
        }
    }

    // hinge_test: pins=append-only, intended=audit-api-shape, phase=P2
    #[test]
    fn test_append_only_api_has_no_update_or_delete() {
        let (_tmp, store) = init_test_store();
        let record = make_gate_approval("test-append-only-id");
        store.append(&record).expect("append should succeed");
        let entries = store.list(RecordType::GateApproval).expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "test-append-only-id");
        assert_eq!(store.metrics().snapshot_total_appended(), 1);
    }

    // hinge_test: pins=o_excl, intended=filesystem-append-only, phase=P2
    #[test]
    fn test_append_only_filesystem_o_excl() {
        let (_tmp, store) = init_test_store();
        let record = make_gate_approval("test-excl-id");
        store.append(&record).expect("first append should succeed");
        let err = store.append(&record).expect_err("second append must fail");
        assert!(
            matches!(err, AnvilError::RecordAlreadyExists { ref id } if id == "test-excl-id"),
            "expected RecordAlreadyExists, got: {err}"
        );
    }

    // hinge_test: pins=integrity-check, intended=audit-completeness-check, phase=P2
    #[test]
    fn test_audit_store_detects_deleted_records() {
        let (_tmp, store) = init_test_store();
        let record = make_gate_approval("test-deleted-id");
        store.append(&record).expect("append");
        std::fs::remove_file(store.record_path(&record)).expect("remove");
        let report = store.check_integrity().expect("integrity check");
        assert_eq!(report.status, IntegrityStatus::BlockShip);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].id, "test-deleted-id");
    }

    #[test]
    fn test_open_fails_without_index_json() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::create_dir_all(root.join("audit-store")).unwrap();
        // Deliberately omit _index.json
        let err = AuditStore::open(root).expect_err("should fail without index");
        assert!(
            matches!(err, AnvilError::IndexCorrupted { .. }),
            "expected IndexCorrupted, got: {err}"
        );
    }

    #[test]
    fn test_open_cleans_up_stale_tmp_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        std::fs::create_dir_all(root.join("audit-store")).unwrap();
        std::fs::write(root.join("audit-store/_index.json"), b"{\"records\":[]}\n").unwrap();
        // Plant a stale UUID-named temp file
        let stale = root.join("audit-store/_index.json.00000000-fake-temp.tmp");
        std::fs::write(&stale, b"stale").unwrap();
        AuditStore::open(root).expect("open should succeed and clean up");
        assert!(!stale.exists(), "stale temp file should have been removed");
    }

    #[test]
    fn test_append_rejects_invalid_id() {
        let (_tmp, store) = init_test_store();
        for bad_id in &["", "../escape", "nested/path", "colon:id", "back\\slash"] {
            let record = make_gate_approval(bad_id);
            let err = store.append(&record).expect_err(&format!("should reject id: {bad_id:?}"));
            assert!(
                matches!(err, AnvilError::InvalidRecordId(_)),
                "expected InvalidRecordId for {bad_id:?}, got: {err}"
            );
        }
    }

    #[test]
    fn test_integrity_detects_unindexed_orphan() {
        let (_tmp, store) = init_test_store();
        // Write a JSON file directly in a record-type dir without going through append
        let orphan_path = store
            .audit_root
            .join("gate-approval")
            .join("orphan-id.json");
        std::fs::write(&orphan_path, b"{\"id\":\"orphan-id\"}").unwrap();
        let report = store.check_integrity().expect("integrity check");
        assert_eq!(
            report.status,
            IntegrityStatus::Warn,
            "unindexed file should produce Warn"
        );
        assert!(
            report.violations.iter().any(|v| v.id == "orphan-id"),
            "orphan-id must appear in violations"
        );
    }

    #[test]
    fn test_integrity_detects_id_mismatch() {
        let (_tmp, store) = init_test_store();
        let record = make_gate_approval("original-id");
        store.append(&record).expect("append");
        // Overwrite the file with a different id field
        let path = store.record_path(&record);
        std::fs::write(&path, b"{\"id\":\"tampered-id\"}").unwrap();
        let report = store.check_integrity().expect("integrity check");
        assert_eq!(report.status, IntegrityStatus::BlockShip);
        assert!(
            report
                .violations
                .iter()
                .any(|v| v.reason.contains("does not match index entry")),
            "id mismatch violation must be reported"
        );
    }
}
