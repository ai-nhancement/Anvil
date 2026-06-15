//! Phase commands — the heart of "build by phases".
//!
//! Philosophy for v0:
//! - The user does most of the actual coding in their normal editor (or using `anvil talk` for spikes).
//! - `anvil phase start <id>` sets the current phase and gives a focused prompt + context from the plan.
//! - When the user believes the phase is done, they run `anvil phase review <id>`.
//!   This **always** runs exactly two reviews (reviewer-a then reviewer-b) against the current state of the phase.
//! - Only after both reviews exist do we allow `anvil phase accept <id>`.
//! - Hard rule: no R3. If the reviews are bad, the user fixes the work and runs `review` again (fresh pair of reviews).

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use colored::Colorize;

use crate::config::load_config;
use crate::llm::LlmClient;
use crate::state::{load_state, save_state, reviews_dir};

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
        println!("{}", "No plan found — run `anvil plan` to generate and review the plan first.".yellow());
        return Ok(());
    }

    for (id, name) in &phases {
        let r1 = rev_dir.join(format!("REVIEW_phase-{}_{}.md", id, "R1"));
        let r2 = rev_dir.join(format!("REVIEW_phase-{}_{}.md", id, "R2"));
        let is_shipped = state.shipped_phases.iter().any(|p| p == id);
        let is_current = state.current_phase.as_deref() == Some(id.as_str());

        let status = if is_shipped {
            format!("{}", "✓ accepted".green())
        } else if is_current && r1.exists() && r2.exists() {
            format!("{}", "R1+R2 done — `anvil phase accept`".yellow())
        } else if is_current && r1.exists() {
            format!("{}", "R1 done — run `anvil phase review` for R2".yellow())
        } else if is_current {
            format!("{}", "in progress — `anvil phase review` when done".cyan())
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
                Some((id, if name.is_empty() { "(unnamed)".to_string() } else { name }))
            } else {
                None
            }
        })
        .collect()
}

pub fn run_phase_start(root: &Path, id: &str) -> Result<()> {
    let mut state = load_state(root);
    state.current_phase = Some(id.to_string());
    save_state(root, &state)?;

    println!(
        "{} Current phase set to {}.",
        "✓".green(),
        id.cyan()
    );

    // Try to give the user the relevant slice of the plan
    let plan_path = root.join("plan.md");
    if plan_path.exists() {
        if let Ok(plan) = fs::read_to_string(&plan_path) {
            // Very crude extraction of the phase section
            if let Some(start) = plan.find(&format!("## {}", id)) {
                let slice: String = plan[start..].lines().take(40).collect::<Vec<_>>().join("\n");
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
    let cfg = load_config(root)?;
    let client = LlmClient::new();
    let reviews = reviews_dir(root);
    fs::create_dir_all(&reviews)?;

    let reviewer_a = cfg.roles.reviewer_a.as_deref().ok_or_else(|| anyhow!("reviewer-a not configured"))?;
    let reviewer_b = cfg.roles.reviewer_b.as_deref().ok_or_else(|| anyhow!("reviewer-b not configured"))?;

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
        "\n{} Running mandatory phase reviews for {} (R1 then R2, different reviewers)...",
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

    println!("\nReviews written to reviews/ directory.");
    println!("Address the findings, then run:");
    println!("  {} {}   (only succeeds after both R1 and R2 exist for the phase)", "`anvil phase accept`".cyan(), id);
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

    let api_key = client.get_credential(name, provider)?;

    let system = "You are performing the mandatory second-opinion review on a completed phase. \
                  Different model family from the implementer is the whole point. \
                  Focus on whether the acceptance criteria are actually met in the delivered work. \
                  Output: ## Verdict (Pass / Needs Work), ## Specific Gaps, ## Recommendations, ## Risks introduced.";

    let user = format!("Phase: {}\n\n{}", phase_id, context);

    println!("  {} reviewing phase {} {} ...", name.cyan(), phase_id, round);

    let findings = LlmClient::block_on(client.chat(provider, &binding.model, &api_key, system, &user))?;

    let out_path = reviews_dir.join(format!("REVIEW_phase-{}_{}.md", phase_id, round));
    let header = format!(
        "# Phase {} — {} ({})\n\nReviewer: {} ({} via {})\nDate: {}\n\n",
        phase_id,
        round,
        if round == "R1" { "first" } else { "second" },
        name, binding.model, provider.r#type,
        chrono::Utc::now().format("%Y-%m-%d")
    );
    fs::write(out_path, format!("{}{}", header, findings))?;
    Ok(findings)
}

pub fn run_phase_accept(root: &Path, id: &str, note: Option<&str>) -> Result<()> {
    let reviews = reviews_dir(root);
    let r1_path = reviews.join(format!("REVIEW_phase-{}_R1.md", id));
    let r2_path = reviews.join(format!("REVIEW_phase-{}_R2.md", id));

    if !r1_path.exists() || !r2_path.exists() {
        return Err(anyhow!(
            "Both R1 and R2 review files must exist before you can accept a phase.\n\
             Run `anvil phase review {}` first (this always does exactly two reviews).",
            id
        ));
    }

    let mut state = load_state(root);

    if !state.shipped_phases.iter().any(|p| p == id) {
        state.shipped_phases.push(id.to_string());
    }
    state.current_phase = None; // ready for next
    save_state(root, &state)?;

    println!(
        "{} Phase {} accepted after R1 + R2.",
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

fn extract_phase(plan: &str, id: &str) -> Option<String> {
    let marker = format!("## {}", id);
    if let Some(start) = plan.find(&marker) {
        let rest = &plan[start..];
        // Take until the next ## that looks like a new phase or the risks section
        let mut lines = rest.lines();
        let mut out = vec![lines.next().unwrap_or("").to_string()];
        for line in lines {
            if line.starts_with("## P") || line.to_lowercase().contains("risk") || line.to_lowercase().contains("open question") {
                break;
            }
            out.push(line.to_string());
        }
        Some(out.join("\n"))
    } else {
        None
    }
}
