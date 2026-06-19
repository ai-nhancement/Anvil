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

/// If `line` is a markdown phase header, return its canonical id ("P0", "P1", …).
/// Tolerant of how the coder actually writes them: `## P0`, `## P0 — Name`,
/// `## P0: Name`, `## Phase 0`, `### Phase 1 — Name`, `## p2`. Requires a leading
/// `#` so prose lines mentioning "phase 1" aren't mistaken for headers.
pub(crate) fn phase_id_from_header(line: &str) -> Option<String> {
    let t = line.trim_start();
    if !t.starts_with('#') {
        return None;
    }
    let s = t.trim_start_matches('#').trim();
    let lower = s.to_ascii_lowercase();
    // "Phase 0" / "Phase0" / "Phase: 0", else bare "P0".
    let after = lower
        .strip_prefix("phase")
        .map(|r| r.trim_start_matches([' ', ':', '-', '—']))
        .or_else(|| lower.strip_prefix('p'))?;
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        Some(format!("P{digits}"))
    }
}

/// Best-effort human name from a phase header (cosmetic — used by `phase list`).
fn phase_name(header: &str) -> String {
    let h = header.trim_start_matches('#').trim();
    let start = if h.to_ascii_lowercase().starts_with("phase") {
        5
    } else if h.to_ascii_lowercase().starts_with('p') {
        1
    } else {
        0
    };
    h[start..]
        .trim_start_matches([' ', ':', '-', '—'])
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim_start_matches([' ', ':', '-', '—'])
        .trim()
        .to_string()
}

/// Ordered, de-duplicated canonical phase ids found in `plan.md` text.
pub(crate) fn plan_phase_ids(plan: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    plan.lines()
        .filter_map(phase_id_from_header)
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

/// Extract (id, name) pairs from plan.md.
fn parse_plan_phases(plan: &str) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    plan.lines()
        .filter_map(|line| {
            let id = phase_id_from_header(line)?;
            if !seen.insert(id.clone()) {
                return None;
            }
            let name = phase_name(line);
            Some((
                id,
                if name.is_empty() {
                    "(unnamed)".to_string()
                } else {
                    name
                },
            ))
        })
        .collect()
}

/// Set the current phase (state only — no stdout, so this is safe to call from
/// the TUI). Returns the relevant slice of `plan.md` for that phase, if found,
/// for the caller to display however it likes.
pub fn run_phase_start(root: &Path, id: &str) -> Result<Option<String>> {
    load_local_env(root);
    let id = normalize_phase_id(id);
    let mut state = load_state(root);
    // Record the phase base only if we don't already have one. The boundary of the
    // previous milestone (plan accept / last ship) is the correct start — re-recording
    // HEAD here would skip work the coder already did *before* /phase-start was run
    // (e.g. building P3 then running /phase-start P3 made the review diff empty).
    if state.phase_base.is_none() {
        state.phase_base = git_head_sha(root);
    }
    state.current_phase = Some(id.clone());
    save_state(root, &state)?;

    let plan_path = root.join("plan.md");
    if plan_path.exists() {
        if let Ok(plan) = fs::read_to_string(&plan_path) {
            return Ok(extract_phase(&plan, &id));
        }
    }
    Ok(None)
}

/// Canonicalize a phase id so it matches the `## P0` headers in plan.md and stays
/// consistent across state, review filenames, and excerpt lookup. Accepts `p0`,
/// `P0`, ` P0 ` → `P0`; leaves anything non-`Pn` untouched. Idempotent.
pub(crate) fn normalize_phase_id(id: &str) -> String {
    let t = id.trim();
    if let Some(rest) = t.to_ascii_lowercase().strip_prefix('p') {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            return format!("P{digits}");
        }
    }
    t.to_string()
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

/// Accept (ship) a phase after its R1+R2 reviews exist (state only — no stdout,
/// so it's safe from the TUI). Errors if both review files aren't present.
pub fn run_phase_accept(root: &Path, id: &str) -> Result<()> {
    load_local_env(root);
    let id = &normalize_phase_id(id);
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
                                // Audit trail: annotate plan.md with this phase's closure, using the phase's
                                // base commit (read before we advance it for the next phase). The latched
                                // "accepted" stage means editing plan.md here won't re-trigger the plan gate.
    annotate_phase_closed(root, id, state.phase_base.as_deref());
    state.phase_base = git_head_sha(root); // the next phase's work starts from here
    save_state(root, &state)?;
    Ok(())
}

/// Append a closure record for `id` into its `plan.md` section: date, that it
/// passed R1+R2, the files changed this phase, and links to the review docs.
/// Deterministic and idempotent (skips if already recorded). Best-effort.
fn annotate_phase_closed(root: &Path, id: &str, base: Option<&str>) {
    let plan_path = root.join("plan.md");
    let Ok(plan) = fs::read_to_string(&plan_path) else {
        return;
    };
    let marker = format!("{id} passed R1 + R2");
    if plan.contains(&marker) {
        return; // already recorded (re-shipped) — don't duplicate
    }
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut block =
        format!("\n> **CLOSED {date} — {id} passed R1 + R2 review and was accepted.**\n");
    let stat = phase_diff_stat(root, base);
    if !stat.is_empty() {
        block.push_str(">\n> Files changed this phase:\n");
        for line in stat.lines() {
            block.push_str(&format!("> - {line}\n"));
        }
    }
    block.push_str(&format!(
        "> Reviews: REVIEW_{id}_R1.md, REVIEW_{id}_R2.md\n"
    ));
    let updated = insert_in_phase_section(&plan, id, &block);
    let _ = fs::write(&plan_path, updated);
}

/// `git diff --stat` for the phase's change set (per-file lines only), from the
/// phase base if known, else the most recent commit. Best-effort, capped.
fn phase_diff_stat(root: &Path, base: Option<&str>) -> String {
    let git = |args: &[&str]| -> Option<String> {
        std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let out = match base {
        Some(b) => git(&["diff", "--stat", b]),
        None => git(&["diff", "--stat", "HEAD~1", "HEAD"]),
    }
    .unwrap_or_default();
    out.lines()
        .filter(|l| l.contains('|')) // per-file rows, not the "N files changed" summary
        .take(25)
        .map(|l| l.trim().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Insert `block` at the end of `id`'s section in the plan (just before the next
/// phase header / risks section), or append it if the section isn't found.
fn insert_in_phase_section(plan: &str, id: &str, block: &str) -> String {
    let want = normalize_phase_id(id);
    let lines: Vec<&str> = plan.lines().collect();
    let start = lines
        .iter()
        .position(|l| phase_id_from_header(l).as_deref() == Some(want.as_str()));
    let Some(start) = start else {
        let mut out = plan.to_string();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(block);
        return out;
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, l)| {
            let low = l.to_lowercase();
            phase_id_from_header(l).is_some()
                || low.contains("risk")
                || low.contains("open question")
        })
        .map(|(i, _)| i)
        .unwrap_or(lines.len());
    let mut out = String::new();
    for line in &lines[..end] {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(block);
    out.push('\n');
    for line in &lines[end..] {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Current HEAD commit sha (short), or None outside a git repo / with no commits.
pub(crate) fn git_head_sha(root: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if sha.is_empty() {
        None
    } else {
        Some(sha)
    }
}

/// Capture the phase's change set for review. Diffs from the recorded phase base
/// (`base..worktree`, so *committed* work since the phase started is included),
/// falling back to `git diff HEAD` (uncommitted) and then the most recent commit
/// when no base is recorded — plus the names of any untracked files.
fn capture_git_diff(root: &Path) -> String {
    use std::process::Command;
    let git = |args: &[&str]| -> Option<String> {
        Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    };

    let base = load_state(root).phase_base;
    let mut diff = String::new();
    // 1) Everything since the phase base (commits + uncommitted), if we have one.
    if let Some(b) = base.as_deref() {
        if let Some(d) = git(&["diff", b]) {
            diff = d;
        }
    }
    // 2) Otherwise (or if the base diff is empty) the uncommitted working tree.
    if diff.trim().is_empty() {
        diff = git(&["diff", "HEAD"]).unwrap_or_default();
    }
    // 3) Last resort: the most recent commit, so a single committed phase with no
    //    recorded base is still reviewable (labelled so the reviewer knows).
    if diff.trim().is_empty() {
        if let Some(d) = git(&["diff", "HEAD~1", "HEAD"]) {
            if !d.trim().is_empty() {
                diff = format!(
                    "(no base recorded and no uncommitted changes — showing the most recent commit)\n{d}"
                );
            }
        }
    }
    // New (untracked) files never appear in `git diff` — but a phase is often
    // implemented as brand-new files, so include their *content* (not just names),
    // or the reviewer would think nothing was done.
    if let Some(list) = git(&["ls-files", "--others", "--exclude-standard"]) {
        for name in list.lines().filter(|l| !l.trim().is_empty()).take(40) {
            let is_binary = Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .map(|ext| {
                    crate::tools::SKIP_EXTS
                        .iter()
                        .any(|x| x.eq_ignore_ascii_case(ext))
                })
                .unwrap_or(false);
            if is_binary {
                diff.push_str(&format!("\n--- New file (binary, not shown): {name} ---\n"));
                continue;
            }
            match fs::read_to_string(root.join(name)) {
                Ok(content) => {
                    diff.push_str(&format!("\n--- New file: {name} ---\n"));
                    let capped: String = content.chars().take(20_000).collect();
                    diff.push_str(&capped);
                    if content.len() > capped.len() {
                        diff.push_str("\n… [new file truncated]");
                    }
                    diff.push('\n');
                }
                Err(_) => diff.push_str(&format!("\n--- New file (unreadable): {name} ---\n")),
            }
        }
    }
    if diff.trim().is_empty() {
        return "(no changes since the phase started — nothing to review yet; the coder may not have implemented this phase)".to_string();
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
    // Prefer the focused phase section; if it can't be located, fall back to the
    // whole plan so the reviewer always has the plan to check drift against.
    let excerpt = extract_phase(&plan, id).unwrap_or_else(|| {
        if plan.trim().is_empty() {
            "(plan.md not found or empty — ask the user for the plan)".to_string()
        } else {
            let mut p = plan.clone();
            if p.len() > 16_000 {
                p.truncate(16_000);
                p.push_str("\n… [plan truncated]");
            }
            format!("(phase section '{id}' not found — full plan below)\n{p}")
        }
    });
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
    let id = normalize_phase_id(id);
    let cfg = load_config(root)?;
    let client = LlmClient::new();
    let reviews = reviews_dir(root);
    fs::create_dir_all(&reviews)?;
    let content = build_phase_diff_content(root, &id);
    let reviewer_a = cfg
        .roles
        .reviewer_a
        .as_deref()
        .ok_or_else(|| anyhow!("reviewer-a role not configured. Run `anvil setup`."))?;
    crate::plan::run_single_review(&client, &cfg, reviewer_a, &content, "R1", root, &id)
}

/// R2 of a phase: reviewer-b critiques the current diff. Writes REVIEW_<id>_R2.md.
pub fn run_phase_r2_diff(root: &Path, id: &str) -> Result<String> {
    load_local_env(root);
    let id = normalize_phase_id(id);
    let cfg = load_config(root)?;
    let client = LlmClient::new();
    let reviews = reviews_dir(root);
    fs::create_dir_all(&reviews)?;
    let content = build_phase_diff_content(root, &id);
    let reviewer_b = cfg
        .roles
        .reviewer_b
        .as_deref()
        .ok_or_else(|| anyhow!("reviewer-b role not configured. Run `anvil setup`."))?;
    crate::plan::run_single_review(&client, &cfg, reviewer_b, &content, "R2", root, &id)
}

pub(crate) fn extract_phase(plan: &str, id: &str) -> Option<String> {
    let want = normalize_phase_id(id);
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;
    for line in plan.lines() {
        if let Some(hid) = phase_id_from_header(line) {
            if in_section {
                break; // the next phase header ends this section
            }
            if hid == want {
                in_section = true;
                out.push(line.to_string());
            }
            continue;
        }
        if in_section {
            let low = line.to_lowercase();
            if low.contains("risk") || low.contains("open question") {
                break;
            }
            out.push(line.to_string());
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_phase_id_canonicalizes() {
        assert_eq!(normalize_phase_id("p0"), "P0");
        assert_eq!(normalize_phase_id("P0"), "P0");
        assert_eq!(normalize_phase_id("  p12 "), "P12");
        // Idempotent, and non-Pn input is left alone.
        assert_eq!(normalize_phase_id(&normalize_phase_id("p3")), "P3");
        assert_eq!(normalize_phase_id("setup"), "setup");
    }

    #[test]
    fn extract_phase_finds_section_after_normalization() {
        let plan = "# Plan\n\n## P0 — Bootstrap\ngoal: x\n- do a thing\n\n## P1 — Next\ngoal: y\n";
        // The user typed "p0"; normalizing lets extract_phase locate "## P0".
        let id = normalize_phase_id("p0");
        let sec = extract_phase(plan, &id).expect("section found");
        assert!(sec.contains("P0 — Bootstrap"), "{sec}");
        assert!(sec.contains("do a thing"), "{sec}");
        assert!(!sec.contains("P1 — Next"), "{sec}");
    }

    #[test]
    fn header_parsing_tolerates_phase_word_and_case() {
        // The coder may write any of these; all must canonicalize to P0/P1/P2.
        assert_eq!(
            phase_id_from_header("## P0 — Bootstrap").as_deref(),
            Some("P0")
        );
        assert_eq!(
            phase_id_from_header("### Phase 1: Build").as_deref(),
            Some("P1")
        );
        assert_eq!(phase_id_from_header("## phase2").as_deref(), Some("P2"));
        assert_eq!(
            phase_id_from_header("## Phase 3 - Ship").as_deref(),
            Some("P3")
        );
        // Not headers / not phases.
        assert_eq!(phase_id_from_header("We finished phase 1 today"), None); // no leading #
        assert_eq!(phase_id_from_header("## Planning"), None);
        assert_eq!(phase_id_from_header("## Performance notes"), None);
    }

    #[test]
    fn plan_phase_ids_and_extract_work_with_phase_word_headers() {
        let plan = "# Plan\n\n## Phase 0 — Bootstrap\ngoal: x\n- do a thing\n\n## Phase 1: Next\ngoal: y\n";
        assert_eq!(
            plan_phase_ids(plan),
            vec!["P0".to_string(), "P1".to_string()]
        );
        // A user typing "p0" still locates the "## Phase 0" section.
        let sec = extract_phase(plan, "p0").expect("section found");
        assert!(sec.contains("Phase 0 — Bootstrap"), "{sec}");
        assert!(sec.contains("do a thing"), "{sec}");
        assert!(!sec.contains("Phase 1"), "{sec}");
    }

    #[test]
    fn insert_in_phase_section_places_block_inside_the_right_phase() {
        let plan =
            "# Plan\n\n## P0 — Bootstrap\ngoal: x\n\n## P1 — Next\ngoal: y\n\n## Risks\n- a risk\n";
        let out = insert_in_phase_section(plan, "P0", "> CLOSED P0 note\n");
        // Block lands in the P0 section, before P1.
        let p0 = out.find("P0 — Bootstrap").unwrap();
        let note = out.find("CLOSED P0 note").unwrap();
        let p1 = out.find("P1 — Next").unwrap();
        assert!(p0 < note && note < p1, "{out}");
        // P1 and Risks remain intact and after the note.
        assert!(out.contains("## P1 — Next"));
        assert!(out.contains("## Risks"));
    }

    #[test]
    fn phase_diff_captures_committed_work_since_base() {
        // Skip gracefully where git isn't available (the rest of the suite is git-free).
        if std::process::Command::new("git")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(root)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .output()
                .unwrap()
        };
        git(&["init", "-q"]);
        std::fs::write(root.join("a.txt"), "one\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-qm", "base"]);
        let base = git_head_sha(root).expect("head sha");

        // Phase work, then committed — a plain `git diff HEAD` would now be empty.
        std::fs::write(root.join("b.txt"), "two\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-qm", "phase work"]);
        assert!(git(&["diff", "HEAD"]).stdout.is_empty());

        // With the phase base recorded, the review still sees the committed change.
        let mut st = load_state(root);
        st.phase_base = Some(base);
        save_state(root, &st).unwrap();
        let diff = capture_git_diff(root);
        assert!(diff.contains("b.txt"), "{diff}");

        // A brand-new untracked file (a phase built as new files) must show its
        // CONTENT, not just its name — git diff never includes untracked files.
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/p3.js"), "function p3() { return 42; }\n").unwrap();
        let diff2 = capture_git_diff(root);
        assert!(diff2.contains("New file: src/p3.js"), "{diff2}");
        assert!(diff2.contains("function p3"), "{diff2}");
    }

    #[test]
    fn build_phase_diff_falls_back_to_full_plan_when_section_missing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("plan.md"),
            "# Plan\n\n## P0 — Only phase\ngoal: ship it\n",
        )
        .unwrap();
        // A phase id with no matching section → reviewer still gets the full plan.
        let content = build_phase_diff_content(dir.path(), "P9");
        assert!(content.contains("full plan below"), "{content}");
        assert!(content.contains("ship it"), "{content}");
    }
}
