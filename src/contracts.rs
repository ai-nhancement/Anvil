//! Embedded, bench-validated coder contracts (the local-model tiers) + tier
//! resolution for the live coder.
//!
//! A local model drives Anvil best under a contract matched to its capability:
//! the FULL contract for a weak (~2B) model, the MINIMAL contract for a capable
//! (>=4B) one. These two tiers are compiled into the binary so the shipped coder
//! always has them — the bench reads them from disk, the live agent uses these.
//! Which tier a model gets is set explicitly per binding (`contract = "..."`),
//! validated on the bench first (see `contracts/MODEL_FINDINGS.md`).

use std::path::Path;

/// The ~2B tier: role + clauses (ACT/VERIFY/TRUTH/PERSISTENCE). A weak model needs
/// every clause to act and verify reliably.
const FULL: &str = include_str!("../contracts/coder_local_base.md");

/// The >=4B tier: role + the edit/write tool line, zero clauses. A capable model is
/// hurt by extra scaffolding, so this is deliberately minimal.
const MINIMAL: &str = include_str!("../contracts/coder_local_base_v4.md");

/// Resolve a tier alias to embedded contract text. Recognized: "full" (~2B) and
/// "minimal" (>=4B), plus the file stems as aliases. None for an unknown name.
pub fn embedded(name: &str) -> Option<&'static str> {
    match name.trim() {
        "full" | "coder_local_base" | "coder_local_base.md" => Some(FULL),
        "minimal" | "v4" | "coder_local_base_v4" | "coder_local_base_v4.md" => Some(MINIMAL),
        _ => None,
    }
}

/// The contract text for a binding's `contract` setting: an embedded tier by alias,
/// or a contract file resolved relative to the project root. None if it is neither a
/// known alias nor a readable file (the caller then falls back to the built-in prompt).
pub fn resolve(name: &str, root: &Path) -> Option<String> {
    if let Some(text) = embedded(name) {
        return Some(text.trim().to_string());
    }
    let path = {
        let p = Path::new(name);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            root.join(p)
        }
    };
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_resolve_to_distinct_nonempty_tiers() {
        let full = embedded("full").expect("full tier");
        let minimal = embedded("minimal").expect("minimal tier");
        assert!(!full.trim().is_empty());
        assert!(!minimal.trim().is_empty());
        // The full tier carries clauses the minimal one drops, so it is the larger.
        assert!(full.len() > minimal.len());
        assert!(embedded("nope").is_none());
    }

    #[test]
    fn stem_aliases_match() {
        assert_eq!(embedded("v4"), embedded("minimal"));
        assert_eq!(embedded("coder_local_base"), embedded("full"));
    }
}
