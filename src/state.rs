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
    root.join("reviews")
}
