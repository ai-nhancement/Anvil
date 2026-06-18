//! Reality snapshot — the agent's "what is actually true right now" grounding.
//!
//! A bounded, plain-text block built purely from disk + git (no model call), so
//! the coder always knows the workflow stage, the current phase + its plan slice,
//! and what has actually changed in the tree — without the user having to remind
//! it. It is *context, never authority*: the reviewed `plan.md` / `REVIEW_*` files
//! remain the source of truth, and the snapshot flags a stale plan rather than
//! trusting it.
//!
//! Exposed two ways: injected into the agent's context each turn (see `agent.rs`)
//! and as the `project_state` tool the agent can call on demand (see `tools.rs`).

use std::path::Path;
use std::process::Command;

use crate::plan::simple_hash;
use crate::state::{load_state, reviews_dir};

/// Hard cap on the whole snapshot so it never bloats the prompt.
const MAX_SNAPSHOT: usize = 4000;
/// Caps on individual sections.
const MAX_EXCERPT: usize = 1200;
const MAX_GIT: usize = 1600;

/// Build the reality snapshot for `root`. Always returns a delimited block.
pub fn snapshot(root: &Path) -> String {
    let mut s = String::from("--- REALITY SNAPSHOT (live; disk + git are the source of truth) ---\n");

    s.push_str(&format!("Platform: {}\n", std::env::consts::OS));
    s.push_str(&format!("Stage: {}\n", stage_label(root)));

    let state = load_state(root);
    if let Some(phase) = &state.current_phase {
        s.push_str(&format!("Current phase: {}\n", phase));
    }
    if !state.shipped_phases.is_empty() {
        s.push_str(&format!("Shipped phases: {}\n", state.shipped_phases.join(", ")));
    }

    // Plan excerpt for the current phase (so the agent sees the spec it's building to).
    if let Some(phase) = &state.current_phase {
        if let Ok(plan) = std::fs::read_to_string(root.join("plan.md")) {
            if let Some(excerpt) = crate::phase::extract_phase(&plan, phase) {
                s.push_str("\nCurrent phase from plan.md:\n");
                s.push_str(&cap(&excerpt, MAX_EXCERPT));
                s.push('\n');
            }
        }
    }

    s.push_str("\nGit:\n");
    s.push_str(&git_summary(root));

    s.push_str("--- END REALITY SNAPSHOT ---\n");
    cap(&s, MAX_SNAPSHOT)
}

/// Derive the workflow stage from disk artifacts (same truth the TUI header uses).
/// Pure: reads `plan.md` + the two plan reviews + the accepted hash in state.
fn stage_label(root: &Path) -> &'static str {
    let plan_path = root.join("plan.md");
    let rev = reviews_dir(root);
    let r1 = rev.join("REVIEW_plan_R1.md");
    let r2 = rev.join("REVIEW_plan_R2.md");

    if plan_path.exists() && r1.exists() && r2.exists() {
        if let Ok(txt) = std::fs::read_to_string(&plan_path) {
            let st = load_state(root);
            if st.accepted_plan_hash.as_deref() == Some(simple_hash(&txt).as_str()) {
                return "PLAN ACCEPTED — building phases (/accept-phase when a phase is done)";
            }
        }
        "PLAN REVIEWED (R1+R2 on disk) — /accept-plan to approve (or revise plan.md)"
    } else if plan_path.exists() {
        "PLANNING — plan.md exists; /lock-plan to run the R1+R2 reviewers"
    } else {
        "TALK — no plan yet; discuss, then write plan.md and /lock-plan"
    }
}

/// Branch + short status + diff stat (names/counts, not full diff — kept cheap).
/// Errors (no git, not a repo) degrade gracefully to a short note.
pub fn git_summary(root: &Path) -> String {
    let run = |args: &[&str]| -> Option<String> {
        Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim_end().to_string())
    };

    let branch = run(&["rev-parse", "--abbrev-ref", "HEAD"]);
    if branch.is_none() {
        return "  (not a git repository, or git unavailable)\n".to_string();
    }

    let mut out = String::new();
    out.push_str(&format!("  branch: {}\n", branch.unwrap_or_default()));

    match run(&["status", "--short"]) {
        Some(st) if !st.is_empty() => {
            out.push_str("  uncommitted changes (git status --short):\n");
            for line in st.lines() {
                out.push_str(&format!("    {}\n", line));
            }
        }
        _ => out.push_str("  working tree clean\n"),
    }

    if let Some(stat) = run(&["diff", "--stat", "HEAD"]) {
        if !stat.is_empty() {
            out.push_str("  diff vs HEAD (stat):\n");
            for line in stat.lines() {
                out.push_str(&format!("    {}\n", line));
            }
        }
    }

    cap(&out, MAX_GIT)
}

/// Truncate to `max` chars on a char boundary, appending a marker if cut.
pub(crate) fn cap(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… [truncated]\n", &s[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_delimited_and_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let snap = snapshot(dir.path());
        assert!(snap.starts_with("--- REALITY SNAPSHOT"));
        assert!(snap.contains("--- END REALITY SNAPSHOT ---"));
        assert!(snap.contains("Stage:"));
        assert!(snap.len() <= MAX_SNAPSHOT + 64);
    }

    #[test]
    fn stage_is_talk_with_no_plan() {
        let dir = tempfile::tempdir().unwrap();
        assert!(stage_label(dir.path()).starts_with("TALK"));
    }

    #[test]
    fn git_summary_handles_non_repo() {
        let dir = tempfile::tempdir().unwrap();
        let g = git_summary(dir.path());
        assert!(g.contains("not a git repository") || g.contains("branch:"));
    }
}
