//! Very lightweight project state.
//! We deliberately keep this minimal. The real history lives in git + the review markdown files we write.
//! State here is mostly "what phase are we on" and "has the plan been accepted after its R1+R2".

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::state_path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectState {
    pub current_phase: Option<String>,
    /// SHA256 or content hash of the last accepted plan (after its R1+R2).
    pub accepted_plan_hash: Option<String>,
    /// Phases that have completed their full R1 + R2 + accept gate.
    #[serde(default)]
    pub shipped_phases: Vec<String>,

    /// Git commit (HEAD sha) marking where the current phase's work begins, so the
    /// phase review can diff `base..worktree` and capture changes even if the coder
    /// committed them (a plain `git diff HEAD` would miss committed work). Recorded
    /// when a phase starts and when one ships (base for the next phase).
    #[serde(default)]
    pub phase_base: Option<String>,

    /// Has the user explicitly accepted "Work in this Repo" (auto context seeding of key files
    /// at TUI boot for the coder chat). This delivers the low-friction "open the folder" experience
    /// without requiring manual /include for basic grounding on every launch.
    /// None = never prompted for this project (show the Yes/No at next configured boot),
    /// Some(true) = auto-seed on boot (current + future launches),
    /// Some(false) = user prefers manual /include only.
    #[serde(default)]
    pub working_in_repo_accepted: Option<bool>,

    /// The active plan file for this project. None = the default `plan.md`. `/new-plan`
    /// points this at a feature-named file like `frontpage_plan.md`. Sequential model:
    /// one active plan at a time; the previous plan + its REVIEW_* files are archived
    /// under `.anvil/plans/archive/` when a new one starts.
    #[serde(default)]
    pub active_plan: Option<String>,
}

/// Filename of the active plan for this project (defaults to `plan.md`).
pub fn active_plan_name(root: &Path) -> String {
    load_state(root)
        .active_plan
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "plan.md".to_string())
}

/// Path to the active plan file (defaults to `<root>/plan.md`).
pub fn active_plan_path(root: &Path) -> std::path::PathBuf {
    root.join(active_plan_name(root))
}

pub fn load_state(root: &Path) -> ProjectState {
    let path = state_path(root);
    if !path.exists() {
        return ProjectState::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => ProjectState::default(),
    }
}

pub fn save_state(root: &Path, state: &ProjectState) -> Result<()> {
    let path = state_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn reviews_dir(root: &Path) -> std::path::PathBuf {
    // REVIEW_* artifacts live at repo root per PHASE_REVIEW_WORKFLOW.md (coder writes R1/R2 briefing docs there;
    // critical reviewer findings are also persisted at root or as _Findings.md siblings).
    // This changed from the old "reviews/" subdir to keep source-of-truth files visible at root alongside plan.md.
    root.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn active_plan_defaults_to_plan_md() {
        let dir = tempdir().unwrap();
        assert_eq!(active_plan_name(dir.path()), "plan.md");
        assert_eq!(active_plan_path(dir.path()), dir.path().join("plan.md"));
    }

    #[test]
    fn active_plan_reflects_saved_state() {
        let dir = tempdir().unwrap();
        let st = ProjectState {
            active_plan: Some("frontpage_plan.md".to_string()),
            ..Default::default()
        };
        save_state(dir.path(), &st).unwrap();
        assert_eq!(active_plan_name(dir.path()), "frontpage_plan.md");
        assert_eq!(
            active_plan_path(dir.path()),
            dir.path().join("frontpage_plan.md")
        );
    }

    #[test]
    fn blank_active_plan_falls_back_to_default() {
        let dir = tempdir().unwrap();
        let st = ProjectState {
            active_plan: Some("   ".to_string()),
            ..Default::default()
        };
        save_state(dir.path(), &st).unwrap();
        assert_eq!(active_plan_name(dir.path()), "plan.md");
    }
}
