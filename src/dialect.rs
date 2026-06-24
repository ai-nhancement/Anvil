//! Per-model tool *dialects* — hand each model the tool surface it works best
//! with, while the agent loop, `tools::execute`, conversation history, and the
//! ledger only ever see Anvil's single *canonical* tool set.
//!
//! Translation happens at the LLM gateway (`llm::chat_turn_stream`), NOT inside
//! the agent loop: the loop's confirmation gate, read-only dedup, and loop-breaker
//! all key on the raw call name, so a dialect call must be normalized to canonical
//! *before* it reaches them. Keeping the ledger canonical also makes a session
//! portable across vendors mid-stream (Anvil's cross-vendor review thesis).
//!
//! Wired so far: `Codex` (pass-through, Anvil's historical surface) and `Generic`
//! (the agnostic floor — drops `apply_patch`, promotes `edit_file`). `Anthropic`
//! is selected for Claude models but still behaves like `Codex` until its native
//! arm lands in Phase 3.
//! See `docs/ROADMAP_model_dialects.md` and `docs/PLAN_dialects_build.md`.

use crate::llm::{ToolCall, ToolDef};

/// Which tool dialect a model is driven with.
#[derive(Debug, Clone, Copy)]
pub enum Dialect {
    /// OpenAI/Codex-native: the `apply_patch` envelope + Anvil's tool set as-is.
    /// Anvil's historical surface and the Phase 0 default for everyone.
    Codex,
    /// Anthropic-native: the built-in `str_replace_based_edit_tool` + `bash`
    /// (Phase 3).
    Anthropic,
    /// Lowest-common-denominator typed function calls — the agnostic floor for
    /// any function-calling model (Phase 1).
    Generic,
}

impl Dialect {
    /// OUTBOUND: render the canonical tool set into this dialect's advertised set.
    pub fn advertise(&self, tools: &[ToolDef]) -> Vec<ToolDef> {
        match self {
            // Codex keeps Anvil's full canonical set (incl. apply_patch). Anthropic
            // is still identity until its native arm lands in Phase 3.
            Dialect::Codex | Dialect::Anthropic => tools.to_vec(),
            // Generic = the local-model FLOOR (see contracts/tool_surface_local.md):
            // the 7 core tools only (apply_patch dropped — Codex DSL; delegate /
            // flag_risk dropped — too much surface for a small model), with the fat
            // schema descriptions slimmed to one line each. The contract + system map
            // own the *discipline*; the schema owns only the signature, said once.
            Dialect::Generic => tools
                .iter()
                .filter(|t| GENERIC_TOOLS.contains(&t.name.as_str()))
                .map(|t| {
                    let mut t = t.clone();
                    if let Some(slim) = generic_slim_desc(&t.name) {
                        t.description = slim.to_string();
                    }
                    t
                })
                .collect(),
        }
    }

    /// INBOUND: map a model-emitted tool call back to a canonical `ToolCall` that
    /// `tools::execute` understands. Identity today; the Anthropic native arm will
    /// dispatch on the editor's `command` arg in Phase 3.
    pub fn to_canonical(self, call: ToolCall) -> ToolCall {
        match self {
            Dialect::Codex | Dialect::Generic | Dialect::Anthropic => call,
        }
    }

    /// Family-specific system-prompt addendum, spliced into the system prompt at
    /// the gateway (`chat_turn_stream`). Empty for Codex/Anthropic; Generic tells
    /// patch-trained models that this surface uses plain typed edits.
    pub fn prompt_addendum(&self) -> &'static str {
        match self {
            Dialect::Codex | Dialect::Anthropic => "",
            Dialect::Generic => GENERIC_ADDENDUM,
        }
    }

    /// Parse an explicit `dialect = "..."` override. `None` for an unrecognized
    /// value — the caller falls back to inference (and may warn the user).
    pub fn parse_override(s: &str) -> Option<Dialect> {
        match s.trim().to_ascii_lowercase().as_str() {
            "codex" => Some(Dialect::Codex),
            "anthropic" => Some(Dialect::Anthropic),
            "generic" => Some(Dialect::Generic),
            _ => None,
        }
    }

    /// Resolve the dialect for a coder binding: an explicit `dialect = "..."`
    /// override wins; an unrecognized value falls through to model-family
    /// inference; the inference floor is `Generic`. (Used by the production agent
    /// path; the benchmark sweeps dialects explicitly via `parse_override`.)
    #[allow(dead_code)]
    pub fn resolve(explicit: Option<&str>, model: &str) -> Dialect {
        if let Some(d) = explicit.and_then(Self::parse_override) {
            return d;
        }
        Self::infer_from_model(model)
    }

    /// Infer a dialect from the model id. GPT/Codex-family → `Codex`; Claude →
    /// `Anthropic`; everything else (Grok, Gemini, local open-weights, …) → the
    /// `Generic` agnostic floor. The o-series check matches the **bare** model id
    /// (the last `/`-segment) so gateway/router prefixes like `azure/o1-preview`
    /// or `openrouter/openai/o3-mini` still route to Codex — while a local model
    /// that merely ends in `-o1` (e.g. `marco-o1`) is not mistaken for one.
    #[allow(dead_code)]
    fn infer_from_model(model: &str) -> Dialect {
        let m = model.to_ascii_lowercase();
        let bare = m.rsplit('/').next().unwrap_or(m.as_str());
        if m.contains("claude") {
            Dialect::Anthropic
        } else if m.contains("gpt")
            || m.contains("codex")
            || bare.starts_with("o1")
            || bare.starts_with("o3")
            || bare.starts_with("o4")
        {
            Dialect::Codex
        } else {
            Dialect::Generic
        }
    }
}

/// The Generic (local-model) tool floor — the minimal core a small model drives
/// well. Mirrors contracts/tool_surface_local.md.
const GENERIC_TOOLS: &[&str] = &[
    "read_file",
    "list_dir",
    "grep",
    "edit_file",
    "write_file",
    "run_command",
    "project_state",
];

/// One-line schema descriptions for the Generic floor — the *signature* only; the
/// contract/system map carry the usage discipline, so nothing is said twice.
fn generic_slim_desc(name: &str) -> Option<&'static str> {
    Some(match name {
        "read_file" => "Read a text file, or a line range with offset+limit.",
        "list_dir" => "List a directory's entries.",
        "grep" => "Find a literal substring; returns path:line: text.",
        "edit_file" => "Replace an exact, unique snippet.",
        "write_file" => "Create or overwrite a file.",
        "run_command" => "Run a shell command from the project root; returns output + exit code.",
        "project_state" => "Live workflow stage, phase, plan slice, and git status.",
        _ => return None,
    })
}

/// Generic-dialect system-prompt addendum — steers models trained on patch/diff
/// DSLs toward this dialect's plain typed edit tools. (Used only when the Generic
/// arm runs WITHOUT the full operational contract, which already covers this.)
const GENERIC_ADDENDUM: &str = "Editing tools: use `edit_file` for targeted changes (an exact, unique snippet → its replacement) and `write_file` to create or fully overwrite a file. There is no patch or diff tool in this environment — do not emit `*** Begin Patch` envelopes or unified diffs; call `edit_file` or `write_file` instead.";

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn td(name: &str) -> ToolDef {
        ToolDef {
            name: name.into(),
            description: String::new(),
            input_schema: json!({}),
        }
    }

    #[test]
    fn codex_advertise_is_identity() {
        let tools = vec![td("read_file"), td("apply_patch"), td("run_command")];
        let names: Vec<String> = Dialect::Codex
            .advertise(&tools)
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, vec!["read_file", "apply_patch", "run_command"]);
    }

    #[test]
    fn codex_to_canonical_is_identity() {
        let call = ToolCall {
            id: "t1".into(),
            name: "run_command".into(),
            arguments: json!({"command": "cargo build"}),
        };
        let out = Dialect::Codex.to_canonical(call);
        // Name preserved → the confirmation gate (keyed on "run_command") still fires.
        assert_eq!(out.name, "run_command");
        assert_eq!(out.arguments, json!({"command": "cargo build"}));
    }

    #[test]
    fn generic_advertise_is_the_slim_floor() {
        let tools = vec![
            td("read_file"),
            td("edit_file"),
            td("apply_patch"),
            td("write_file"),
            td("run_command"),
            td("project_state"),
            td("delegate"),
            td("flag_risk"),
        ];
        let out = Dialect::Generic.advertise(&tools);
        let names: Vec<&str> = out.iter().map(|t| t.name.as_str()).collect();
        // The floor: the 7 core tools survive; apply_patch / delegate / flag_risk drop.
        for dropped in ["apply_patch", "delegate", "flag_risk"] {
            assert!(!names.contains(&dropped), "{dropped} must be dropped");
        }
        for keep in [
            "read_file",
            "edit_file",
            "write_file",
            "run_command",
            "project_state",
        ] {
            assert!(names.contains(&keep), "missing {keep}");
        }
        // Descriptions are slimmed to the one-line signature.
        let ef = out.iter().find(|t| t.name == "edit_file").unwrap();
        assert_eq!(ef.description, "Replace an exact, unique snippet.");
    }

    #[test]
    fn resolve_explicit_override_wins_then_falls_through() {
        assert!(matches!(
            Dialect::resolve(Some("generic"), "gpt-4o"),
            Dialect::Generic
        ));
        assert!(matches!(
            Dialect::resolve(Some("Codex"), "claude-opus-4-8"),
            Dialect::Codex
        ));
        // Unrecognized override falls through to family inference.
        assert!(matches!(
            Dialect::resolve(Some("nonsense"), "claude-opus-4-8"),
            Dialect::Anthropic
        ));
    }

    #[test]
    fn infer_routes_by_model_family() {
        assert!(matches!(
            Dialect::resolve(None, "claude-opus-4-8"),
            Dialect::Anthropic
        ));
        assert!(matches!(
            Dialect::resolve(None, "gpt-5-codex"),
            Dialect::Codex
        ));
        assert!(matches!(Dialect::resolve(None, "o3-mini"), Dialect::Codex));
        // Gateway / router prefixes still route to Codex (matched on the bare id).
        assert!(matches!(
            Dialect::resolve(None, "azure/o1-preview"),
            Dialect::Codex
        ));
        assert!(matches!(
            Dialect::resolve(None, "openrouter/openai/o3-mini"),
            Dialect::Codex
        ));
        assert!(matches!(
            Dialect::resolve(None, "openrouter/openai/gpt-4o"),
            Dialect::Codex
        ));
        for generic in [
            "grok-code-fast",
            "gemini-2.5-flash",
            "qwen2.5-coder:32b",
            "llama3.1:70b",
            // A local reasoning model ending in "-o1" must NOT be read as OpenAI o-series.
            "marco-o1",
        ] {
            assert!(
                matches!(Dialect::resolve(None, generic), Dialect::Generic),
                "{generic}"
            );
        }
    }
}
