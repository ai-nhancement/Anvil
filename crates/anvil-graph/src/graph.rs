use std::collections::HashMap;

use anvil_audit::store::AuditStore;
use anvil_audit::records::ALL_RECORD_TYPES;
use anvil_core::error::AnvilError;

use crate::CrossRefKey;

/// Queryable provenance graph built from the audit store.
///
/// Maps each cross-reference key to the IDs of all records that back it.
/// Built by scanning every record in the store; rebuild after new appends.
pub struct ProvenanceGraph {
    /// cross-ref key string → vec of record IDs
    edges: HashMap<String, Vec<String>>,
}

impl ProvenanceGraph {
    /// Scans all records in `store` and builds the provenance graph.
    ///
    /// # Errors
    ///
    /// Returns [`AnvilError::IndexCorrupted`] or [`AnvilError::Io`] if the index
    /// cannot be read, or [`AnvilError::RecordUtf8Error`] / [`AnvilError::Json`]
    /// if a record file is malformed.
    pub fn build(store: &AuditStore) -> Result<Self, AnvilError> {
        let mut edges: HashMap<String, Vec<String>> = HashMap::new();

        for record_type in ALL_RECORD_TYPES {
            let entries = store.list(record_type)?;
            for entry in entries {
                let value = store.get(&entry.id)?;
                // cross_references must be an array of strings; anything else is a
                // malformed record that would silently produce incorrect unbacked results.
                match value.get("cross_references") {
                    None => {}
                    Some(v) => {
                        let refs = v.as_array().ok_or_else(|| AnvilError::IndexCorrupted {
                            path: std::path::PathBuf::from(&entry.file_path),
                            reason: format!(
                                "record '{}': cross_references is not an array",
                                entry.id
                            ),
                        })?;
                        for r in refs {
                            let key_str =
                                r.as_str().ok_or_else(|| AnvilError::IndexCorrupted {
                                    path: std::path::PathBuf::from(&entry.file_path),
                                    reason: format!(
                                        "record '{}': cross_references contains a non-string element",
                                        entry.id
                                    ),
                                })?;
                            edges
                                .entry(key_str.to_owned())
                                .or_default()
                                .push(entry.id.clone());
                        }
                    }
                }
            }
        }

        Ok(Self { edges })
    }

    /// Returns the IDs of all records that back the given cross-reference key.
    /// An empty slice means no records back this section yet.
    #[must_use]
    pub fn records_for_key(&self, key: &CrossRefKey) -> &[String] {
        self.edges
            .get(&key.to_key_string())
            .map_or(&[][..], Vec::as_slice)
    }

    /// Returns `true` if at least one record backs the given cross-reference key.
    #[must_use]
    pub fn is_backed(&self, key: &CrossRefKey) -> bool {
        !self.records_for_key(key).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tempfile::TempDir;

    use anvil_audit::records::{ALL_RECORD_TYPES, GateApproval};
    use anvil_audit::store::AuditStore;

    use super::*;

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

    #[test]
    fn test_provenance_graph_resolves_backing_records() {
        let (_tmp, store) = init_test_store();

        let record = GateApproval {
            id: "graph-test-id".to_owned(),
            created_at: Utc::now(),
            cross_references: vec!["plan.md:§p1:v1".to_owned()],
            gate_name: "plan-stage".to_owned(),
            approver: "john".to_owned(),
        };
        store.append(&record).expect("append");

        let graph = ProvenanceGraph::build(&store).expect("build");
        let key = CrossRefKey::new("plan.md", "§p1", "v1");

        let backing = graph.records_for_key(&key);
        assert_eq!(backing.len(), 1);
        assert_eq!(backing[0], "graph-test-id");
        assert!(graph.is_backed(&key));
    }

    #[test]
    fn test_provenance_graph_returns_empty_for_unbacked_key() {
        let (_tmp, store) = init_test_store();
        let graph = ProvenanceGraph::build(&store).expect("build");
        let key = CrossRefKey::new("charter.md", "§never-referenced", "v1");
        assert!(graph.records_for_key(&key).is_empty());
        assert!(!graph.is_backed(&key));
    }
}
