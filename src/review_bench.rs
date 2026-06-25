//! Reviewer-role benchmark — measure how well a model performs the R1/R2 REVIEWER
//! role, so the model-findings doc can rate reviewing, not just coding.
//!
//! Each case is a diff with a planted defect (plus a decoy with no defect) and a
//! briefing of what the change is meant to do. The model under test reviews the
//! diff (pure diff, no tools), and a strong `--judge` model decides whether the
//! review CAUGHT the planted defect — robust to phrasing, the way a keyword match
//! is not. Decoy cases (bug starting with "NONE") measure false positives.
//!
//! Like the coder bench, this is a dev tool: run from the Anvil source tree where
//! `bench/review_fixtures/` lives, against configured providers.

use std::path::Path;

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;

use crate::config::{load_config, load_local_env, AnvilConfig, ProviderConnection};
use crate::llm::LlmClient;

/// The reviewer prompt mirrors Anvil's real second-opinion reviewer (see
/// `phase.rs`), generalized to a pure-diff review: verdict + specific issues +
/// risks, skeptical but not invented.
const REVIEWER_SYSTEM: &str = "You are performing a mandatory second-opinion code review — a \
    different model family from the implementer, which is the whole point of the independent eye. \
    You are given a short briefing of what the change is meant to do, then the diff under review. \
    Find REAL defects: correctness bugs, off-by-one / boundary errors, missing edge cases (e.g. empty \
    input), broken or overly-broad error handling, regressions. Be precise and skeptical of the code, \
    but do NOT invent problems — if the diff is correct, say so plainly. Output exactly these sections: \
    ## Verdict (Pass / Needs Work), ## Specific Issues (each: the concrete defect and why it is wrong), \
    ## Risks.";

#[derive(Deserialize)]
struct CaseToml {
    title: String,
    briefing: String,
    bug: String,
}

struct ReviewCase {
    id: String,
    #[allow(dead_code)]
    title: String,
    briefing: String,
    /// Ground-truth defect for the judge. Starts with "NONE" for a decoy (correct code).
    bug: String,
    diff: String,
}

/// A decoy case (correct code) — the `bug` field describes why it's fine and starts with NONE.
fn is_decoy(bug: &str) -> bool {
    bug.trim_start().to_ascii_uppercase().starts_with("NONE")
}

fn load_cases(dir: &Path, filter: Option<&str>) -> Result<Vec<ReviewCase>> {
    if !dir.is_dir() {
        bail!(
            "no review fixtures at {} — run `anvil review-bench` from the Anvil source tree",
            dir.display()
        );
    }
    let mut paths: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    paths.sort();

    let mut cases = Vec::new();
    for path in paths {
        let id = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if let Some(only) = filter {
            if id != only {
                continue;
            }
        }
        let case_toml = path.join("case.toml");
        let diff_path = path.join("diff.patch");
        if !case_toml.is_file() || !diff_path.is_file() {
            continue;
        }
        let parsed: CaseToml = toml::from_str(&std::fs::read_to_string(&case_toml)?)?;
        let diff = std::fs::read_to_string(&diff_path)?;
        cases.push(ReviewCase {
            id,
            title: parsed.title,
            briefing: parsed.briefing,
            bug: parsed.bug,
            diff,
        });
    }
    if cases.is_empty() {
        bail!("no review cases found under {}", dir.display());
    }
    Ok(cases)
}

/// Resolve a model spec — a role/binding name, or a raw `<provider>/<model>` — to
/// (label, model id, provider connection, api key). Mirrors the coder bench.
fn resolve_model(
    cfg: &AnvilConfig,
    client: &LlmClient,
    key: &str,
) -> Result<(String, String, ProviderConnection, String)> {
    match key.split_once('/') {
        Some((prov, raw)) if cfg.providers.contains_key(prov) => {
            let conn = &cfg.providers[prov];
            let api_key = client.get_credential(prov, conn)?;
            Ok((key.to_string(), raw.to_string(), conn.clone(), api_key))
        }
        _ => {
            let (name, binding, conn) = cfg.resolve_role_or_binding(key).map_err(|_| {
                anyhow!(
                    "'{}' is not a configured role/binding (or a <provider>/<model> spec)",
                    key
                )
            })?;
            let api_key = client.get_credential(&binding.provider, conn)?;
            Ok((
                name.to_string(),
                binding.model.clone(),
                conn.clone(),
                api_key,
            ))
        }
    }
}

/// Ask the judge whether the review caught the planted defect (or, for a decoy,
/// whether it avoided a false positive). Returns Ok(true) for a good outcome.
#[allow(clippy::too_many_arguments)]
fn judge_caught(
    client: &LlmClient,
    conn: &ProviderConnection,
    model: &str,
    api_key: &str,
    case: &ReviewCase,
    review: &str,
    decoy: bool,
) -> Result<bool> {
    let (system, prompt) = if decoy {
        (
            "You are scoring a code review for FALSE POSITIVES. The diff is actually correct. \
             A minor style nit is still CLEAN; inventing a substantive bug that is not there is \
             FALSE_POSITIVE. Answer with exactly CLEAN or FALSE_POSITIVE on the first line, then \
             one short sentence."
                .to_string(),
            format!(
                "The diff is CORRECT — no real defect:\n{}\n\nReviewer's output:\n{}\n\nDid the \
                 reviewer avoid inventing a substantive bug? First line: CLEAN or FALSE_POSITIVE.",
                case.bug, review
            ),
        )
    } else {
        (
            "You are scoring whether a code review CAUGHT a specific known defect. Judge by \
             substance, not exact wording or line numbers. Mentioning only unrelated issues does \
             not count. Answer with exactly CAUGHT or MISSED on the first line, then one short \
             sentence."
                .to_string(),
            format!(
                "KNOWN defect in the diff:\n{}\n\nReviewer's output:\n{}\n\nDid the review identify \
                 this specific defect? First line: CAUGHT or MISSED.",
                case.bug, review
            ),
        )
    };

    let verdict = LlmClient::block_on(client.chat(conn, model, api_key, &system, &prompt))?;
    let first = verdict
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_ascii_uppercase();
    Ok(if decoy {
        first.contains("CLEAN") && !first.contains("FALSE")
    } else {
        first.contains("CAUGHT")
    })
}

/// Run the reviewer benchmark: every case × `runs`, scored by the judge.
pub fn run_review_bench(
    root: &Path,
    target_key: Option<&str>,
    judge_spec: &str,
    runs: usize,
    case_filter: Option<&str>,
) -> Result<()> {
    load_local_env(root);
    let cfg = load_config(root)?;
    let client = LlmClient::new();

    let target = target_key.unwrap_or("coder");
    let (tlabel, tmodel, tconn, tkey) = resolve_model(&cfg, &client, target)?;
    let (jlabel, jmodel, jconn, jkey) = resolve_model(&cfg, &client, judge_spec)?;

    let cases = load_cases(&root.join("bench").join("review_fixtures"), case_filter)?;

    println!(
        "Reviewer benchmark — target '{}' (model {}), judge '{}' (model {}), {} run(s)/case\n",
        tlabel, tmodel, jlabel, jmodel, runs
    );
    println!("{:<26} {:<8} {}", "case", "kind", "result");

    let (mut bug_pass, mut bug_done) = (0usize, 0usize);
    let (mut decoy_pass, mut decoy_done) = (0usize, 0usize);

    for case in &cases {
        let decoy = is_decoy(&case.bug);
        let (mut pass, mut done) = (0usize, 0usize);
        for _ in 0..runs {
            let user = format!(
                "BRIEFING (what this change is meant to do):\n{}\n\nDIFF UNDER REVIEW:\n```diff\n{}```",
                case.briefing, case.diff
            );
            let review = match LlmClient::block_on(client.chat(
                &tconn,
                &tmodel,
                &tkey,
                REVIEWER_SYSTEM,
                &user,
            )) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("  [{}] reviewer call failed: {}", case.id, e);
                    continue;
                }
            };
            match judge_caught(&client, &jconn, &jmodel, &jkey, case, &review, decoy) {
                Ok(good) => {
                    done += 1;
                    if good {
                        pass += 1;
                    }
                }
                Err(e) => eprintln!("  [{}] judge call failed: {}", case.id, e),
            }
        }
        let kind = if decoy { "decoy" } else { "bug" };
        println!("{:<26} {:<8} {}/{}", case.id, kind, pass, done);
        if decoy {
            decoy_pass += pass;
            decoy_done += done;
        } else {
            bug_pass += pass;
            bug_done += done;
        }
    }

    println!("{}", "-".repeat(46));
    if bug_done > 0 {
        println!("catch-rate (planted bugs):       {}/{}", bug_pass, bug_done);
    }
    if decoy_done > 0 {
        println!(
            "clean-rate (decoys, no false +): {}/{}",
            decoy_pass, decoy_done
        );
    }
    println!(
        "\n(catch-rate = bugs the reviewer flagged; clean-rate = decoys it correctly left alone)"
    );
    Ok(())
}
