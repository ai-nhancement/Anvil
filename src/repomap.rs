//! Lightweight repo map (ROADMAP_codex.md #4, adapted from Aider).
//!
//! Builds a ranked, budgeted table of top-level symbols across the project and
//! injects it into the coder's context each turn, so it knows what exists
//! without reading every file (cutting the list_dir → read_file → grep flailing
//! and the context bloat that comes with it).
//!
//! Deliberately lightweight vs. Aider: regex-style signature extraction + simple
//! ranking (task-mentioned, then most-recently-modified) + a char budget — no
//! tree-sitter parsers and no PageRank. Cross-language by keyword heuristics.
//! Rebuilt once per turn (not per request), bounded by the caps below.

use std::path::Path;
use std::time::SystemTime;

use crate::tools::{SKIP_DIRS, SKIP_EXTS};

const MAX_FILES: usize = 500;
const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_SIGS_PER_FILE: usize = 30;
const SIG_MAX_LEN: usize = 120;

struct FileEntry {
    rel: String,
    mtime: SystemTime,
    sigs: Vec<String>,
}

/// Build the repo map within `char_budget`, or None if there's nothing to show.
/// `task` (the current objective) biases ranking toward mentioned files.
pub fn build(root: &Path, task: Option<&str>, char_budget: usize) -> Option<String> {
    let mut files: Vec<FileEntry> = Vec::new();
    collect(root, root, &mut files);
    files.retain(|f| !f.sigs.is_empty());
    if files.is_empty() {
        return None;
    }

    // Rank: files named in the task first, then most-recently-modified, then path.
    let task_lc = task.map(|t| t.to_lowercase()).unwrap_or_default();
    files.sort_by(|a, b| {
        mentioned(&b.rel, &task_lc)
            .cmp(&mentioned(&a.rel, &task_lc))
            .then(b.mtime.cmp(&a.mtime))
            .then(a.rel.cmp(&b.rel))
    });

    let total = files.len();
    let mut out = String::from(
        "--- REPO MAP (top-level symbols — a guide, not the full code; use read_file for bodies) ---\n",
    );
    let mut used = 0usize;
    for (i, f) in files.iter().enumerate() {
        let mut block = format!("{}:\n", f.rel);
        for s in f.sigs.iter().take(MAX_SIGS_PER_FILE) {
            block.push_str("  ");
            block.push_str(s);
            block.push('\n');
        }
        // Always include at least one file; stop once the budget is reached.
        if i > 0 && used + block.len() > char_budget {
            out.push_str(&format!(
                "… (+{} more files — use grep/read_file for the rest)\n",
                total - i
            ));
            break;
        }
        out.push_str(&block);
        used += block.len();
    }
    out.push_str("--- END REPO MAP ---");
    Some(out)
}

/// 1 when the file's path or stem appears in the task text, else 0.
fn mentioned(rel: &str, task_lc: &str) -> u8 {
    if task_lc.is_empty() {
        return 0;
    }
    let lc = rel.to_lowercase();
    let stem = Path::new(&lc)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if task_lc.contains(&lc) || (stem.len() >= 3 && task_lc.contains(&stem)) {
        1
    } else {
        0
    }
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<FileEntry>) {
    if out.len() >= MAX_FILES {
        return;
    }
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut paths: Vec<_> = read.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if out.len() >= MAX_FILES {
            return;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if path.is_dir() {
            if name.starts_with('.') || SKIP_DIRS.iter().any(|d| d.eq_ignore_ascii_case(&name)) {
                continue;
            }
            collect(root, &path, out);
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if SKIP_EXTS.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
                continue;
            }
            let meta = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.len() > MAX_FILE_BYTES {
                continue;
            }
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue, // not UTF-8 text
            };
            let sigs = extract_signatures(&content);
            if sigs.is_empty() {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(FileEntry {
                rel,
                mtime: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                sigs,
            });
        }
    }
}

/// Pull top-level definition lines (fn/struct/class/def/func/…) from a file.
fn extract_signatures(content: &str) -> Vec<String> {
    let mut sigs = Vec::new();
    for raw in content.lines() {
        let line = raw.trim_start();
        if line.is_empty()
            || line.starts_with("//")
            || line.starts_with('#')
            || line.starts_with('*')
            || line.starts_with("/*")
        {
            continue;
        }
        if is_definition(line) {
            let sig = line.split('{').next().unwrap_or(line).trim_end();
            let sig = sig.trim_end_matches(';').trim_end();
            let sig: String = sig.chars().take(SIG_MAX_LEN).collect();
            if !sig.is_empty() {
                sigs.push(sig);
            }
        }
        if sigs.len() >= MAX_SIGS_PER_FILE {
            break;
        }
    }
    sigs
}

/// Heuristic: does this (already left-trimmed) line declare a function/type?
fn is_definition(line: &str) -> bool {
    const KW: &[&str] = &[
        "fn ",
        "struct ",
        "enum ",
        "trait ",
        "impl ",
        "type ",
        "mod ",
        "macro_rules!",
        "def ",
        "class ",
        "func ",
        "function ",
        "interface ",
    ];
    // Peel common leading modifiers so `pub async fn`, `export class`, etc. match.
    let mut l = line;
    loop {
        let mut changed = false;
        for pre in [
            "pub(crate) ",
            "pub(super) ",
            "pub ",
            "async ",
            "export ",
            "default ",
            "unsafe ",
            "extern ",
            "static ",
            "const ",
            "final ",
            "abstract ",
        ] {
            if let Some(rest) = l.strip_prefix(pre) {
                l = rest.trim_start();
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    KW.iter().any(|k| l.starts_with(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_signatures_across_modifiers() {
        let content = "// a comment\n\
pub fn foo(x: u32) -> u32 {\n    x\n}\n\
struct Bar {\n    a: u32,\n}\n\
fn helper() {}\n\
const N: usize = 5;\n\
impl Bar {}\n";
        let sigs = extract_signatures(content);
        assert!(
            sigs.iter().any(|s| s == "pub fn foo(x: u32) -> u32"),
            "{:?}",
            sigs
        );
        assert!(sigs.iter().any(|s| s == "struct Bar"), "{:?}", sigs);
        assert!(sigs.iter().any(|s| s == "fn helper()"), "{:?}", sigs);
        assert!(sigs.iter().any(|s| s == "impl Bar"), "{:?}", sigs);
        // A plain const is not a definition we map.
        assert!(!sigs.iter().any(|s| s.contains("const N")), "{:?}", sigs);
    }

    #[test]
    fn build_maps_code_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}\nstruct App {}\n").unwrap();
        std::fs::write(root.join("notes.md"), "# heading\njust prose, no code\n").unwrap();
        let map = build(root, None, 4000).unwrap();
        assert!(map.contains("src/main.rs:"), "{}", map);
        assert!(map.contains("fn main()"), "{}", map);
        assert!(map.contains("struct App"), "{}", map);
        // No definitions → the markdown file is omitted.
        assert!(!map.contains("notes.md"), "{}", map);
    }
}
