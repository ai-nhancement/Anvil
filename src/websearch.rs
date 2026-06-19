//! Outward-facing operations for evidence-gathering specialists (v0.4.0).
//!
//! These are the tools a *specialist* runs under contract — they retrieve
//! evidence from the outside world (web search results, fetched page text, a
//! shallow clone of a reference repo) and hand it back. They never reason about
//! the evidence; that stays with the governed coder.
//!
//! Web search is pluggable like the LLM providers: the provider is set in the
//! `[web_search]` block of anvil.toml (default `tavily`), and the API key is read
//! from a conventional environment variable (`TAVILY_API_KEY` / `BRAVE_API_KEY`),
//! which Anvil already loads from the global/local `.env`.

use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::Value;

/// Default web-search provider when none is configured.
pub const DEFAULT_WEB_SEARCH_PROVIDER: &str = "tavily";

/// The conventional environment variable holding the API key for `provider`.
pub fn key_env_var(provider: &str) -> &'static str {
    match provider {
        "brave" => "BRAVE_API_KEY",
        // tavily and anything else default to the Tavily convention.
        _ => "TAVILY_API_KEY",
    }
}

fn http() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("anvil-coder/0.4 (+https://anvil.codes)")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow!("failed to build HTTP client: {e}"))
}

/// Run a web search and return a compact, citation-friendly digest of the top
/// results (title, URL, snippet), plus a direct answer when the backend offers one.
pub async fn search(query: &str, provider: &str, api_key: &str) -> Result<String> {
    let query = query.trim();
    if query.is_empty() {
        return Err(anyhow!("empty search query"));
    }
    match provider {
        "brave" => search_brave(query, api_key).await,
        "tavily" => search_tavily(query, api_key).await,
        other => Err(anyhow!(
            "unknown web_search provider '{other}'. Supported: tavily, brave."
        )),
    }
}

async fn search_tavily(query: &str, api_key: &str) -> Result<String> {
    let body = serde_json::json!({
        "api_key": api_key,
        "query": query,
        "max_results": 6,
        "include_answer": true,
        "search_depth": "basic",
    });
    let resp = http()?
        .post("https://api.tavily.com/search")
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow!("tavily request failed: {e}"))?;
    if !resp.status().is_success() {
        let code = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("tavily error ({code}): {}", truncate(&text, 300)));
    }
    let v: Value = resp
        .json()
        .await
        .map_err(|e| anyhow!("tavily bad json: {e}"))?;

    let mut out = format!("Web search results for: {query}\n");
    if let Some(answer) = v.get("answer").and_then(Value::as_str) {
        if !answer.trim().is_empty() {
            out.push_str(&format!("\nDirect answer: {}\n", answer.trim()));
        }
    }
    if let Some(results) = v.get("results").and_then(Value::as_array) {
        out.push_str("\nSources:\n");
        for (i, r) in results.iter().enumerate() {
            let title = r
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("(untitled)");
            let url = r.get("url").and_then(Value::as_str).unwrap_or("");
            let content = r.get("content").and_then(Value::as_str).unwrap_or("");
            out.push_str(&format!(
                "{}. {title}\n   {url}\n   {}\n",
                i + 1,
                truncate(content.trim(), 400)
            ));
        }
    }
    Ok(out)
}

async fn search_brave(query: &str, api_key: &str) -> Result<String> {
    let resp = http()?
        .get("https://api.search.brave.com/res/v1/web/search")
        .query(&[("q", query), ("count", "6")])
        .header("X-Subscription-Token", api_key)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| anyhow!("brave request failed: {e}"))?;
    if !resp.status().is_success() {
        let code = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("brave error ({code}): {}", truncate(&text, 300)));
    }
    let v: Value = resp
        .json()
        .await
        .map_err(|e| anyhow!("brave bad json: {e}"))?;

    let mut out = format!("Web search results for: {query}\n\nSources:\n");
    if let Some(results) = v
        .get("web")
        .and_then(|w| w.get("results"))
        .and_then(Value::as_array)
    {
        for (i, r) in results.iter().enumerate() {
            let title = r
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("(untitled)");
            let url = r.get("url").and_then(Value::as_str).unwrap_or("");
            let desc = r.get("description").and_then(Value::as_str).unwrap_or("");
            out.push_str(&format!(
                "{}. {title}\n   {url}\n   {}\n",
                i + 1,
                truncate(desc.trim(), 400)
            ));
        }
    }
    Ok(out)
}

/// Fetch a URL and return its readable text (HTML tags stripped, whitespace
/// collapsed, truncated). For grabbing the actual content behind a search hit.
pub async fn fetch(url: &str) -> Result<String> {
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(anyhow!("web_fetch needs an http(s) URL, got: {url}"));
    }
    let resp = http()?
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow!("fetch failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("fetch error ({}) for {url}", resp.status()));
    }
    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let raw = resp.text().await.map_err(|e| anyhow!("fetch body: {e}"))?;
    let text = if ctype.contains("html") || raw.trim_start().starts_with('<') {
        strip_html(&raw)
    } else {
        raw
    };
    Ok(format!(
        "Fetched {url}\n\n{}",
        truncate(text.trim(), 12_000)
    ))
}

/// Shallow-clone an external git repo into `dest` for read-only reference. Returns
/// a short summary; the specialist then greps/reads inside `dest`.
pub async fn pull_repo(repo_url: &str, dest: &Path) -> Result<String> {
    let repo_url = repo_url.trim();
    if repo_url.is_empty() {
        return Err(anyhow!("empty repo URL"));
    }
    if dest.exists() {
        return Ok(format!(
            "Reference repo already present at {}. Use grep/read_file scoped there.",
            dest.display()
        ));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| anyhow!("could not create refs dir: {e}"))?;
    }
    let out = tokio::process::Command::new("git")
        .args(["clone", "--depth", "1", repo_url, &dest.to_string_lossy()])
        .output()
        .await
        .map_err(|e| anyhow!("failed to run git: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("git clone failed: {}", truncate(err.trim(), 400)));
    }
    Ok(format!(
        "Cloned {repo_url} (shallow) to {}. Use grep/read_file scoped to that path to study it.",
        dest.display()
    ))
}

/// Very small HTML-to-text: drop <script>/<style> bodies and all tags, decode a
/// few common entities, collapse whitespace. Good enough to feed a model.
fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let bytes = html.as_bytes();
    let mut i = 0;
    let lower = html.to_ascii_lowercase();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            // Skip <script>...</script> and <style>...</style> wholesale.
            for tag in ["script", "style"] {
                let open = format!("<{tag}");
                if lower[i..].starts_with(&open) {
                    let close = format!("</{tag}>");
                    if let Some(end) = lower[i..].find(&close) {
                        i += end + close.len();
                        continue;
                    }
                }
            }
            // Otherwise skip to the end of this tag.
            match html[i..].find('>') {
                Some(end) => i += end + 1,
                None => break,
            }
        } else {
            out.push(html[i..].chars().next().unwrap_or(' '));
            i += html[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        }
    }
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    // Collapse runs of whitespace/blank lines.
    let mut collapsed = String::with_capacity(decoded.len());
    let mut last_blank = false;
    for line in decoded.lines() {
        let t = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if t.is_empty() {
            if !last_blank {
                collapsed.push('\n');
            }
            last_blank = true;
        } else {
            collapsed.push_str(&t);
            collapsed.push('\n');
            last_blank = false;
        }
    }
    collapsed
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}… [truncated]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_removes_tags_and_scripts() {
        let html = "<html><head><style>p{color:red}</style></head><body><p>Hello <b>world</b></p><script>alert(1)</script></body></html>";
        let text = strip_html(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("color:red"));
    }

    #[test]
    fn truncate_marks_cut() {
        assert_eq!(truncate("abc", 10), "abc");
        assert!(truncate("abcdefghijk", 5).starts_with("abcde"));
        assert!(truncate("abcdefghijk", 5).contains("truncated"));
    }

    #[test]
    fn key_env_var_maps_provider() {
        assert_eq!(key_env_var("brave"), "BRAVE_API_KEY");
        assert_eq!(key_env_var("tavily"), "TAVILY_API_KEY");
        assert_eq!(key_env_var("anything"), "TAVILY_API_KEY");
    }
}
