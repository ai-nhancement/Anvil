//! Phase commands — the heart of "build by phases".
//!
//! Preferred flow (TUI chat-driven, matches PHASE_REVIEW_WORKFLOW.md + user spec):
//! - /phase-start Px (or chat "start phase Px") — coder + human implement per plan excerpt.
//! - When done: human tells *coder* "phase Px complete, write the R1 review document" (coder outputs full REVIEW_Px_R1.md markdown per template).
//! - Human uses /save-r1 (TUI) to persist the coder-written briefing to root as REVIEW_Px_R1.md.
//! - /critical-r1 (or equivalent) — R1 (reviewer_a) *automatically reads the coder's review doc* and runs critical review, presents findings in chat, writes sibling _Findings.
//! - Human approves findings; coder implements fixes (code + tests).
//! - Human tells coder "write the R2 review document" (coder outputs REVIEW_Px_R2.md including "Findings from R1" table).
//! - /save-r2 ; /critical-r2 (reviewer_b critical on the R2 doc coder wrote) ; human approves ; coder implements ; coder summarizes and asks approval to ship phase.
//! - /phase-accept Px — mark shipped (updates state, clears current_phase).
//!
//! Legacy CLI `anvil phase review` still does the old "always two reviews immediately" against implementation state (kept for scripts). New flow keeps human gates between coder-written docs and each critical reviewer pass. All REVIEW_* now at repo root.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use colored::Colorize;

use crate::config::{load_config, load_local_env};
use crate::llm::LlmClient;
use crate::state::{load_state, reviews_dir, save_state};

pub fn run_phase_list(root: &Path) -> Result<()> {
    let state = load_state(root);
    let rev_dir = reviews_dir(root);

    println!("{}", "Phases".bold());
    println!();

    // Parse phase declarations from plan.md
    let plan_path = root.join("plan.md");
    let phases = if plan_path.exists() {
        let plan = fs::read_to_string(&plan_path).unwrap_or_default();
        parse_plan_phases(&plan)
    } else {
        vec![]
    };

    if phases.is_empty() {
        println!(
            "{}",
            "No plan found — run `anvil plan` to generate and review the plan first.".yellow()
        );
        return Ok(());
    }

    for (id, name) in &phases {
        // Detect review artifacts for both new TUI flow (REVIEW_Px_R*.md from /save-r*) and legacy.
        let r1_new = rev_dir.join(format!("REVIEW_{}_R1.md", id));
        let r2_new = rev_dir.join(format!("REVIEW_{}_R2.md", id));
        let r1_leg = rev_dir.join(format!("REVIEW_phase-{}_R1.md", id));
        let r2_leg = rev_dir.join(format!("REVIEW_phase-{}_R2.md", id));
        let has_both = (r1_new.exists() && r2_new.exists()) || (r1_leg.exists() && r2_leg.exists());
        let has_r1 = r1_new.exists() || r1_leg.exists();

        let is_shipped = state.shipped_phases.iter().any(|p| p == id);
        let is_current = state.current_phase.as_deref() == Some(id.as_str());

        let status = if is_shipped {
            format!("{}", "✓ accepted".green())
        } else if is_current && has_both {
            format!(
                "{}",
                "R1+R2 artifacts present — /phase-accept (or legacy review)".yellow()
            )
        } else if is_current && has_r1 {
            format!(
                "{}",
                "R1 review doc present — continue to R2 doc + criticals".yellow()
            )
        } else if is_current {
            format!(
                "{}",
                "in progress — tell coder 'write R1 review doc' then /save-r1 + /critical-r1"
                    .cyan()
            )
        } else {
            format!("{}", "pending".dimmed())
        };

        let marker = if is_current { "→ " } else { "  " };
        println!("{}{} — {}  [{}]", marker, id.cyan(), name, status);
    }

    println!();
    if let Some(phase) = &state.current_phase {
        println!("Current phase: {}", phase.cyan());
    }
    if !state.shipped_phases.is_empty() {
        println!("Shipped: {}", state.shipped_phases.join(", "));
    }

    Ok(())
}

/// Extract (id, name) pairs from plan.md. Matches "## Px — Name" or "## Px: Name" style headers.
fn parse_plan_phases(plan: &str) -> Vec<(String, String)> {
    plan.lines()
        .filter_map(|line| {
            let stripped = line.trim_start_matches('#').trim();
            // Match "P0", "P1", ... optionally followed by " — Name" or ": Name"
            if let Some(rest) = stripped.strip_prefix('P') {
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if digits.is_empty() {
                    return None;
                }
                let id = format!("P{}", digits);
                let after = &rest[digits.len()..];
                let name = after
                    .trim_start_matches([' ', '—', '-', ':'])
                    .trim()
                    .to_string();
                Some((
                    id,
                    if name.is_empty() {
                        "(unnamed)".to_string()
                    } else {
                        name
                    },
                ))
            } else {
                None
            }
        })
        .collect()
}

pub fn run_phase_start(root: &Path, id: &str) -> Result<()> {
    load_local_env(root);
    let mut state = load_state(root);
    state.current_phase = Some(id.to_string());
    save_state(root, &state)?;

    println!("{} Current phase set to {}.", "✓".green(), id.cyan());

    // Try to give the user the relevant slice of the plan
    let plan_path = root.join("plan.md");
    if plan_path.exists() {
        if let Ok(plan) = fs::read_to_string(&plan_path) {
            // Very crude extraction of the phase section
            if let Some(start) = plan.find(&format!("## {}", id)) {
                let slice: String = plan[start..]
                    .lines()
                    .take(40)
                    .collect::<Vec<_>>()
                    .join("\n");
                println!("\nRelevant plan excerpt:\n{}", slice);
            }
        }
    }

    println!("\nNow go implement the phase in your editor (or use `anvil talk --model coder` for assistance).");
    println!("When you are ready for the mandatory two reviews, run:");
    println!("  {} {}", "`anvil phase review`".cyan(), id);
    Ok(())
}

pub fn run_phase_review(root: &Path, id: &str) -> Result<()> {
    load_local_env(root);
    let cfg = load_config(root)?;
    let client = LlmClient::new();
    let reviews = reviews_dir(root);
    fs::create_dir_all(&reviews)?;

    let reviewer_a = cfg
        .roles
        .reviewer_a
        .as_deref()
        .ok_or_else(|| anyhow!("reviewer-a not configured"))?;
    let reviewer_b = cfg
        .roles
        .reviewer_b
        .as_deref()
        .ok_or_else(|| anyhow!("reviewer-b not configured"))?;

    // For the review we give the model:
    // - the plan excerpt for this phase
    // - a note that the user has declared the work done
    // - we ask it to review against the acceptance criteria.
    //
    // In a more advanced version we would compute a real diff or feed file contents.
    // For anti-drift the important thing is that two different models from different providers see the work.

    let plan = fs::read_to_string(root.join("plan.md")).unwrap_or_default();
    let phase_excerpt = extract_phase(&plan, id).unwrap_or_else(|| plan.clone());

    println!(
        "\n{} Running legacy phase reviews for {} (R1 then R2). Preferred: chat-driven where coder writes the R1/R2 review *docs*, then separate /critical-* trigger reviewer critical passes with user approve between each.",
        "anvil".green(),
        id.cyan()
    );

    let context = format!(
        "Phase {} — the user says implementation is complete.\n\n\
         Plan excerpt for this phase:\n{}\n\n\
         Review the actual work that was done for this phase against the acceptance criteria. \
         Be specific about gaps, over-engineering, missing tests, etc.",
        id, phase_excerpt
    );

    let _r1 = run_phase_review_one(&client, &cfg, reviewer_a, id, "R1", &reviews, &context)?;
    println!("{} R1 (reviewer-a) complete", "✓".green());

    let _r2 = run_phase_review_one(&client, &cfg, reviewer_b, id, "R2", &reviews, &context)?;
    println!("{} R2 (reviewer-b) complete", "✓".green());

    println!("\nReviews written (legacy path). For the chat-driven flow use coder to write REVIEW_Px_R1.md, /save-r1, then critical reviewer passes with human approve gates between.");
    println!("Address the findings, then run:");
    println!(
        "  {} {}   (only succeeds after both R1 and R2 exist for the phase)",
        "`anvil phase accept`".cyan(),
        id
    );
    Ok(())
}

fn run_phase_review_one(
    client: &LlmClient,
    cfg: &crate::config::AnvilConfig,
    reviewer_role: &str,
    phase_id: &str,
    round: &str,
    reviews_dir: &Path,
    context: &str,
) -> Result<String> {
    let (name, binding, provider) = cfg.resolve_role_full(reviewer_role)?;

    let api_key = client.get_credential(&binding.provider, provider)?;

    let system = "You are performing the mandatory second-opinion review on a completed phase. \
                  Different model family from the implementer is the whole point. \
                  Focus on whether the acceptance criteria are actually met in the delivered work. \
                  Output: ## Verdict (Pass / Needs Work), ## Specific Gaps, ## Recommendations, ## Risks introduced.";

    let user = format!("Phase: {}\n\n{}", phase_id, context);

    println!(
        "  {} reviewing phase {} {} ...",
        name.cyan(),
        phase_id,
        round
    );

    let findings =
        LlmClient::block_on(client.chat(provider, &binding.model, &api_key, system, &user))?;

    let out_path = reviews_dir.join(format!("REVIEW_phase-{}_{}.md", phase_id, round));
    let header = format!(
        "# Phase {} — {} ({})\n\nReviewer: {} ({} via {})\nDate: {}\n\n",
        phase_id,
        round,
        if round == "R1" { "first" } else { "second" },
        name,
        binding.model,
        provider.r#type,
        chrono::Utc::now().format("%Y-%m-%d")
    );
    fs::write(out_path, format!("{}{}", header, findings))?;
    Ok(findings)
}

pub fn run_phase_accept(root: &Path, id: &str, note: Option<&str>) -> Result<()> {
    load_local_env(root);
    let reviews = reviews_dir(root);

    // Support both the preferred new TUI flow naming (REVIEW_Px_R1.md written by /save-r1 etc.)
    // and the legacy CLI naming (REVIEW_phase-Px_R1.md from `anvil phase review`).
    let r1_new = reviews.join(format!("REVIEW_{}_R1.md", id));
    let r2_new = reviews.join(format!("REVIEW_{}_R2.md", id));
    let r1_leg = reviews.join(format!("REVIEW_phase-{}_R1.md", id));
    let r2_leg = reviews.join(format!("REVIEW_phase-{}_R2.md", id));

    let has_r1r2 = (r1_new.exists() && r2_new.exists()) || (r1_leg.exists() && r2_leg.exists());
    if !has_r1r2 {
        return Err(anyhow!(
            "Both R1 and R2 review files must exist before you can accept a phase.\n\
             Preferred (TUI): tell coder to write REVIEW_{}_R1.md, /save-r1, /critical-r1, then R2 doc + /save-r2 + /critical-r2.\n\
             Legacy: run `anvil phase review {}` (writes the phase- named files).",
            id, id
        ));
    }

    let mut state = load_state(root);

    if !state.shipped_phases.iter().any(|p| p == id) {
        state.shipped_phases.push(id.to_string());
    }
    state.current_phase = None; // ready for next
    save_state(root, &state)?;

    println!(
        "{} Phase {} accepted after its full review cycle (R1 doc by coder + critical R1 + R2 doc by coder + critical R2).",
        "✓".green().bold(),
        id
    );
    if let Some(n) = note {
        println!("  Note recorded: {}", n);
    }

    println!("\nMove to the next phase with `anvil phase start <next-id>`.");
    println!("When all phases that deliver value are done, you can ship (simple for v0: just commit and tag).");
    Ok(())
}

/// Capture the working-tree diff against HEAD (staged + unstaged), plus the
/// names of any untracked files, so reviewers critique the *actual* change.
fn capture_git_diff(root: &Path) -> String {
    use std::process::Command;
    let mut diff = match Command::new("git")
        .args(["diff", "HEAD"])
        .current_dir(root)
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(e) => return format!("(could not run `git diff`: {})", e),
    };
    if let Ok(o) = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(root)
        .output()
    {
        let untracked = String::from_utf8_lossy(&o.stdout);
        if !untracked.trim().is_empty() {
            diff.push_str("\n\n--- Untracked files (names only) ---\n");
            diff.push_str(&untracked);
        }
    }
    if diff.trim().is_empty() {
        return "(no changes vs HEAD — nothing to review for this phase yet)".to_string();
    }
    if diff.len() > 120_000 {
        diff.truncate(120_000);
        diff.push_str("\n... [diff truncated for review]");
    }
    diff
}

/// Compose the reviewer input for a phase: the plan excerpt + the real diff.
fn build_phase_diff_content(root: &Path, id: &str) -> String {
    let plan = fs::read_to_string(root.join("plan.md")).unwrap_or_default();
    let excerpt = extract_phase(&plan, id)
        .unwrap_or_else(|| "(no plan excerpt found for this phase)".to_string());
    let diff = capture_git_diff(root);
    format!(
        "Phase {} — critically review the implementation against the plan.\n\n\
         --- PLAN EXCERPT ---\n{}\n\n\
         --- GIT DIFF (working tree vs HEAD) ---\n{}\n",
        id, excerpt, diff
    )
}

/// R1 of a phase: reviewer-a critiques the current diff. Writes REVIEW_<id>_R1.md.
/// Used by the TUI `/accept-phase` gate.
pub fn run_phase_r1_diff(root: &Path, id: &str) -> Result<String> {
    load_local_env(root);
    let cfg = load_config(root)?;
    let client = LlmClient::new();
    let reviews = reviews_dir(root);
    fs::create_dir_all(&reviews)?;
    let content = build_phase_diff_content(root, id);
    let reviewer_a = cfg
        .roles
        .reviewer_a
        .as_deref()
        .ok_or_else(|| anyhow!("reviewer-a role not configured. Run `anvil setup`."))?;
    crate::plan::run_single_review(&client, &cfg, reviewer_a, &content, "R1", &reviews, id)
}

/// R2 of a phase: reviewer-b critiques the current diff. Writes REVIEW_<id>_R2.md.
pub fn run_phase_r2_diff(root: &Path, id: &str) -> Result<String> {
    load_local_env(root);
    let cfg = load_config(root)?;
    let client = LlmClient::new();
    let reviews = reviews_dir(root);
    fs::create_dir_all(&reviews)?;
    let content = build_phase_diff_content(root, id);
    let reviewer_b = cfg
        .roles
        .reviewer_b
        .as_deref()
        .ok_or_else(|| anyhow!("reviewer-b role not configured. Run `anvil setup`."))?;
    crate::plan::run_single_review(&client, &cfg, reviewer_b, &content, "R2", &reviews, id)
}

pub(crate) fn extract_phase(plan: &str, id: &str) -> Option<String> {
    let marker = format!("## {}", id);
    if let Some(start) = plan.find(&marker) {
        let rest = &plan[start..];
        // Take until the next ## that looks like a new phase or the risks section
        let mut lines = rest.lines();
        let mut out = vec![lines.next().unwrap_or("").to_string()];
        for line in lines {
            if line.starts_with("## P")
                || line.to_lowercase().contains("risk")
                || line.to_lowercase().contains("open question")
            {
                break;
            }
            out.push(line.to_string());
        }
        Some(out.join("\n"))
    } else {
        None
    }
}
