use std::path::{Path, PathBuf};

use crate::config::{save_config, AnvilConfig};
use crate::error::AnvilError;

/// All directories created by `anvil init`, relative to the project root.
/// Pinned by `hinge_test`: `test_project_layout_directories` (phase P1).
pub const LAYOUT_DIRS: &[&str] = &[
    "phases",
    "audit-store",
    "audit-store/reviewer-finding-packet",
    "audit-store/verifier-result",
    "audit-store/rotation-log",
    "audit-store/charter-amendment",
    "audit-store/plan-amendment",
    "audit-store/phase-disposition",
    "audit-store/hinge-flip",
    "audit-store/gate-approval",
    "audit-store/convergence-declaration",
    "audit-store/provisional-lock",
    "audit-store/rollback-event",
    ".anvil",
    ".anvil/run",
    ".anvil/logs",
];

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

    for dir in LAYOUT_DIRS {
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
        dirs_created: LAYOUT_DIRS.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // hinge_test: pins=16, intended=project-layout-directories, phase=P1
    #[test]
    fn test_project_layout_directories() {
        // Pins: the per-project directory layout has exactly these 16 entries.
        // Changing a directory name or adding/removing a directory requires updating
        // LAYOUT_DIRS and this test together; it is a breaking change for existing projects.
        let expected: &[&str] = &[
            "phases",
            "audit-store",
            "audit-store/reviewer-finding-packet",
            "audit-store/verifier-result",
            "audit-store/rotation-log",
            "audit-store/charter-amendment",
            "audit-store/plan-amendment",
            "audit-store/phase-disposition",
            "audit-store/hinge-flip",
            "audit-store/gate-approval",
            "audit-store/convergence-declaration",
            "audit-store/provisional-lock",
            "audit-store/rollback-event",
            ".anvil",
            ".anvil/run",
            ".anvil/logs",
        ];
        assert_eq!(
            LAYOUT_DIRS, expected,
            "project layout directories must match pinned set"
        );
    }
}
