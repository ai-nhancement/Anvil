use std::path::{Path, PathBuf};

use crate::config::{save_config, AnvilConfig};
use crate::error::AnvilError;

/// Non-record-type directories that appear before the `audit-store/` record subdirs.
const BEFORE_AUDIT_STORE_DIRS: &[&str] = &["phases", "audit-store"];

/// Non-record-type directories that appear after the `audit-store/` record subdirs.
const AFTER_AUDIT_STORE_DIRS: &[&str] = &[".anvil", ".anvil/run", ".anvil/logs"];

/// Subdirectory names (without the `audit-store/` prefix) for all 15 audit record types.
///
/// This is the single source of truth for the names of the per-type record directories.
/// The full `audit-store/<name>` paths are created by [`init`] and returned by [`layout_dirs`].
///
/// **Coupling constraint:** these names must match `RecordType::dir_name()` in `anvil-audit` for
/// every variant in `ALL_RECORD_TYPES`. The invariant is enforced at test time by
/// `test_all_record_type_dirs_covered_by_layout_dirs` in `anvil-audit`.
///
/// Cannot be derived from `ALL_RECORD_TYPES` at compile time because `anvil-core` cannot
/// import `anvil-audit` (would create a circular dependency: `anvil-audit` depends on
/// `anvil-core` for error types).
pub const AUDIT_RECORD_DIR_NAMES: &[&str] = &[
    "reviewer-finding-packet",
    "verifier-result",
    "rotation-log",
    "charter-amendment",
    "plan-amendment",
    "phase-disposition",
    "hinge-flip",
    "gate-approval",
    "convergence-declaration",
    "provisional-lock",
    "rollback-event",
    "arbiter-finding-resolution",
    "sidecar-reload",
    "curated-findings",
    "plan-consolidation",
];

/// Returns the full ordered list of directories created by [`init`], relative to the project root.
///
/// The list is derived: [`BEFORE_AUDIT_STORE_DIRS`] + `"audit-store/"` + each
/// [`AUDIT_RECORD_DIR_NAMES`] entry + [`AFTER_AUDIT_STORE_DIRS`].
///
/// Adding a new audit record type requires adding its dir name to [`AUDIT_RECORD_DIR_NAMES`];
/// the invariant test in `anvil-audit` will enforce the coupling to `RecordType::dir_name()`.
#[must_use]
pub fn layout_dirs() -> Vec<String> {
    let mut dirs: Vec<String> = BEFORE_AUDIT_STORE_DIRS
        .iter()
        .map(|&s| s.to_string())
        .collect();
    for name in AUDIT_RECORD_DIR_NAMES {
        dirs.push(format!("audit-store/{name}"));
    }
    dirs.extend(AFTER_AUDIT_STORE_DIRS.iter().map(|&s| s.to_string()));
    dirs
}

/// Outcome of [`init`].
pub enum InitResult {
    /// First initialization: directories and `anvil.toml` were created.
    Initialized { root: PathBuf, dirs_created: usize },
    /// Project was already initialized (`anvil.toml` already present).
    AlreadyInitialized { root: PathBuf },
}

/// Idempotent project initialization.
///
/// If `anvil.toml` already exists at `root`, returns `AlreadyInitialized` without
/// modifying any state. Otherwise creates the full directory layout, writes a default
/// `anvil.toml`, and creates empty placeholder files.
///
/// # Errors
///
/// Returns [`AnvilError::Io`] on any filesystem failure, or
/// [`AnvilError::ConfigSerialize`] / [`AnvilError::ProvisionalMissingField`] if the
/// default config cannot be written.
pub fn init(root: &Path) -> Result<InitResult, AnvilError> {
    let config_path = root.join("anvil.toml");
    if config_path.exists() {
        return Ok(InitResult::AlreadyInitialized {
            root: root.to_path_buf(),
        });
    }

    std::fs::create_dir_all(root)?;

    let dirs = layout_dirs();
    for dir in &dirs {
        std::fs::create_dir_all(root.join(dir))?;
    }

    // Empty audit-store index.
    let index_path = root.join("audit-store/_index.json");
    std::fs::write(&index_path, b"{\"records\":[]}\n")?;

    // Placeholder project files (empty; filled in during Charter and Plan stages).
    for name in &[
        "charter.md",
        "plan.md",
        "CHARTER_HARDENING_HISTORY.md",
        "PLAN_HARDENING_HISTORY.md",
    ] {
        let p = root.join(name);
        if !p.exists() {
            std::fs::write(&p, b"")?;
        }
    }

    let config = AnvilConfig::default_locked();
    save_config(root, &config)?;

    Ok(InitResult::Initialized {
        root: root.to_path_buf(),
        dirs_created: dirs.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // hinge_test: pins=20, intended=project-layout-directories, phase=P5
    #[test]
    fn test_project_layout_directories() {
        // Pins: the per-project directory layout has exactly 20 entries
        // (2 pre-audit + 15 audit-record + 3 post-audit).
        //
        // Previously maintained as a hardcoded LAYOUT_DIRS constant; refactored in P9 R3 so
        // audit-record dir names live solely in AUDIT_RECORD_DIR_NAMES (single source of truth)
        // and layout_dirs() derives the full list. The coupling from AUDIT_RECORD_DIR_NAMES to
        // RecordType::dir_name() is enforced by test_all_record_type_dirs_covered_by_layout_dirs
        // in anvil-audit.
        //
        // Adding/removing a directory or renaming an entry requires updating
        // AUDIT_RECORD_DIR_NAMES (or the before/after constants) and this count; it is a
        // breaking change for existing initialized projects.
        let dirs = layout_dirs();
        // Hard-pin to 20: 2 pre-audit + 15 record subdirs + 3 post-audit.
        // This count must be updated manually whenever a directory is added or removed.
        // It is intentionally NOT derived from the same constants used by layout_dirs()
        // so that additions to AUDIT_RECORD_DIR_NAMES are caught here as a size change.
        assert_eq!(
            dirs.len(),
            20,
            "layout_dirs() must have exactly 20 entries (2 pre + 15 record + 3 post)"
        );

        // Structural order: audit-store root precedes record subdirs, which precede .anvil.
        let audit_root_pos = dirs.iter().position(|d| d == "audit-store").unwrap();
        let anvil_pos = dirs.iter().position(|d| d == ".anvil").unwrap();
        assert!(
            audit_root_pos < anvil_pos,
            "audit-store must appear before .anvil in layout order"
        );

        // Every AUDIT_RECORD_DIR_NAMES entry must be present with its prefix.
        for name in AUDIT_RECORD_DIR_NAMES {
            let expected = format!("audit-store/{name}");
            assert!(dirs.contains(&expected), "missing audit dir: {expected}");
        }

        // All base (non-record) directories must be present.
        for d in BEFORE_AUDIT_STORE_DIRS.iter().chain(AFTER_AUDIT_STORE_DIRS) {
            assert!(dirs.contains(&d.to_string()), "missing base dir: {d}");
        }
    }
}
