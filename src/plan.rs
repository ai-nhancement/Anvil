//! `anvil plan`
//!
//! Generate (or refresh) a phased plan using the configured planner role.
//! Then **immediately and automatically** run exactly two reviews:
//!   R1 using reviewer-a
//!   R2 using reviewer-b
//!
//! These two must be different bindings (ideally different providers).
//! After R2 the user must explicitly accept before phases can be marked complete.
//! There is no R3 path in the UI.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use colored::Colorize;

use crate::config::load_config;
use crate::llm::LlmClient;
use crate::state::reviews_dir;

const PLAN_SYSTEM: &str = "\
You are an excellent technical planner. Given the user's project intent, produce a realistic phased plan.

Rules:
- Phases must be small enough that a reviewer can understand the scope in one sitting, but large enough to deliver meaningful value.
- Each phase must have: id (P0, P1, ...), name, one-sentence goal, 3-8 concrete actions, deliverable, 2-5 testable acceptance criteria, dependencies (list of prior phase ids or empty).
- Keep total phases reasonable (usually 4-10 for a focused project).
- Explicitly call out cross-cutting concerns and where big risky decisions are deferred.
- Output a clean Markdown document starting with a title and a short summary, then the phases as ## Px — Name sections.
- At the end include a 'Risks & Open Questions' section.

Be precise and skeptical of scope creep.";

pub fn run_plan(root: &Path, fresh: bool) -> Result<()> {
    let cfg = load_config(root)?;
    let client = LlmClient::new();

    let plan_path = root.join("plan.md");
    let reviews = reviews_dir(root);
    fs::create_dir_all(&reviews)?;

    // 1. Get (or generate) the plan content
    let plan_content = if !fresh && plan_path.exists() {
        println!("Using existing plan.md (use --fresh to regenerate).");
        fs::read_to_string(&plan_path)?
    } else {
        // Resolve planner
        let (planner_name, planner_binding, planner_provider) = cfg
            .resolve_role_full("planner")
            .or_else(|_| cfg.resolve_role_full("coder"))
            .map_err(|_| anyhow!("Configure a 'planner' (or at least 'coder') role via `anvil setup`."))?;

        let api_key = client.get_credential(planner_name, planner_provider)?;

        // If we have prior talk artifacts or a charter, we could feed them. For v0 we keep it simple:
        // user is expected to have done `anvil talk` and the context is in their head + any saved artifacts.
        println!(
            "\n{} Generating plan with {} ({} via {})...",
            "anvil".green(),
            planner_name.cyan(),
            planner_binding.model,
            planner_provider.r#type
        );

        // For the first cut we just give the planner a generic prompt.
        // A nicer version would slurp recent artifacts from reviews/ or let user point at a charter.
        let user_msg = "Produce the phased plan now for the project we have been discussing. Make it concrete and reviewable.";

        let content = LlmClient::block_on(client.chat(
            planner_provider,
            &planner_binding.model,
            &api_key,
            PLAN_SYSTEM,
            user_msg,
        ))?;

        fs::write(&plan_path, &content)?;
        println!("{} Plan written to plan.md", "✓".green());
        content
    };

    // 2. Run exactly two reviews using the two different reviewer roles.
    let reviewer_a = cfg.roles.reviewer_a.as_deref()
        .ok_or_else(|| anyhow!("reviewer-a role not configured. Run `anvil setup`."))?;
    let reviewer_b = cfg.roles.reviewer_b.as_deref()
        .ok_or_else(|| anyhow!("reviewer-b role not configured. Run `anvil setup`."))?;

    if reviewer_a == reviewer_b {
        println!("{}", "WARNING: reviewer-a and reviewer-b are the same binding. This is bad for drift protection.".red().bold());
    }

    println!("\n{}", "Running mandatory plan reviews (exactly two rounds, different reviewers)...".bold());

    // R1
    let _r1 = run_single_review(&client, &cfg, reviewer_a, &plan_content, "R1", &reviews, "plan")?;
    println!("{} R1 complete — findings saved", "✓".green());

    // R2 — always happens, even if R1 was brutal.
    let _r2 = run_single_review(&client, &cfg, reviewer_b, &plan_content, "R2", &reviews, "plan")?;
    println!("{} R2 complete — findings saved", "✓".green());

    println!("\n{}", "Both review rounds finished.".bold());
    println!("Review documents:");
    println!("  {}", reviews.join("REVIEW_plan_R1.md").display());
    println!("  {}", reviews.join("REVIEW_plan_R2.md").display());

    println!("\nAddress the findings in plan.md (or in your implementation approach).");
    println!("When you are satisfied that the plan (after addressing R1+R2) is solid, run:");
    println!("  {}   — this records that the plan passed its two-review gate.", "`anvil plan --accept` (not yet wired)".cyan());

    println!("\n{} Address findings from the two reviews, then /accept-plan (TUI) or the --accept step before starting phases with `anvil phase start P0`.", "Next:".green());
    Ok(())
}

pub fn run_single_review(
    client: &LlmClient,
    cfg: &crate::config::AnvilConfig,
    reviewer_role: &str,
    plan_content: &str,
    round: &str,
    reviews_dir: &Path,
    artifact: &str,
) -> Result<String> {
    let (name, binding, provider) = cfg.resolve_role_full(reviewer_role)
        .map_err(|_| anyhow!("reviewer role '{}' is not fully configured", reviewer_role))?;

    let api_key = client.get_credential(name, provider)?;

    let system = "You are a skeptical, experienced engineer from a *different* model family than the planner. \
                  Your job is to find real problems, scope issues, hidden risks, and weak acceptance criteria. \
                  Do not be nice. Be specific. Cite exact sections or phase ids. \
                  Output a short structured review with sections: ## Summary, ## High, ## Medium, ## Low, ## Questions.";

    let user = format!(
        "Review the following plan ({}).\n\n--- PLAN ---\n{}\n--- END PLAN ---\n\nProduce the structured review now.",
        round, plan_content
    );

    println!("  Invoking {} ({} via {}) for {}...", name.cyan(), binding.model, provider.r#type, round);

    let findings = LlmClient::block_on(client.chat(provider, &binding.model, &api_key, system, &user))?;

    let out_path = reviews_dir.join(format!("REVIEW_{}_{}.md", artifact, round));
    let header = format!(
        "# {} — {} Review ({})\n\n**Reviewer:** {} ({} via {})\n**Date:** {}\n\n",
        artifact,
        round,
        if round == "R1" { "first" } else { "second" },
        name,
        binding.model,
        provider.r#type,
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
    );
    fs::write(&out_path, format!("{}{}", header, findings))?;

    Ok(findings)
}

pub fn simple_hash(s: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
