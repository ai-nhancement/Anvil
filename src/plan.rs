//! `anvil plan` (and TUI /plan support)
//!
//! Interactive flow (preferred): the *coder* discusses and writes plan.md inside
//! the TUI chat. Then the plan gate runs as a single sequential loop:
//!   /lock-plan   -> R1 (reviewer-a) reviews plan.md (writes REVIEW_plan_R1.md)
//!                   -> coder applies fixes -> (user /continue)
//!                   -> R2 (reviewer-b) re-reviews the revised plan -> coder fixes
//!                   -> (user /continue) -> coder summarizes
//!   /accept-plan -> records the plan hash, unlocks phase work.
//!
//! The legacy one-shot `anvil plan --fresh` still generates via the coder +
//! immediately runs both R1+R2 (for scripts/CLI users). Both paths write
//! REVIEW_plan_R*.md at the repo root.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use colored::Colorize;

use crate::config::{load_config, load_local_env};
use crate::llm::{ChatMessage, LlmClient};
use crate::state::{active_plan_path, load_state, reviews_dir, save_state};

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

    let plan_path = active_plan_path(root);
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
    let plan_path = active_plan_path(root);
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
    // Boundary before P0 so the first phase's review diffs from here.
    state.phase_base = crate::phase::git_head_sha(root);
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

    // Lean by design: capable models review well with little instruction. The one
    // thing that must stay is "verify against the real files with your tools" —
    // in the manual workflow the human did that; here the reviewer must, because
    // coders sometimes report work as done when it isn't.
    // A reviewer binding may set `contract` (a tier alias like "reviewer", or a path)
    // to run under a bench-tuned reviewer contract — e.g. one with a no-false-alarm
    // clause for a model that over-flags. Unset, or a name that doesn't resolve, uses
    // this built-in default. (Validate a reviewer contract first: `anvil review-bench
    // --contract …`; see contracts/MODEL_FINDINGS.md.)
    let default_system = "You are a skeptical senior engineer critically reviewing another model's implementation. \
                  Find real errors, bugs, risks, scope drift, and missing or weak tests, and suggest improvements. \
                  You have read-only tools (read_file, list_dir, grep, project_state) — use them to verify the work against the actual files rather than trusting the implementer's claims, which are sometimes wrong. \
                  Present findings in order of priority, highest first, citing exact file:line, then suggested improvements. Do NOT write code.";
    let system: String = match binding.contract.as_deref() {
        Some(name) => {
            crate::contracts::resolve(name, root).unwrap_or_else(|| default_system.to_string())
        }
        None => default_system.to_string(),
    };

    // The coder's durable decisions / known-issues — so the reviewer doesn't
    // re-flag things intentionally deferred (e.g. a test that hangs, skipped on
    // purpose), which was sending the coder into a redo loop.
    let decisions_block = std::fs::read_to_string(crate::agent::decisions_path(root))
        .ok()
        .filter(|c| has_meaningful_body(c))
        .map(|d| {
            format!(
                "--- KNOWN DECISIONS / DEFERRALS (.anvil/decisions.md — already accepted; don't re-flag these) ---\n{}\n---\n\n",
                crate::reality::cap(&d, 4000)
            )
        })
        .unwrap_or_default();

    // Read-only reference repos the reviewer may consult to verify the work against
    // a predecessor/sibling codebase (addressed as @name/path).
    let references_block = crate::agent::references_block(cfg)
        .map(|b| format!("{b}\n"))
        .unwrap_or_default();

    // Reviewer memory: the earlier review findings from previous phases/rounds, so
    // the reviewer has continuity (what was already raised, accepted, or fixed) the
    // way a retained chat thread would — instead of starting blind each time.
    let prior_reviews = prior_reviews_digest(root, artifact, 10_000);

    // A plan review and a phase review judge fundamentally different things, and the
    // framing must match the artifact:
    //   - a *plan* review judges a DOCUMENT describing work that is NOT yet built —
    //     nothing is supposed to exist on disk, so "verify the work / no regressions"
    //     is wrong and pushes the reviewer to flag the absent build as defects.
    //   - a *phase* review judges a CODE DIFF (phase base → current tree), where
    //     verifying the work against the real files and checking for regressions is
    //     exactly right.
    // Conflating the two made an R2 reviewer report a sound plan as "every acceptance
    // criterion unmet" because no code was built yet — so branch on the artifact.
    let is_r2 = round.eq_ignore_ascii_case("R2");
    let is_plan = artifact.eq_ignore_ascii_case("plan");

    // Base instruction: for a plan, point the reviewer at the document's soundness and
    // explicitly forbid treating the unbuilt repo as a defect (tools are still useful
    // for cross-checking the plan against referenced docs). For a phase, keep the
    // verify-against-the-files framing.
    let review_instruction = if is_plan {
        format!(
            "Critically review the {round} of the plan \"{artifact}\" below. It describes work that is NOT yet implemented — review the plan's soundness (scope, phasing, acceptance criteria, internal consistency, terminology), NOT the repo's build state. Use your read-only tools to cross-check the plan against referenced docs and existing files, but do NOT treat unbuilt phases, missing crates/files, or unrun commands as defects — nothing is supposed to be built yet. Give your findings, highest priority first."
        )
    } else {
        format!(
            "Critically review the {round} of \"{artifact}\" below — verify it against the real files with your tools, then give your findings, highest priority first."
        )
    };

    // R2 is the SECOND pass. For a plan, R2 re-reads the (possibly revised) plan text
    // and confirms R1's points were addressed in the document. For a phase, R2 judges
    // the full change (the diff spans the phase base to the current tree, so it already
    // carries the R1 fixes) and checks for regressions.
    let round_note = match (is_r2, is_plan) {
        (true, true) => " This is the SECOND pass — R1 already reviewed the plan (it may since have been revised). Re-review the plan DOCUMENT: confirm R1's points were actually addressed in the plan text, and flag anything R1 missed (R1's findings are below). Do not look for implemented code — this is still a plan, not a build.",
        (true, false) => " This is the SECOND pass — R1 already reviewed and the coder applied fixes. Judge the work as a whole: confirm R1's points were actually fixed, that the fixes caused no regressions, and flag anything R1 missed (R1's findings are below).",
        _ => "",
    };
    let prior_round_block = if is_r2 {
        let r1_path = reviews_dir(root).join(format!("REVIEW_{}_R1.md", artifact));
        let label = if is_plan {
            "R1 FINDINGS (confirm each was addressed in the revised plan)"
        } else {
            "R1 FINDINGS (verify each was fixed and caused no regressions)"
        };
        match std::fs::read_to_string(&r1_path) {
            Ok(f) if !f.trim().is_empty() => format!(
                "\n\n--- {} ---\n{}\n---",
                label,
                crate::reality::cap(&f, 8000)
            ),
            _ => String::new(),
        }
    } else {
        String::new()
    };
    let user = format!(
        "{decisions_block}{references_block}{prior_reviews}{review_instruction}{round_note}\n\n--- CONTENT ---\n{content}\n--- END CONTENT ---{prior_round_block}",
    );

    // Agentic read-only loop: the reviewer may read/grep/list the repo to confirm
    // claims, then writes its findings. Runs inside the TUI alternate screen, so no
    // stdout here; the header shows live "R1/R2 reviewing — <model>" status.
    let tools = crate::tools::read_only_tool_defs();
    // Read-only reference repos the reviewer may consult (e.g. verify the work
    // against a predecessor codebase). Same `@name/...` addressing as the coder.
    let refs = cfg.reference_roots();
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
                    &system,
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
                let result = crate::tools::execute_with_refs(call, root, &refs);
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
                    &system,
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

    let plan_path = active_plan_path(root);
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

    let plan_path = active_plan_path(root);
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

/// True if the markdown has real content beyond headers / HTML-comments /
/// blockquotes — i.e. something worth injecting. A fresh template has none, so we
/// don't feed the reviewer an empty decisions scaffold. Mirrors agent::has_body.
fn has_meaningful_body(content: &str) -> bool {
    content.lines().any(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with('#') && !t.starts_with("<!--") && !t.starts_with('>')
    })
}

/// Reviewer memory: a bounded, newest-first digest of the review findings from
/// EARLIER phases/rounds (REVIEW_<other>_R{1,2}.md), excluding `current_artifact`
/// (the current phase's own R1 is injected separately for an R2 pass). This gives
/// the reviewer continuity across the project — what was already raised, accepted,
/// or fixed — the way a retained chat thread would, instead of starting blind.
fn prior_reviews_digest(root: &Path, current_artifact: &str, budget: usize) -> String {
    let dir = reviews_dir(root);
    let cur_r1 = format!("REVIEW_{current_artifact}_R1.md");
    let cur_r2 = format!("REVIEW_{current_artifact}_R2.md");

    let mut files: Vec<(std::time::SystemTime, String, std::path::PathBuf)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let is_review = name.starts_with("REVIEW_")
                && (name.ends_with("_R1.md") || name.ends_with("_R2.md"));
            if !is_review || name == cur_r1 || name == cur_r2 {
                continue;
            }
            let mtime = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            files.push((mtime, name, e.path()));
        }
    }
    if files.is_empty() {
        return String::new();
    }
    files.sort_by_key(|f| std::cmp::Reverse(f.0)); // newest first

    let mut body = String::new();
    let mut used = 0usize;
    for (_, name, path) in files {
        if used >= budget {
            break;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            let per_file = (budget - used).min(4_000);
            let snippet = crate::reality::cap(content.trim(), per_file);
            let block = format!("\n### {name}\n{snippet}\n");
            used += block.len();
            body.push_str(&block);
        }
    }
    if body.trim().is_empty() {
        return String::new();
    }
    format!(
        "--- PRIOR REVIEWS (your earlier findings from previous phases/rounds — for continuity; don't re-litigate settled points) ---\n{body}\n---\n\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_plan_requires_plan_and_both_reviews_then_records_hash() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // No plan.md → error.
        assert!(accept_plan(root).is_err());

        // Plan present but no reviews → still error.
        std::fs::write(root.join("plan.md"), "# Plan\n\n## P0 — Start\ngoal: x\n").unwrap();
        assert!(accept_plan(root).is_err());

        // Both plan reviews present → accepts and records the plan hash.
        std::fs::write(root.join("REVIEW_plan_R1.md"), "r1 findings").unwrap();
        std::fs::write(root.join("REVIEW_plan_R2.md"), "r2 findings").unwrap();
        accept_plan(root).unwrap();
        assert!(crate::state::load_state(root).accepted_plan_hash.is_some());
    }

    #[test]
    fn prior_reviews_digest_excludes_current_and_includes_others() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("REVIEW_P0_R1.md"), "earlier phase finding").unwrap();
        std::fs::write(root.join("REVIEW_P1_R1.md"), "current phase R1").unwrap();
        // Building the digest for P1 should carry P0's review but not P1's own.
        let digest = prior_reviews_digest(root, "P1", 10_000);
        assert!(digest.contains("earlier phase finding"), "{digest}");
        assert!(!digest.contains("current phase R1"), "{digest}");
        // When the only review present is the current artifact's, the digest is empty.
        let dir2 = tempfile::tempdir().unwrap();
        std::fs::write(dir2.path().join("REVIEW_P0_R1.md"), "only mine").unwrap();
        assert!(prior_reviews_digest(dir2.path(), "P0", 10_000).is_empty());
    }
}
