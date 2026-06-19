//! Specialists — scoped, single-purpose sub-agents the coder delegates to (v0.4.0).
//!
//! The principle (borrowed from AiMe's COGS): *an agent reasons over the query and
//! the evidence to produce an answer; a **specialist** reasons only about the
//! **operation** to retrieve evidence, then hands it back to the governed model.*
//! The coder stays the narrator and decision-maker. A specialist is a focused
//! system prompt + a **scoped tool allow-list** + a bounded step budget — a
//! carpenter with one hammer, not a generalist with the whole toolbox. That makes
//! it faster and far less prone to inventing things outside its lane.
//!
//! This first slice ships **evidence-gatherers only** — both read-only, both
//! returning evidence and never touching the working tree:
//!   - `researcher` — web_search + web_fetch + read_file
//!   - `repo-scout` — repo_pull + grep + read_file (scoped to the pulled repo)
//!
//! The coder reaches a specialist through the `delegate` tool (see `tools.rs`),
//! intercepted in the agent loop. Outward actions (fetching a URL, cloning a repo)
//! go through the same y/n confirmation as `run_command`.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde_json::json;
use tokio::sync::mpsc::UnboundedSender;

use crate::agent::ConfirmHandle;
use crate::config::{anvil_dir, AnvilConfig, ProviderConnection};
use crate::llm::{ChatMessage, LlmClient, ToolCall, ToolDef};
use crate::websearch::{self, DEFAULT_WEB_SEARCH_PROVIDER};

/// Safety cap on a specialist's tool-call iterations for a single delegation.
const SPECIALIST_MAX_STEPS: usize = 12;

/// A scoped specialist: a focused system prompt + an allow-list of tool names.
/// The runner builds the actual `ToolDef`s from these names (see `tools_for`).
pub struct SpecialistDef {
    pub name: &'static str,
    /// One-line description surfaced to the coder in the `delegate` tool schema.
    pub description: &'static str,
    pub system_prompt: &'static str,
    pub tool_names: &'static [&'static str],
}

/// The built-in specialist registry. Keep this small and high-signal; writing
/// specialists (test-writer, refactorer, debugger) come in a later 0.4.x slice.
pub const SPECIALISTS: &[SpecialistDef] = &[
    SpecialistDef {
        name: "researcher",
        description: "searches the web and reads pages for external information (library/API docs, usage examples, best practices, error explanations); returns a cited summary",
        system_prompt: "You are a research specialist working under contract for a coding agent. \
            Your ONLY job is to gather external evidence from the web and report it back faithfully — you do not edit code or make decisions. \
            Use web_search to find relevant sources, then web_fetch to read the most promising ones for the actual details (don't rely on snippets alone). \
            Stay tightly on the task you were given; do not wander into unrelated topics. \
            When you have enough, STOP calling tools and write a concise, well-organized findings report: the direct answer first, then key details, then the sources (title + URL) you actually read. \
            Quote exact API signatures, version numbers, and config keys when relevant. If the evidence is thin or contradictory, say so plainly rather than guessing.",
        tool_names: &["web_search", "web_fetch", "read_file"],
    },
    SpecialistDef {
        name: "repo-scout",
        description: "shallow-clones an external git repo and studies how it does something using grep/read_file; returns findings about that codebase",
        system_prompt: "You are a repo-scout specialist working under contract for a coding agent. \
            Your ONLY job is to study an EXTERNAL reference repository and report how it does something — you do not edit our project or make decisions. \
            First call repo_pull with the repo's git URL to clone it locally (read-only). The result tells you the local path it landed in. \
            Then use grep and read_file SCOPED TO THAT PATH to investigate: find the relevant modules, read the real implementation, note the patterns and key APIs. \
            Base every claim on code you actually read; cite exact file paths (and line numbers where useful). \
            When you have enough, STOP calling tools and write a concise findings report: what you were asked, how that repo does it, the specific files/functions that matter, and any caveats.",
        tool_names: &["repo_pull", "grep", "read_file"],
    },
];

/// Look up a specialist by name.
pub fn find(name: &str) -> Option<&'static SpecialistDef> {
    SPECIALISTS.iter().find(|s| s.name == name)
}

/// The specialist names, for the `delegate` tool's `specialist` enum.
pub fn names() -> Vec<&'static str> {
    SPECIALISTS.iter().map(|s| s.name).collect()
}

/// A bulleted "- name: description" listing of every specialist, used in the
/// `delegate` tool description and onboarding/help text.
pub fn help_listing() -> String {
    SPECIALISTS
        .iter()
        .map(|s| format!("- {}: {}", s.name, s.description))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The outward-facing tool defs only a specialist may use (never in the coder's
/// own set): web_search, web_fetch, repo_pull.
fn web_tool_defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "web_search".into(),
            description: "Search the web and get a compact, citation-friendly digest of the top results (title, URL, snippet). Use this to find sources, then web_fetch to read them.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "The search query"}
                },
                "required": ["query"]
            }),
        },
        ToolDef {
            name: "web_fetch".into(),
            description: "Fetch a single http(s) URL and return its readable text (HTML stripped). Use after web_search to read the actual page content. Requires user confirmation.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "The http(s) URL to fetch"}
                },
                "required": ["url"]
            }),
        },
        ToolDef {
            name: "repo_pull".into(),
            description: "Shallow-clone an external git repository locally (read-only) so you can study it with grep/read_file. Returns the local path it was cloned to. Requires user confirmation.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repo_url": {"type": "string", "description": "The git URL to clone (e.g. https://github.com/owner/name)"}
                },
                "required": ["repo_url"]
            }),
        },
    ]
}

/// Build the concrete `ToolDef`s for a specialist by resolving its `tool_names`
/// against the web tools and the coder's base toolset (read_file, grep, …).
fn tools_for(spec: &SpecialistDef) -> Vec<ToolDef> {
    let web = web_tool_defs();
    let base = crate::tools::tool_defs();
    spec.tool_names
        .iter()
        .filter_map(|name| {
            web.iter()
                .find(|d| d.name == *name)
                .or_else(|| base.iter().find(|d| d.name == *name))
                .cloned()
        })
        .collect()
}

/// Read a required string argument from a tool call, or an `Err` to feed back.
fn arg_str(call: &ToolCall, key: &str) -> Result<String> {
    call.arguments
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("missing required string argument '{}'", key))
}

/// Turn a repo URL into a safe single-segment directory name under `.anvil/refs/`.
fn repo_dir_name(repo_url: &str) -> String {
    let trimmed = repo_url.trim().trim_end_matches('/');
    let last = trimmed
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("repo")
        .trim_end_matches(".git");
    let sanitized: String = last
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "repo".to_string()
    } else {
        sanitized
    }
}

/// Run a specialist to completion for one delegation and return its findings text.
///
/// Modeled on `plan::run_single_review` (a bounded read-only investigator), but
/// async (we are already inside the agent's async tool loop) so outward actions
/// can be gated through the live `ConfirmHandle`. The specialist reuses the
/// coder's model connection for this slice; `cfg` supplies the web-search backend.
#[allow(clippy::too_many_arguments)]
pub async fn run_specialist(
    client: &LlmClient,
    cfg: &AnvilConfig,
    conn: &ProviderConnection,
    model: &str,
    api_key: &str,
    root: &Path,
    specialist_name: &str,
    task: &str,
    tx: &UnboundedSender<String>,
    confirm: &mut ConfirmHandle,
) -> Result<String> {
    let spec = find(specialist_name).ok_or_else(|| {
        anyhow!(
            "unknown specialist '{}'. Available: {}",
            specialist_name,
            names().join(", ")
        )
    })?;

    let tools = tools_for(spec);
    let provider = cfg
        .web_search
        .provider
        .as_deref()
        .unwrap_or(DEFAULT_WEB_SEARCH_PROVIDER)
        .to_string();

    let user = format!(
        "Task delegated to you by the coding agent. You have no access to its conversation, so treat this as fully self-contained:\n\n{}\n\nInvestigate with your tools, then report your findings.",
        task.trim()
    );
    let mut history = vec![ChatMessage::user(user)];
    let mut final_text = String::new();

    for _ in 0..SPECIALIST_MAX_STEPS {
        let turn = client
            .chat_turn_stream(
                conn,
                model,
                api_key,
                spec.system_prompt,
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
            let _ = tx.send(format!(
                "[tool-start]{} {}",
                call.name,
                crate::tools::summarize_args(call)
            ));
            let result = dispatch_tool(call, root, &provider, cfg, tx, confirm).await;
            let _ = tx.send(format!(
                "[tool-end]{} {}",
                call.name,
                crate::tools::result_summary(&call.name, &result)
            ));
            history.push(ChatMessage::tool_result(call.id.clone(), result));
        }
    }

    // Hit the step cap mid-investigation — force the writeup with no tools.
    if final_text.trim().is_empty() {
        history.push(ChatMessage::user(
            "Stop investigating now and write your findings report based on what you have gathered so far.".to_string(),
        ));
        let turn = client
            .chat_turn_stream(
                conn,
                model,
                api_key,
                spec.system_prompt,
                &history,
                &[],
                tx.clone(),
            )
            .await?;
        final_text = turn.text;
    }

    if final_text.trim().is_empty() {
        final_text = "(specialist returned no findings)".to_string();
    }
    Ok(format!(
        "Findings from the {} specialist:\n\n{}",
        spec.name, final_text
    ))
}

/// Dispatch one specialist tool call. Web-facing tools run via `websearch`;
/// `web_fetch` and `repo_pull` are gated through the same confirmation as
/// `run_command`. Everything else (read_file, grep) goes to `tools::execute`.
async fn dispatch_tool(
    call: &ToolCall,
    root: &Path,
    provider: &str,
    _cfg: &AnvilConfig,
    tx: &UnboundedSender<String>,
    confirm: &mut ConfirmHandle,
) -> String {
    match call.name.as_str() {
        "web_search" => {
            let query = match arg_str(call, "query") {
                Ok(q) => q,
                Err(e) => return format!("ERROR: {e}"),
            };
            let key_var = websearch::key_env_var(provider);
            let key = std::env::var(key_var).unwrap_or_default();
            if key.trim().is_empty() {
                return format!(
                    "ERROR: web search needs an API key. Set {key_var} in your environment or .anvil/.env (provider: {provider})."
                );
            }
            match websearch::search(&query, provider, &key).await {
                Ok(s) => s,
                Err(e) => format!("ERROR: {e}"),
            }
        }
        "web_fetch" => {
            let url = match arg_str(call, "url") {
                Ok(u) => u,
                Err(e) => return format!("ERROR: {e}"),
            };
            if !confirm.confirm(tx, &format!("web_fetch {url}")).await {
                return "ERROR: fetch was declined by the user".to_string();
            }
            match websearch::fetch(&url).await {
                Ok(s) => s,
                Err(e) => format!("ERROR: {e}"),
            }
        }
        "repo_pull" => {
            let repo_url = match arg_str(call, "repo_url") {
                Ok(u) => u,
                Err(e) => return format!("ERROR: {e}"),
            };
            if !confirm
                .confirm(tx, &format!("git clone --depth 1 {repo_url}"))
                .await
            {
                return "ERROR: repo clone was declined by the user".to_string();
            }
            let dest: PathBuf = anvil_dir(root).join("refs").join(repo_dir_name(&repo_url));
            match websearch::pull_repo(&repo_url, &dest).await {
                Ok(s) => {
                    // Report the path relative to root so the specialist can scope
                    // grep/read_file there with the project-relative paths it expects.
                    let rel = dest
                        .strip_prefix(root)
                        .unwrap_or(&dest)
                        .display()
                        .to_string()
                        .replace('\\', "/");
                    format!("{s}\n(project-relative path: {rel})")
                }
                Err(e) => format!("ERROR: {e}"),
            }
        }
        // read_file, grep, project_state, etc. — sandboxed to the project root.
        _ => crate::tools::execute(call, root),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_both_evidence_gatherers() {
        assert!(find("researcher").is_some());
        assert!(find("repo-scout").is_some());
        assert!(find("nonexistent").is_none());
        assert_eq!(names(), vec!["researcher", "repo-scout"]);
    }

    #[test]
    fn researcher_tools_resolve_and_are_read_only() {
        let spec = find("researcher").unwrap();
        let defs = tools_for(spec);
        let got: Vec<String> = defs.into_iter().map(|d| d.name).collect();
        assert_eq!(got, vec!["web_search", "web_fetch", "read_file"]);
        // A specialist never gets write/exec tools.
        for danger in ["write_file", "edit_file", "apply_patch", "run_command"] {
            assert!(!got.iter().any(|n| n == danger), "leaked {danger}");
        }
    }

    #[test]
    fn repo_scout_tools_resolve() {
        let spec = find("repo-scout").unwrap();
        let got: Vec<String> = tools_for(spec).into_iter().map(|d| d.name).collect();
        assert_eq!(got, vec!["repo_pull", "grep", "read_file"]);
    }

    #[test]
    fn repo_dir_name_is_safe_single_segment() {
        assert_eq!(
            repo_dir_name("https://github.com/owner/cool-repo.git"),
            "cool-repo"
        );
        assert_eq!(
            repo_dir_name("https://github.com/owner/cool-repo/"),
            "cool-repo"
        );
        assert_eq!(repo_dir_name("git@github.com:owner/thing.git"), "thing");
        // No path traversal or separators survive.
        let n = repo_dir_name("../../etc/passwd");
        assert!(!n.contains('/') && !n.contains('.'), "{n}");
    }

    #[test]
    fn help_listing_names_each_specialist() {
        let h = help_listing();
        assert!(h.contains("researcher:"));
        assert!(h.contains("repo-scout:"));
    }
}
