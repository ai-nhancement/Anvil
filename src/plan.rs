//! `anvil plan` (and TUI /plan support)
//!
//! The interactive flow (preferred): plan is discussed and written by the *coder* role inside the TUI chat.
//! User saves it (`/save-plan`), then uses the explicit human gates:
//!   /lock-plan  -> R1 (reviewer-a) automatically reviews plan.md and presents findings (writes REVIEW_plan_R1.md at root)
//!   (coder helps apply fixes to plan.md; user may /save-plan again)
//!   /approve-r1 -> R2 (reviewer-b) automatically reviews (updated) plan and presents findings
//!   (user approves R2 findings)
//!   /accept-plan -> records hash, plan approved, phases unlocked.
//!
//! The legacy one-shot `anvil plan --fresh` still generates via coder + immediately runs both R1+R2 (for scripts/CLI users).
//! Both paths now write REVIEW_plan_R*.md at repo *root* (see PHASE_REVIEW_WORKFLOW.md).

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use colored::Colorize;

use crate::config::{load_config, load_local_env};
use crate::llm::{ChatMessage, LlmClient};
use crate::state::{load_state, reviews_dir, save_state};

/// Max investigation steps a reviewer may take (tool calls) before it must write
/// up its findings — bounds cost on a cross-vendor review.
const REVIEWER_MAX_STEPS: usize = 14;

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

pub fn run_plan(root: &Path, fresh: bool, context_file: Option<&Path>) -> Result<()> {
    // Make .anvil/.env secrets (written during interactive add) available no matter
    // what shell / OS / CI environment launched us.
    load_local_env(root);

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
        let (planner_name, planner_binding, planner_provider) = cfg
            .resolve_role_full("coder")
            .map_err(|_| anyhow!("Configure a 'coder' role via `anvil setup`."))?;

        let api_key = client.get_credential(&planner_binding.provider, planner_provider)?;

        println!(
            "\n{} Generating plan with {} ({} via {})...",
            "anvil".green(),
            planner_name.cyan(),
            planner_binding.model,
            planner_provider.r#type
        );

        // Build the user message. If a context file was provided (e.g. a saved talk artifact
        // or charter), prepend it so the planner has grounded input to work from.
        let user_msg = build_plan_prompt(root, context_file)?;

        let content = LlmClient::block_on(client.chat(
            planner_provider,
            &planner_binding.model,
            &api_key,
            PLAN_SYSTEM,
            &user_msg,
        ))?;

        fs::write(&plan_path, &content)?;
        println!("{} Plan written to plan.md", "✓".green());
        content
    };

    // 2. Run exactly two reviews using the two different reviewer roles.
    let reviewer_a = cfg
        .roles
        .reviewer_a
        .as_deref()
        .ok_or_else(|| anyhow!("reviewer-a role not configured. Run `anvil setup`."))?;
    let reviewer_b = cfg
        .roles
        .reviewer_b
        .as_deref()
        .ok_or_else(|| anyhow!("reviewer-b role not configured. Run `anvil setup`."))?;

    if reviewer_a == reviewer_b {
        println!("{}", "WARNING: reviewer-a and reviewer-b are the same binding. This is bad for drift protection.".red().bold());
    }

    println!(
        "\n{}",
        "Running mandatory plan reviews (exactly two rounds, different reviewers)...".bold()
    );

    // R1
    let _r1 = run_single_review(&client, &cfg, reviewer_a, &plan_content, "R1", root, "plan")?;
    println!("{} R1 complete — findings saved", "✓".green());

    // R2 — always happens, even if R1 was brutal.
    let _r2 = run_single_review(&client, &cfg, reviewer_b, &plan_content, "R2", root, "plan")?;
    println!("{} R2 complete — findings saved", "✓".green());

    println!("\n{}", "Both review rounds finished.".bold());
    println!("Review documents (at repo root):");
    println!("  {}", reviews.join("REVIEW_plan_R1.md").display());
    println!("  {}", reviews.join("REVIEW_plan_R2.md").display());

    println!("\nAddress the findings in plan.md (or in your implementation approach).");
    println!("When you are satisfied that the plan (after addressing R1+R2) is solid, run:");
    println!(
        "  {}   — this records that the plan passed its two-review gate.",
        "`anvil plan --accept`".cyan()
    );
    Ok(())
}

/// Record that the user has addressed R1+R2 findings for the current plan.md.
/// Writes `accepted_plan_hash` to .anvil/state.json (same value the TUI /accept-plan produces).
/// Requires plan.md + both REVIEW_plan_R*.md to exist.
pub fn accept_plan(root: &Path) -> Result<()> {
    let plan_path = root.join("plan.md");
    let rev_dir = reviews_dir(root);
    let r1 = rev_dir.join("REVIEW_plan_R1.md");
    let r2 = rev_dir.join("REVIEW_plan_R2.md");

    if !plan_path.exists() {
        return Err(anyhow!(
            "plan.md not found. Run `anvil plan` first to generate and review it."
        ));
    }
    if !r1.exists() || !r2.exists() {
        return Err(anyhow!(
            "Both review files (REVIEW_plan_R1.md and REVIEW_plan_R2.md) must exist before accepting.\n\
             Run `anvil plan` (which always runs both reviews) then address the findings."
        ));
    }

    let plan_txt = fs::read_to_string(&plan_path)?;
    let hash = simple_hash(&plan_txt);

    let mut state = load_state(root);

    // Warn if re-accepting (plan may have changed since reviews were written).
    if state.accepted_plan_hash.is_some() {
        println!(
            "{} Re-accepting plan. Make sure the two review files still cover the current plan.md.",
            "Warning:".yellow()
        );
    }

    state.accepted_plan_hash = Some(hash.clone());
    save_state(root, &state)?;

    println!(
        "{} Plan accepted. Hash {} recorded in .anvil/state.json.",
        "✓".green().bold(),
        &hash[..8]
    );
    println!("  R1: {}", r1.display());
    println!("  R2: {}", r2.display());
    println!("\n{} Start building phases (in TUI: chat with coder, /phase-start P0, coder writes review docs on done, sequential critical reviews with human approve between):", "Next:".green());
    println!(
        "  {} P0   — set current phase",
        "`anvil phase start`".cyan()
    );
    Ok(())
}

/// Build the user message for plan generation.
/// If --context was given, reads that file and prepends it.
/// Otherwise looks for saved talk artifacts in reviews/ as optional context.
fn build_plan_prompt(root: &Path, context_file: Option<&Path>) -> Result<String> {
    let base = "Produce the phased plan now for the project. Make it concrete and reviewable.";

    if let Some(ctx_path) = context_file {
        // Resolve relative to project root if not absolute.
        let resolved = if ctx_path.is_absolute() {
            ctx_path.to_path_buf()
        } else {
            root.join(ctx_path)
        };
        let content = fs::read_to_string(&resolved)
            .map_err(|e| anyhow!("Could not read context file {}: {}", resolved.display(), e))?;
        println!(
            "  {} Using context from {} ({} chars)",
            "✓".green(),
            resolved.display(),
            content.len()
        );
        Ok(format!(
            "Here is the project context / charter to plan from:\n\n---\n{}\n---\n\n{}",
            content, base
        ))
    } else {
        Ok(base.to_string())
    }
}

pub fn run_single_review(
    client: &LlmClient,
    cfg: &crate::config::AnvilConfig,
    reviewer_role: &str,
    content: &str,
    round: &str,
    root: &Path,
    artifact: &str,
) -> Result<String> {
    // Accepts either a role keyword ("reviewer_a"/"reviewer_b") or the bound
    // binding name stored in that role — callers pass the latter.
    let (name, binding, provider) = cfg
        .resolve_role_or_binding(reviewer_role)
        .map_err(|_| anyhow!("reviewer role '{}' is not fully configured", reviewer_role))?;

    let api_key = client.get_credential(&binding.provider, provider)?;

    // The reviewer is an *investigator*, not a rubber stamp. The hard-won lesson:
    // every frontier coder (Claude/GPT/Grok) will at times claim work it did not
    // do — so the reviewer must confirm against the real files, never the handoff.
    let system = "You are a skeptical, experienced engineer from a *different* model family than the coder/implementer. \
                  Your job is to find real problems: work the implementer claims but did NOT actually do, scope drift, hidden risks, missing or broken tests, and weak acceptance criteria. \
                  CRITICAL: do NOT trust the summary or diff you are given — coders frequently report work as done when it is not. \
                  You have READ-ONLY tools (read_file, list_dir, grep, project_state). USE THEM to verify against the actual files on disk: open the files the change claims to touch, confirm the code really exists and does what is claimed, check that tests exist and cover it, and confirm earlier phases' acceptance criteria still hold. \
                  Base every finding on what you actually read; cite exact file paths and line numbers. Do not be nice. Be specific. \
                  When you have investigated enough, output a structured review with sections: ## Summary, ## High, ## Medium, ## Low, ## Questions. \
                  In ## Summary, state explicitly what you independently verified in the code versus what you could not confirm.";

    let user = format!(
        "Review the following artifact ({round}). The content below is the implementer's CLAIM, not ground truth — verify it against the real files with your tools before trusting any of it.\n\n--- CONTENT ---\n{content}\n--- END CONTENT ---\n\nInvestigate with your read-only tools, then produce the structured review.",
    );

    // Agentic read-only loop: the reviewer may read/grep/list the repo to confirm
    // claims, then writes its findings. Runs inside the TUI alternate screen, so no
    // stdout here; the header shows live "R1/R2 reviewing — <model>" status.
    let tools = crate::tools::read_only_tool_defs();
    let findings = LlmClient::block_on(async {
        // Sink for streamed deltas — kept in scope so sends don't fail; the gate
        // surfaces progress via the header, not this stream.
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let mut history = vec![ChatMessage::user(user)];
        let mut final_text = String::new();
        for _ in 0..REVIEWER_MAX_STEPS {
            let turn = client
                .chat_turn_stream(
                    provider,
                    &binding.model,
                    &api_key,
                    system,
                    &history,
                    &tools,
                    tx.clone(),
                )
                .await?;
            history.push(ChatMessage::assistant(
                turn.text.clone(),
                turn.tool_calls.clone(),
            ));
            if turn.tool_calls.is_empty() {
                final_text = turn.text;
                break;
            }
            for call in &turn.tool_calls {
                let result = crate::tools::execute(call, root);
                history.push(ChatMessage::tool_result(call.id.clone(), result));
            }
        }
        // Hit the step cap mid-investigation — force the writeup with no tools.
        if final_text.trim().is_empty() {
            history.push(ChatMessage::user(
                "Stop investigating now and output the structured review based on what you have verified so far.".to_string(),
            ));
            let turn = client
                .chat_turn_stream(
                    provider,
                    &binding.model,
                    &api_key,
                    system,
                    &history,
                    &[],
                    tx.clone(),
                )
                .await?;
            final_text = turn.text;
        }
        Ok::<String, anyhow::Error>(final_text)
    })?;

    let out_path = reviews_dir(root).join(format!("REVIEW_{}_{}.md", artifact, round));
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

/// Run a *critical* review (R1 or R2) against a *review document that the coder wrote*
/// (e.g. the structured REVIEW_Px_R1.md briefing the coder produced after declaring a phase done).
/// Writes REVIEW_<artifact>_<round>.md (caller chooses artifact like "P0-R1-doc" to get REVIEW_P0-R1-doc_R1.md,
/// or use "P0_R1_Findings" style). Uses a prompt that tells the reviewer it is critiquing the implementer's
/// own R1/R2 briefing doc, not the raw code/plan.
#[allow(dead_code, clippy::too_many_arguments)]
pub fn run_critical_review_on_doc(
    client: &LlmClient,
    cfg: &crate::config::AnvilConfig,
    reviewer_role: &str,
    doc_content: &str,
    round: &str,
    reviews_dir: &Path,
    artifact: &str,
    extra_context: &str,
) -> Result<String> {
    // Accepts either a role keyword ("reviewer_a"/"reviewer_b") or the bound
    // binding name stored in that role — callers pass the latter.
    let (name, binding, provider) = cfg
        .resolve_role_or_binding(reviewer_role)
        .map_err(|_| anyhow!("reviewer role '{}' is not fully configured", reviewer_role))?;

    let api_key = client.get_credential(&binding.provider, provider)?;

    let system = "You are a skeptical, experienced engineer from a *different* model family than the coder who wrote the review briefing. \
                  The coder/implementer just wrote a structured R1 (or R2) review document claiming what was built, decisions, test coverage and risks. \
                  Your job is to critically audit that document itself: does the evidence actually support the claims? Are success criteria truly met? \
                  Are risks understated? Is the 'What Was Built' table accurate and complete? Be direct and specific. Cite the briefing's own sections. \
                  Output: ## Summary, ## High (must-fix), ## Medium, ## Low, ## Risks understated, ## Recommendations.";

    let user = format!(
        "Critically review the following R{} review document written by the coder/implementer.\n\n{}\n\n--- REVIEW DOC ---\n{}\n--- END REVIEW DOC ---\n\nProduce the structured critical findings now.",
        round, extra_context, doc_content
    );

    // No stdout (would corrupt the TUI alternate screen).
    let findings =
        LlmClient::block_on(client.chat(provider, &binding.model, &api_key, system, &user))?;

    // Write as e.g. REVIEW_P0_R1_Findings.md or REVIEW_P0-R1-doc_R1.md depending on what caller passes as artifact.
    let out_path = reviews_dir.join(format!("REVIEW_{}_{}.md", artifact, round));
    let header = format!(
        "# {} — Critical {} ({} on coder's review doc)\n\n**Reviewer:** {} ({} via {})\n**Date:** {}\n\n",
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

/// One-round R1 review of the current plan.md (used by TUI /lock-plan).
/// Writes REVIEW_plan_R1.md at root and returns the findings text.
pub fn run_plan_r1(root: &Path) -> Result<String> {
    load_local_env(root);
    let cfg = load_config(root)?;
    let client = LlmClient::new();
    let reviews = reviews_dir(root);
    fs::create_dir_all(&reviews)?;

    let plan_path = root.join("plan.md");
    if !plan_path.exists() {
        return Err(anyhow!("plan.md not found — coder must write it (via chat) and user must save it to disk before /lock-plan."));
    }
    let plan_content = fs::read_to_string(&plan_path)?;

    let reviewer_a = cfg
        .roles
        .reviewer_a
        .as_deref()
        .ok_or_else(|| anyhow!("reviewer-a role not configured. Run `anvil setup`."))?;

    let findings = run_single_review(&client, &cfg, reviewer_a, &plan_content, "R1", root, "plan")?;
    Ok(findings)
}

/// One-round R2 review of the current plan.md (used by TUI /approve-r1 after coder incorporated R1 findings).
/// Writes REVIEW_plan_R2.md at root and returns the findings text.
pub fn run_plan_r2(root: &Path) -> Result<String> {
    load_local_env(root);
    let cfg = load_config(root)?;
    let client = LlmClient::new();

    let plan_path = root.join("plan.md");
    if !plan_path.exists() {
        return Err(anyhow!("plan.md not found."));
    }
    let plan_content = fs::read_to_string(&plan_path)?;

    let reviewer_b = cfg
        .roles
        .reviewer_b
        .as_deref()
        .ok_or_else(|| anyhow!("reviewer-b role not configured. Run `anvil setup`."))?;

    let findings = run_single_review(&client, &cfg, reviewer_b, &plan_content, "R2", root, "plan")?;
    Ok(findings)
}

pub fn simple_hash(s: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
