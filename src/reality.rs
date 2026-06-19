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

use crate::state::{active_plan_name, active_plan_path, load_state, reviews_dir};

/// Hard cap on the whole snapshot so it never bloats the prompt.
const MAX_SNAPSHOT: usize = 4000;
/// Caps on individual sections.
const MAX_EXCERPT: usize = 1200;
const MAX_GIT: usize = 1600;

/// Build the reality snapshot for `root`. Always returns a delimited block.
pub fn snapshot(root: &Path) -> String {
    let mut s =
        String::from("--- REALITY SNAPSHOT (live; disk + git are the source of truth) ---\n");

    s.push_str(&format!("Platform: {}\n", std::env::consts::OS));
    s.push_str(&format!("Stage: {}\n", stage_label(root)));

    let plan_name = active_plan_name(root);
    let state = load_state(root);
    if root.join(&plan_name).exists() || state.active_plan.is_some() {
        s.push_str(&format!(
            "Plan file: {plan_name} (write and read the plan here; its reviews and phases use this file)\n"
        ));
    } else {
        s.push_str(
            "Plan file: not chosen yet. When the user asks you to write the plan, pick a short \
             descriptive name like <feature>_plan.md (e.g. trusteazy_plan.md) and write the full \
             plan there. Anvil adopts that file as the active plan automatically. Use the generic \
             plan.md only for a tiny throwaway project.\n",
        );
    }

    if let Some(phase) = &state.current_phase {
        s.push_str(&format!("Current phase: {}\n", phase));
    }
    if !state.shipped_phases.is_empty() {
        s.push_str(&format!(
            "Shipped phases: {}\n",
            state.shipped_phases.join(", ")
        ));
    }

    let plan_text = std::fs::read_to_string(active_plan_path(root)).unwrap_or_default();

    // Plan excerpt for the current phase (so the agent sees the spec it's building to).
    if let Some(phase) = &state.current_phase {
        if let Some(excerpt) = crate::phase::extract_phase(&plan_text, phase) {
            s.push_str(&format!("\nCurrent phase from {plan_name}:\n"));
            s.push_str(&cap(&excerpt, MAX_EXCERPT));
            s.push('\n');
        }
    } else {
        // Between phases: point at the next unshipped phase so the agent builds it
        // rather than getting confused and suggesting the plan be re-accepted.
        let ids = crate::phase::plan_phase_ids(&plan_text);
        if let Some(next) = ids.iter().find(|id| !state.shipped_phases.contains(id)) {
            s.push_str(&format!(
                "Next phase to build: {next} (not started). Build it directly, or the user can /phase-start {next}. \
                 The plan is already accepted — do NOT run or suggest /accept-plan again.\n"
            ));
            if let Some(excerpt) = crate::phase::extract_phase(&plan_text, next) {
                s.push_str(&format!("\nNext phase from {plan_name}:\n"));
                s.push_str(&cap(&excerpt, MAX_EXCERPT));
                s.push('\n');
            }
        } else if !ids.is_empty() {
            s.push_str("All planned phases are shipped.\n");
        }
    }

    s.push_str("\nGit:\n");
    s.push_str(&git_summary(root));

    s.push_str("--- END REALITY SNAPSHOT ---\n");
    cap(&s, MAX_SNAPSHOT)
}

/// Derive the workflow stage from disk artifacts (same truth the TUI header uses).
/// Pure: reads `plan.md` + the two plan reviews + the accepted hash in state.
fn stage_label(root: &Path) -> String {
    let plan_name = active_plan_name(root);
    let plan_path = root.join(&plan_name);
    let rev = reviews_dir(root);
    let r1 = rev.join("REVIEW_plan_R1.md");
    let r2 = rev.join("REVIEW_plan_R2.md");
    let st = load_state(root);

    if plan_path.exists() && r1.exists() && r2.exists() {
        // "Accepted" is a latched state: once the plan was accepted, or any phase
        // has shipped, we're past the plan gate and building phases. Don't bounce
        // back to "/accept-plan" just because the plan was edited during the work.
        if st.accepted_plan_hash.is_some() || !st.shipped_phases.is_empty() {
            "PLAN ACCEPTED — building phases (/accept-phase when a phase is done)".to_string()
        } else {
            format!(
                "PLAN REVIEWED (R1+R2 on disk) — /accept-plan to approve (or revise {plan_name})"
            )
        }
    } else if plan_path.exists() {
        format!("PLANNING — {plan_name} exists; /lock-plan to run the R1+R2 reviewers")
    } else {
        format!("TALK — no plan yet; discuss, then write {plan_name} and /lock-plan")
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
