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
