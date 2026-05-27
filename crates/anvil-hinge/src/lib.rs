//! Hinge-test framework for Anvil (P10b).
//!
//! Scans Rust (`crates/`) and Go (`sidecar/`) source files for `// hinge_test:`
//! annotations, builds a unified registry, and checks cross-language consensus.
//! Provides the data layer for `anvil hinge list` and `anvil hinge flip`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anvil_core::error::AnvilError;
use serde::{Deserialize, Serialize};

// ── Public types ──────────────────────────────────────────────────────────────

/// A hinge-test entry extracted from a source-file annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HingeEntry {
    /// Canonical ID used as the join key and as `HingeFlip.hinge_test_name`.
    pub intended: String,
    /// Current pinned value (e.g., `"1.80"`, `"anvil"`, `"source-scanner"`).
    pub pins: String,
    /// Phase this hinge was introduced in (e.g., `"P0"`, `"P10b"`).
    pub phase: String,
    /// Language the annotation was found in.
    pub source: HingeSource,
    /// Source file containing the annotation.
    pub file: PathBuf,
    /// Name of the test function immediately following the annotation.
    pub fn_name: String,
}

/// Language a hinge annotation was found in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HingeSource {
    Rust,
    Go,
}

/// An alternative-mechanism entry for non-test-harness deferred decisions.
///
/// Loaded from `.anvil/hinge-alternatives.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternativeEntry {
    /// Canonical ID (same namespace as `HingeEntry.intended`).
    pub intended: String,
    /// Current pinned value.
    pub pins: String,
    /// Phase this entry was introduced in.
    pub phase: String,
    /// Description of the non-test mechanism used to track this decision.
    pub mechanism: String,
}

/// Unified hinge registry combining test-harness and alternative-mechanism entries.
#[derive(Debug, Default, Clone)]
pub struct HingeRegistry {
    /// All hinge-test entries found by scanning source files.
    pub entries: Vec<HingeEntry>,
    /// Alternative-mechanism entries from `.anvil/hinge-alternatives.toml`.
    pub alternatives: Vec<AlternativeEntry>,
}

/// A cross-language hinge consensus violation.
///
/// Occurs when the same `intended` ID appears in both Rust and Go entries but
/// with a differing `phase` value, indicating the entries are tracking different
/// phases of the same concept.
#[derive(Debug, Clone)]
pub struct ConsensusViolation {
    /// The `intended` ID with inconsistent metadata across languages.
    pub intended: String,
    /// Human-readable description of the mismatch.
    pub reason: String,
}

// ── Registry impl ─────────────────────────────────────────────────────────────

impl HingeRegistry {
    /// Returns cross-language consensus violations in this registry.
    ///
    /// A violation is raised when the same `intended` appears in both a Rust and a
    /// Go entry but with different `phase` values. Cross-language hinges may have
    /// different `pins` values (the language-specific representation of the same
    /// invariant), but they must belong to the same phase.
    #[must_use]
    pub fn consensus_violations(&self) -> Vec<ConsensusViolation> {
        let mut rust_map: HashMap<&str, &HingeEntry> = HashMap::new();
        let mut go_map: HashMap<&str, &HingeEntry> = HashMap::new();
        for entry in &self.entries {
            match entry.source {
                HingeSource::Rust => {
                    rust_map.insert(entry.intended.as_str(), entry);
                }
                HingeSource::Go => {
                    go_map.insert(entry.intended.as_str(), entry);
                }
            }
        }

        let mut violations = Vec::new();
        for (intended, rust_entry) in &rust_map {
            if let Some(go_entry) = go_map.get(intended) {
                if rust_entry.phase != go_entry.phase {
                    violations.push(ConsensusViolation {
                        intended: (*intended).to_owned(),
                        reason: format!(
                            "phase mismatch: Rust={}, Go={}",
                            rust_entry.phase, go_entry.phase
                        ),
                    });
                }
            }
        }
        violations
    }
}

// ── Comment parser ────────────────────────────────────────────────────────────

/// Parses a `// hinge_test: pins=X, intended=Y, phase=Z` annotation line.
///
/// Returns `(pins, intended, phase)` if the line is a valid hinge annotation
/// with all three required fields present and non-empty. Returns `None` for any
/// other line or if a required field is absent.
#[must_use]
pub fn parse_hinge_comment(line: &str) -> Option<(String, String, String)> {
    let trimmed = line.trim();
    let after_prefix = trimmed.strip_prefix("// hinge_test:")?;
    let rest = after_prefix.trim();
    let mut pins: Option<String> = None;
    let mut intended: Option<String> = None;
    let mut phase: Option<String> = None;
    for segment in rest.split(',') {
        let segment = segment.trim();
        if let Some(v) = segment.strip_prefix("pins=") {
            pins = Some(v.trim().to_owned());
        } else if let Some(v) = segment.strip_prefix("intended=") {
            intended = Some(v.trim().to_owned());
        } else if let Some(v) = segment.strip_prefix("phase=") {
            phase = Some(v.trim().to_owned());
        }
    }
    let pins = pins.filter(|s| !s.is_empty())?;
    let intended = intended.filter(|s| !s.is_empty())?;
    let phase = phase.filter(|s| !s.is_empty())?;
    Some((pins, intended, phase))
}

// ── File scanners ─────────────────────────────────────────────────────────────

fn scan_rust_file(path: &Path, entries: &mut Vec<HingeEntry>) -> Result<(), AnvilError> {
    let content = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if let Some((pins, intended, phase)) = parse_hinge_comment(lines[i]) {
            let end = (i + 6).min(lines.len());
            let mut saw_test = false;
            let mut fn_name: Option<String> = None;
            for line in &lines[i + 1..end] {
                let trimmed = line.trim();
                if trimmed == "#[test]" {
                    saw_test = true;
                } else if saw_test {
                    if let Some(rest) = trimmed.strip_prefix("fn ") {
                        if let Some(name_str) = rest.split('(').next() {
                            let name_str = name_str.trim();
                            if !name_str.is_empty() {
                                fn_name = Some(name_str.to_owned());
                            }
                        }
                        break;
                    }
                }
            }
            if let Some(name) = fn_name {
                entries.push(HingeEntry {
                    intended,
                    pins,
                    phase,
                    source: HingeSource::Rust,
                    file: path.to_path_buf(),
                    fn_name: name,
                });
            }
        }
        i += 1;
    }
    Ok(())
}

fn scan_go_file(path: &Path, entries: &mut Vec<HingeEntry>) -> Result<(), AnvilError> {
    let content = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if let Some((pins, intended, phase)) = parse_hinge_comment(lines[i]) {
            let end = (i + 4).min(lines.len());
            let mut fn_name: Option<String> = None;
            for line in &lines[i + 1..end] {
                let trimmed = line.trim();
                if trimmed.starts_with("func Test") {
                    if let Some(rest) = trimmed.strip_prefix("func ") {
                        if let Some(name_str) = rest.split('(').next() {
                            let name_str = name_str.trim();
                            if !name_str.is_empty() {
                                fn_name = Some(name_str.to_owned());
                            }
                        }
                    }
                    break;
                }
            }
            if let Some(name) = fn_name {
                entries.push(HingeEntry {
                    intended,
                    pins,
                    phase,
                    source: HingeSource::Go,
                    file: path.to_path_buf(),
                    fn_name: name,
                });
            }
        }
        i += 1;
    }
    Ok(())
}

fn walk_files_with_ext(dir: &Path, ext: &str, files: &mut Vec<PathBuf>) -> Result<(), AnvilError> {
    for item in std::fs::read_dir(dir)? {
        let item = item?;
        let path = item.path();
        let file_name = item.file_name();
        let name = file_name.to_string_lossy();
        if path.is_dir() {
            if !matches!(name.as_ref(), ".git" | "target" | "vendor" | "node_modules") {
                walk_files_with_ext(&path, ext, files)?;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some(ext) {
            files.push(path);
        }
    }
    Ok(())
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Scans the workspace for hinge-test annotations in Rust and Go source files.
///
/// Rust sources are read from `<root>/crates/`; Go sources from `<root>/sidecar/`.
/// Directories named `.git`, `target`, `vendor`, or `node_modules` are skipped.
///
/// # Errors
///
/// Returns an error if any source file cannot be read or if the alternatives
/// TOML is malformed.
pub fn scan_workspace(root: &Path) -> Result<HingeRegistry, AnvilError> {
    let mut entries = Vec::new();

    let crates_dir = root.join("crates");
    if crates_dir.exists() {
        let mut rs_files = Vec::new();
        walk_files_with_ext(&crates_dir, "rs", &mut rs_files)?;
        for file in &rs_files {
            scan_rust_file(file, &mut entries)?;
        }
    }

    let sidecar_dir = root.join("sidecar");
    if sidecar_dir.exists() {
        let mut go_files = Vec::new();
        walk_files_with_ext(&sidecar_dir, "go", &mut go_files)?;
        for file in &go_files {
            scan_go_file(file, &mut entries)?;
        }
    }

    let alternatives = load_alternatives(root)?;
    Ok(HingeRegistry {
        entries,
        alternatives,
    })
}

#[derive(Deserialize)]
struct AlternativesFile {
    #[serde(default)]
    alternative: Vec<AlternativeEntry>,
}

/// Loads alternative-mechanism entries from `.anvil/hinge-alternatives.toml`.
///
/// Returns an empty vec if the file does not exist.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be parsed as valid TOML.
pub fn load_alternatives(root: &Path) -> Result<Vec<AlternativeEntry>, AnvilError> {
    let path = root.join(".anvil/hinge-alternatives.toml");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path)?;
    let parsed: AlternativesFile = toml::from_str(&raw).map_err(|e| AnvilError::ConfigParse {
        path: path.clone(),
        source: Box::new(e),
    })?;
    Ok(parsed.alternative)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // hinge_test: pins=source-scanner, intended=test_hinge_decorator_metadata_required, phase=P10b
    #[test]
    fn test_hinge_decorator_metadata_required() {
        // Pins: Rust hinge entries are found by scanning // hinge_test: comments
        // immediately before a #[test] fn declaration.
        // Flipping requires changing the annotation format and updating the scanner.
        let entry = HingeEntry {
            intended: "my-hinge".to_owned(),
            pins: "v1".to_owned(),
            phase: "P5".to_owned(),
            source: HingeSource::Rust,
            file: PathBuf::from("test.rs"),
            fn_name: "test_foo".to_owned(),
        };
        assert!(!entry.intended.is_empty(), "intended must be non-empty");
        assert!(!entry.pins.is_empty(), "pins must be non-empty");
        assert!(!entry.phase.is_empty(), "phase must be non-empty");

        // Complete annotation parses
        assert!(
            parse_hinge_comment("// hinge_test: pins=v1, intended=my-hinge, phase=P5").is_some(),
            "complete annotation must parse"
        );
        // Missing any field returns None
        assert!(
            parse_hinge_comment("// hinge_test: pins=v1, intended=my-hinge").is_none(),
            "annotation missing phase must be rejected"
        );
        assert!(
            parse_hinge_comment("// hinge_test: pins=v1, phase=P5").is_none(),
            "annotation missing intended must be rejected"
        );
    }

    // hinge_test: pins=registry-merge, intended=test_bi_language_registry_merge, phase=P10b
    #[test]
    fn test_bi_language_registry_merge() {
        // Pins: consensus check detects phase mismatches between Rust and Go entries
        // for the same intended. Cross-language pin differences are permitted (each
        // language may express the invariant differently), but phase must agree.
        // Flipping requires changing the consensus algorithm.
        let mut registry = HingeRegistry {
            entries: vec![
                HingeEntry {
                    intended: "cross-hinge".to_owned(),
                    pins: "v1".to_owned(),
                    phase: "P5".to_owned(),
                    source: HingeSource::Rust,
                    file: PathBuf::from("test.rs"),
                    fn_name: "test_cross".to_owned(),
                },
                HingeEntry {
                    intended: "cross-hinge".to_owned(),
                    pins: "go-v1".to_owned(), // different pins — permitted
                    phase: "P6".to_owned(),   // different phase — violation
                    source: HingeSource::Go,
                    file: PathBuf::from("cross_test.go"),
                    fn_name: "TestCross".to_owned(),
                },
            ],
            alternatives: Vec::new(),
        };

        let violations = registry.consensus_violations();
        assert_eq!(
            violations.len(),
            1,
            "phase mismatch between Rust and Go must produce one violation"
        );
        assert_eq!(violations[0].intended, "cross-hinge");

        // Fix the phase mismatch
        registry.entries[1].phase = "P5".to_owned();
        assert!(
            registry.consensus_violations().is_empty(),
            "matching phases must produce no violations"
        );
    }
}
