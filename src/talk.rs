//! `anvil talk` — interactive, streaming conversation with a chosen model binding.
//!
//! This is the "vibe" entry point. Use it to explore the problem, surface goals,
//! constraints, success criteria, and risky assumptions before you commit to a plan.
//!
//! You can ask the model to emit structured artifacts (charter, draft plan, open questions list)
//! inside <artifact>...</artifact> tags and the CLI will offer to save them.

use std::io::{BufRead, Write};
use std::path::Path;

use anyhow::{anyhow, Result};
use colored::Colorize;

use crate::config::load_config;
use crate::llm::LlmClient;

const DEFAULT_SYSTEM: &str = "\
You are a thoughtful technical thought partner. The user is doing vibe-driven coding and wants to avoid drift.

Help them clarify:
- What the project actually is (and is not)
- Goals and non-goals
- Success criteria that are observable
- Key risks and open questions
- Rough shape of a phased plan (but do not produce the final plan yet)

Be direct, ask clarifying questions, and surface assumptions. When the user seems ready, you can offer to emit a structured summary inside <artifact name=\"charter\">...</artifact> or <artifact name=\"plan-draft\">...</artifact> tags. The user can then save those to disk.";

pub fn run_talk(root: &Path, role_or_binding: Option<&str>) -> Result<()> {
    let cfg = load_config(root)?;
    let client = LlmClient::new();

    // Resolve which binding to use
    let (binding_name, binding, provider) = if let Some(r) = role_or_binding {
        if let Ok(full) = cfg.resolve_role_full(r) {
            full
        } else {
            // treat as explicit binding name
            let b = cfg.get_binding(r)?;
            let p = cfg.get_provider(&b.provider)?;
            (r, b, p)
        }
    } else {
        cfg.resolve_role_full("coder")
            .map_err(|_| anyhow!("No coder role configured. Run `anvil setup` first."))?
    };

    let api_key = client.get_credential(binding_name, provider)?;

    println!(
        "\n{} Talking with {} ({} via {})",
        "anvil".green(),
        binding_name.cyan(),
        binding.model,
        provider.r#type
    );
    println!("Type your message. Commands: {} to finish, {} to save last response, {} to quit.", ":done".yellow(), ":save".yellow(), ":q".yellow());
    println!("───────────────────────────────────────────────────────────────────────────────\n");

    let mut history: Vec<(String, String)> = vec![]; // (role, content)
    let stdin = std::io::stdin();

    // Seed with a light opening from the model
    let opening = "Let's explore the project. What's the real goal, and what would success look like in concrete terms?";
    print!("{} ", "Model:".dimmed());
    std::io::stdout().flush().ok();

    let first = LlmClient::block_on(client.chat_stream(
        provider,
        &binding.model,
        &api_key,
        DEFAULT_SYSTEM,
        opening,
    ))?;
    println!(); // ensure newline

    history.push(("assistant".to_string(), first.clone()));

    loop {
        print!("\n{} ", "You:".blue());
        std::io::stdout().flush().ok();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break;
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        match input {
            ":q" | ":quit" | ":exit" => {
                println!("Exiting talk session.");
                break;
            }
            ":done" => {
                println!("Wrapping up. If you want a structured artifact, ask the model one more time with explicit instructions, then :save.");
                // Let the model give a final summary turn
                let prompt = "Summarize the key decisions, goals, out-of-scope items, risks, and a very rough phase breakdown. Emit a clean charter-style artifact if it would be useful.";
                history.push(("user".to_string(), prompt.to_string()));
                let _ = LlmClient::block_on(run_turn(&client, provider, &binding.model, &api_key, &history));
                break;
            }
            ":save" => {
                // Save the last assistant message
                if let Some((_, last)) = history.iter().rev().find(|(r, _)| r == "assistant") {
                    save_artifact(root, last)?;
                } else {
                    println!("Nothing to save yet.");
                }
                continue;
            }
            other if other.starts_with(":save ") => {
                if let Some((_, last)) = history.iter().rev().find(|(r, _)| r == "assistant") {
                    let name = other.trim_start_matches(":save ").trim();
                    save_named_artifact(root, name, last)?;
                }
                continue;
            }
            _ => {}
        }

        history.push(("user".to_string(), input.to_string()));

        let response = LlmClient::block_on(run_turn(&client, provider, &binding.model, &api_key, &history))?;
        history.push(("assistant".to_string(), response));
    }

    println!("\n{} Talk session ended. Next: {} (then the plan will be reviewed by your two configured reviewers).", "✓".green(), "`anvil plan`".cyan());
    Ok(())
}

async fn run_turn(
    client: &LlmClient,
    provider: &crate::config::ProviderConnection,
    model: &str,
    api_key: &str,
    history: &[(String, String)],
) -> Result<String> {
    // For simplicity we rebuild a flat prompt each turn.
    // A more sophisticated version would keep proper message history.
    let mut user_turn = String::new();
    for (role, content) in history {
        match role.as_str() {
            "user" => user_turn.push_str(&format!("\nUser: {}\n", content)),
            "assistant" => user_turn.push_str(&format!("\nAssistant: {}\n", content)),
            _ => {}
        }
    }

    print!("{} ", "Model:".dimmed());
    std::io::stdout().flush().ok();

    let full = client
        .chat_stream(provider, model, api_key, DEFAULT_SYSTEM, &user_turn)
        .await?;

    println!(); // final newline after stream
    Ok(full)
}

fn save_artifact(root: &Path, content: &str) -> Result<()> {
    save_named_artifact(root, "artifact", content)
}

fn save_named_artifact(root: &Path, suggested_name: &str, content: &str) -> Result<()> {
    use std::fs;

    let reviews = crate::state::reviews_dir(root);
    fs::create_dir_all(&reviews)?;

    // Try to extract a name from <artifact name="foo"> if present
    let name = extract_artifact_name(content).unwrap_or_else(|| suggested_name.to_string());
    let safe = sanitize_filename(&name);
    let path = reviews.join(format!("{}.md", safe));

    fs::write(&path, content)?;
    println!("{} Saved to {}", "✓".green(), path.display());
    Ok(())
}

fn extract_artifact_name(s: &str) -> Option<String> {
    if let Some(start) = s.find("<artifact") {
        if let Some(end) = s[start..].find('>') {
            let tag = &s[start..start + end + 1];
            if let Some(n) = tag.split("name=\"").nth(1) {
                if let Some(endq) = n.find('"') {
                    return Some(n[..endq].to_string());
                }
            }
        }
    }
    None
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
