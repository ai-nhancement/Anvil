//! models.dev metadata (ROADMAP_codex.md #5).
//!
//! Per-model context window, pricing, and tool-call capability, sourced from the
//! community DB at <https://models.dev/api.json>. Cached globally (OS cache dir)
//! and refreshed in the background. Used today to warn when a model lacks
//! tool-calling (the coder needs it) and to show model facts in `/models`; the
//! per-model context window also unblocks token-based budgeting (#6).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::Deserialize;

const API_URL: &str = "https://models.dev/api.json";
const TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub context: Option<u64>,
    pub output: Option<u64>,
    pub tool_call: Option<bool>,
    pub cost_input: Option<f64>,  // USD per 1M input tokens
    pub cost_output: Option<f64>, // USD per 1M output tokens
}

// ── raw deserialization of the api.json shape ────────────────────────────────
#[derive(Deserialize)]
struct RawProvider {
    #[serde(default)]
    models: HashMap<String, RawModel>,
}
#[derive(Deserialize)]
struct RawModel {
    name: Option<String>,
    tool_call: Option<bool>,
    limit: Option<RawLimit>,
    cost: Option<RawCost>,
}
#[derive(Deserialize)]
struct RawLimit {
    context: Option<u64>,
    output: Option<u64>,
}
#[derive(Deserialize)]
struct RawCost {
    input: Option<f64>,
    output: Option<f64>,
}

/// Flat lookup of model id → info, merged across all providers (model ids are
/// distinctive enough that collisions carry equivalent metadata).
pub struct ModelsDb {
    by_model: HashMap<String, ModelInfo>,
}

impl ModelsDb {
    /// Look up a model by the exact id a provider expects (e.g. "claude-opus-4-5",
    /// "qwen2.5-coder:32b"), case-insensitively; also tries stripping a leading
    /// "provider/" prefix.
    pub fn lookup(&self, model: &str) -> Option<&ModelInfo> {
        let key = model.trim().to_lowercase();
        if let Some(info) = self.by_model.get(&key) {
            return Some(info);
        }
        key.rsplit('/').next().and_then(|m| self.by_model.get(m))
    }
}

fn cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("anvil").join("models-dev.json"))
}

fn parse(raw: &str) -> Option<ModelsDb> {
    let providers: HashMap<String, RawProvider> = serde_json::from_str(raw).ok()?;
    let mut by_model: HashMap<String, ModelInfo> = HashMap::new();
    for prov in providers.into_values() {
        for (mid, m) in prov.models {
            let info = ModelInfo {
                name: m.name.unwrap_or_else(|| mid.clone()),
                context: m.limit.as_ref().and_then(|l| l.context),
                output: m.limit.as_ref().and_then(|l| l.output),
                tool_call: m.tool_call,
                cost_input: m.cost.as_ref().and_then(|c| c.input),
                cost_output: m.cost.as_ref().and_then(|c| c.output),
            };
            by_model.insert(mid.to_lowercase(), info);
        }
    }
    if by_model.is_empty() {
        return None;
    }
    Some(ModelsDb { by_model })
}

/// Load the cached DB (sync). None until the first successful background refresh.
pub fn load() -> Option<ModelsDb> {
    let raw = std::fs::read_to_string(cache_path()?).ok()?;
    parse(&raw)
}

fn cache_is_fresh() -> bool {
    let Some(p) = cache_path() else {
        return false;
    };
    std::fs::metadata(&p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map(|age| age < TTL)
        .unwrap_or(false)
}

/// Async, best-effort: fetch models.dev and write the cache, unless the cache is
/// still fresh. Any failure is silent (this is supplementary metadata).
pub async fn refresh_if_stale() {
    if cache_is_fresh() {
        return;
    }
    let Some(p) = cache_path() else {
        return;
    };
    let client = match reqwest::Client::builder().user_agent("anvil").build() {
        Ok(c) => c,
        Err(_) => return,
    };
    let body = match client.get(API_URL).send().await {
        Ok(r) if r.status().is_success() => match r.text().await {
            Ok(b) => b,
            Err(_) => return,
        },
        _ => return,
    };
    // Only cache something that actually parses.
    if parse(&body).is_none() {
        return;
    }
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&p, body);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_looks_up() {
        let raw = r#"{
            "anthropic": { "models": {
                "claude-opus-4-5": { "name": "Claude Opus 4.5", "tool_call": true,
                    "limit": { "context": 200000, "output": 64000 },
                    "cost": { "input": 5, "output": 25 } }
            }},
            "ollama": { "models": {
                "qwen2.5-coder:32b": { "tool_call": false, "limit": { "context": 32768 } }
            }}
        }"#;
        let db = parse(raw).unwrap();
        let a = db.lookup("claude-opus-4-5").unwrap();
        assert_eq!(a.context, Some(200000));
        assert_eq!(a.tool_call, Some(true));
        assert_eq!(a.cost_input, Some(5.0));
        // case-insensitive + provider/ prefix
        assert!(db.lookup("anthropic/Claude-Opus-4-5").is_some());
        let q = db.lookup("qwen2.5-coder:32b").unwrap();
        assert_eq!(q.tool_call, Some(false));
        assert_eq!(q.context, Some(32768));
        assert!(db.lookup("nope").is_none());
    }
}
