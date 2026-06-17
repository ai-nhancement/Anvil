//! Agent tools — the hands the coder was always missing.
//!
//! These give the coder the same kind of abilities a modern coding agent has:
//! read, write and edit files, list directories, search the tree, and run
//! commands — all scoped to the project root. The model requests them as tool
//! calls (see `llm.rs`); `agent.rs` runs the loop that executes them.
//!
//! Design notes:
//! - Every path is resolved against the project root and rejected if it escapes
//!   it (no `..` traversal, no absolute paths outside the tree).
//! - Tool errors are returned as plain strings (prefixed `ERROR:`) and fed back
//!   to the model so it can recover, rather than aborting the whole turn.
//! - `run_command` is flagged via `requires_confirmation` so the UI can gate it.

use std::path::{Component, Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Result};
use serde_json::json;

use crate::llm::{ToolCall, ToolDef};

/// Directories we never descend into for `list_dir` / `grep` (build output, VCS, vendored deps).
pub const SKIP_DIRS: &[&str] = &[
    "target", ".git", "node_modules", ".anvil", "dist", "build", "out", "bin", "obj",
    "__pycache__", ".next", ".cache", "debug", "release", "archive",
];

/// File extensions treated as binary/uninteresting for search + listing.
pub const SKIP_EXTS: &[&str] = &[
    "exe", "dll", "so", "dylib", "o", "a", "lib", "pdb", "rlib", "rmeta", "d",
    "png", "jpg", "jpeg", "gif", "ico", "svg", "webp", "bmp",
    "pdf", "zip", "tar", "gz", "bz2", "7z", "rar", "xz",
    "bin", "dat", "db", "sqlite", "lock", "log",
];

/// Cap on bytes returned from a single read / command so one tool call can't
/// blow the model's context window.
const MAX_READ_BYTES: usize = 200_000;
const MAX_CMD_OUTPUT: usize = 60_000;

/// The JSON-Schema tool definitions advertised to the model each turn.
pub fn tool_defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "read_file".into(),
            description: "Read a UTF-8 text file from the project. Returns the full contents (truncated if very large).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path relative to the project root, e.g. src/llm.rs"}
                },
                "required": ["path"]
            }),
        },
        ToolDef {
            name: "write_file".into(),
            description: "Create or overwrite a file with the given contents. Creates parent directories as needed.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path relative to the project root"},
                    "content": {"type": "string", "description": "Full file contents to write"}
                },
                "required": ["path", "content"]
            }),
        },
        ToolDef {
            name: "edit_file".into(),
            description: "Replace an exact, unique snippet in a file. `old_string` must appear exactly once; it is replaced with `new_string`. Use for targeted edits instead of rewriting whole files.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path relative to the project root"},
                    "old_string": {"type": "string", "description": "Exact text to find (must be unique in the file)"},
                    "new_string": {"type": "string", "description": "Replacement text"}
                },
                "required": ["path", "old_string", "new_string"]
            }),
        },
        ToolDef {
            name: "list_dir".into(),
            description: "List the entries of a directory (files and subdirectories), skipping build/VCS/vendored directories.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Directory path relative to the project root. Omit or use \".\" for the root."}
                }
            }),
        },
        ToolDef {
            name: "grep".into(),
            description: "Search the project tree for a literal substring. Returns matching lines as `path:line: text`. Optionally restrict to a subdirectory.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Literal substring to search for"},
                    "path": {"type": "string", "description": "Optional subdirectory to limit the search (relative to root)"}
                },
                "required": ["pattern"]
            }),
        },
        ToolDef {
            name: "run_command".into(),
            description: "Run a shell command from the project root (e.g. `cargo build`, `cargo test`, `git diff`). Returns combined stdout+stderr and the exit code. Requires user confirmation.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "The shell command line to execute"}
                },
                "required": ["command"]
            }),
        },
    ]
}

/// Whether a tool must be confirmed by the user before it runs.
pub fn requires_confirmation(name: &str) -> bool {
    name == "run_command"
}

/// Short human label for the tool call, shown in the transcript (`[tool] ...`).
pub fn summarize_args(call: &ToolCall) -> String {
    match call.name.as_str() {
        "read_file" | "write_file" | "edit_file" | "list_dir" => {
            arg_str(call, "path").unwrap_or_else(|_| ".".into())
        }
        "grep" => format!("\"{}\"", arg_str(call, "pattern").unwrap_or_default()),
        "run_command" => arg_str(call, "command").unwrap_or_default(),
        _ => String::new(),
    }
}

/// The command line for a `run_command` call (used by the confirmation prompt).
pub fn command_string(call: &ToolCall) -> String {
    arg_str(call, "command").unwrap_or_default()
}

/// A one-line summary of a tool result for the transcript, tailored per tool so
/// a directory listing reads as "12 entries" rather than just its first file.
/// (The model always receives the full result; this is display-only.)
pub fn result_summary(name: &str, result: &str) -> String {
    if let Some(rest) = result.strip_prefix("ERROR:") {
        return format!("error —{}", truncate_one_line(rest));
    }
    match name {
        "read_file" => format!("ok — {} lines, {} bytes", result.lines().count(), result.len()),
        "list_dir" => {
            if result.starts_with("(empty directory") {
                "ok — empty".to_string()
            } else {
                format!("ok — {} entries", result.lines().count())
            }
        }
        "grep" => {
            if result.starts_with("no matches") {
                "ok — no matches".to_string()
            } else {
                let n = result.lines().filter(|l| !l.starts_with("...")).count();
                format!("ok — {} matches", n)
            }
        }
        "run_command" => {
            // Result begins with "exit code: N".
            let code = result
                .lines()
                .next()
                .and_then(|l| l.strip_prefix("exit code:"))
                .map(|c| c.trim())
                .unwrap_or("?");
            format!("ok — exit {}", code)
        }
        // write_file / edit_file already return a descriptive one-liner.
        _ => format!("ok —{}", truncate_one_line(result)),
    }
}

fn truncate_one_line(s: &str) -> String {
    let first = s.lines().next().unwrap_or("").trim();
    if first.is_empty() {
        return String::new();
    }
    let prefixed = format!(" {}", first);
    if prefixed.chars().count() > 60 {
        let cut: String = prefixed.chars().take(60).collect();
        format!("{}…", cut)
    } else {
        prefixed
    }
}

/// Execute a tool call against `root`. Always returns a string to feed back to
/// the model — errors are returned as `ERROR: ...` rather than propagated.
pub fn execute(call: &ToolCall, root: &Path) -> String {
    match run(call, root) {
        Ok(s) => s,
        Err(e) => format!("ERROR: {}", e),
    }
}

fn run(call: &ToolCall, root: &Path) -> Result<String> {
    match call.name.as_str() {
        "read_file" => read_file(root, &arg_str(call, "path")?),
        "write_file" => write_file(root, &arg_str(call, "path")?, &arg_str(call, "content")?),
        "edit_file" => edit_file(
            root,
            &arg_str(call, "path")?,
            &arg_str(call, "old_string")?,
            &arg_str(call, "new_string")?,
        ),
        "list_dir" => list_dir(root, &arg_str(call, "path").unwrap_or_else(|_| ".".into())),
        "grep" => grep(root, &arg_str(call, "pattern")?, arg_opt(call, "path")),
        "run_command" => run_command(root, &arg_str(call, "command")?),
        other => bail!("unknown tool '{}'", other),
    }
}

// ── individual tools ─────────────────────────────────────────────────────────

fn read_file(root: &Path, rel: &str) -> Result<String> {
    let path = resolve(root, rel)?;
    let bytes = std::fs::read(&path).map_err(|e| anyhow!("could not read {}: {}", rel, e))?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if text.len() > MAX_READ_BYTES {
        text.truncate(MAX_READ_BYTES);
        text.push_str("\n... [truncated]");
    }
    Ok(text)
}

fn write_file(root: &Path, rel: &str, content: &str) -> Result<String> {
    let path = resolve(root, rel)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&path, content).map_err(|e| anyhow!("could not write {}: {}", rel, e))?;
    Ok(format!("wrote {} bytes to {}", content.len(), rel))
}

fn edit_file(root: &Path, rel: &str, old: &str, new: &str) -> Result<String> {
    let path = resolve(root, rel)?;
    let content = std::fs::read_to_string(&path).map_err(|e| anyhow!("could not read {}: {}", rel, e))?;
    let count = content.matches(old).count();
    if count == 0 {
        bail!("old_string not found in {} — it must match exactly (including whitespace)", rel);
    }
    if count > 1 {
        bail!("old_string occurs {} times in {} — make it unique (add surrounding context)", count, rel);
    }
    let updated = content.replacen(old, new, 1);
    std::fs::write(&path, updated).map_err(|e| anyhow!("could not write {}: {}", rel, e))?;
    Ok(format!("edited {} (1 replacement)", rel))
}

fn list_dir(root: &Path, rel: &str) -> Result<String> {
    let dir = resolve(root, rel)?;
    let mut entries: Vec<String> = vec![];
    let read = std::fs::read_dir(&dir).map_err(|e| anyhow!("could not list {}: {}", rel, e))?;
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.path().is_dir();
        if is_dir {
            if SKIP_DIRS.iter().any(|d| d.eq_ignore_ascii_case(&name)) {
                continue;
            }
            entries.push(format!("{}/", name));
        } else {
            entries.push(name);
        }
    }
    entries.sort();
    if entries.is_empty() {
        Ok(format!("(empty directory: {})", rel))
    } else {
        Ok(entries.join("\n"))
    }
}

fn grep(root: &Path, pattern: &str, sub: Option<String>) -> Result<String> {
    let base = match sub {
        Some(s) if !s.trim().is_empty() && s != "." => resolve(root, &s)?,
        _ => root.to_path_buf(),
    };
    let mut out: Vec<String> = vec![];
    let mut hits = 0usize;
    collect_matches(root, &base, pattern, &mut out, &mut hits);
    if out.is_empty() {
        Ok(format!("no matches for '{}'", pattern))
    } else {
        Ok(out.join("\n"))
    }
}

fn collect_matches(root: &Path, dir: &Path, pattern: &str, out: &mut Vec<String>, hits: &mut usize) {
    const MAX_HITS: usize = 200;
    if *hits >= MAX_HITS {
        return;
    }
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut paths: Vec<PathBuf> = read.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if *hits >= MAX_HITS {
            return;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if path.is_dir() {
            if SKIP_DIRS.iter().any(|d| d.eq_ignore_ascii_case(&name)) {
                continue;
            }
            collect_matches(root, &path, pattern, out, hits);
        } else {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SKIP_EXTS.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
                    continue;
                }
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                let rel = path.strip_prefix(root).unwrap_or(&path).display().to_string().replace('\\', "/");
                for (i, line) in content.lines().enumerate() {
                    if line.contains(pattern) {
                        out.push(format!("{}:{}: {}", rel, i + 1, line.trim()));
                        *hits += 1;
                        if *hits >= MAX_HITS {
                            out.push("... [more matches truncated]".into());
                            return;
                        }
                    }
                }
            }
        }
    }
}

fn run_command(root: &Path, command: &str) -> Result<String> {
    let output = shell(command, root).map_err(|e| anyhow!("failed to launch command: {}", e))?;
    let mut combined = String::new();
    combined.push_str(&String::from_utf8_lossy(&output.stdout));
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        combined.push_str("\n[stderr]\n");
        combined.push_str(&stderr);
    }
    if combined.len() > MAX_CMD_OUTPUT {
        combined.truncate(MAX_CMD_OUTPUT);
        combined.push_str("\n... [output truncated]");
    }
    let code = output.status.code().unwrap_or(-1);
    Ok(format!("exit code: {}\n{}", code, combined))
}

#[cfg(windows)]
fn shell(command: &str, cwd: &Path) -> std::io::Result<std::process::Output> {
    Command::new("cmd").arg("/C").arg(command).current_dir(cwd).output()
}

#[cfg(not(windows))]
fn shell(command: &str, cwd: &Path) -> std::io::Result<std::process::Output> {
    Command::new("sh").arg("-c").arg(command).current_dir(cwd).output()
}

// ── path sandboxing + argument helpers ───────────────────────────────────────

/// Resolve `rel` against `root` and guarantee the result stays inside the tree.
/// Normalizes `.`/`..` logically (without touching the filesystem) so it also
/// works for not-yet-existing files (e.g. `write_file` to a new path).
pub fn resolve(root: &Path, rel: &str) -> Result<PathBuf> {
    let candidate = Path::new(rel);
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };
    let normalized = normalize(&joined);
    let root_norm = normalize(root);
    if !normalized.starts_with(&root_norm) {
        bail!("path '{}' is outside the project root", rel);
    }
    Ok(normalized)
}

/// Logical path normalization: collapse `.` and `..` without resolving symlinks
/// or requiring the path to exist.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn arg_str(call: &ToolCall, key: &str) -> Result<String> {
    call.arguments
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("missing required string argument '{}'", key))
}

fn arg_opt(call: &ToolCall, key: &str) -> Option<String> {
    call.arguments.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ToolCall;
    use serde_json::{json, Value};

    fn call(name: &str, args: Value) -> ToolCall {
        ToolCall { id: "t".into(), name: name.into(), arguments: args }
    }

    #[test]
    fn write_then_read_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let w = execute(&call("write_file", json!({"path": "a/b.txt", "content": "hello"})), root);
        assert!(w.starts_with("wrote"), "{}", w);
        let r = execute(&call("read_file", json!({"path": "a/b.txt"})), root);
        assert_eq!(r, "hello");
    }

    #[test]
    fn edit_requires_unique_match() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("f.txt"), "x x").unwrap();
        let dup = execute(&call("edit_file", json!({"path": "f.txt", "old_string": "x", "new_string": "y"})), root);
        assert!(dup.contains("occurs 2 times"), "{}", dup);
        let ok = execute(&call("edit_file", json!({"path": "f.txt", "old_string": "x x", "new_string": "y y"})), root);
        assert!(ok.starts_with("edited"), "{}", ok);
        assert_eq!(std::fs::read_to_string(root.join("f.txt")).unwrap(), "y y");
    }

    #[test]
    fn grep_finds_substring_with_location() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("code.rs").as_path(), "fn main() {}\nlet needle = 1;\n").unwrap();
        let g = execute(&call("grep", json!({"pattern": "needle"})), root);
        assert!(g.contains("code.rs:2:"), "{}", g);
    }

    #[test]
    fn path_escape_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let r = execute(&call("read_file", json!({"path": "../../etc/passwd"})), root);
        assert!(r.starts_with("ERROR:") && r.contains("outside the project root"), "{}", r);
    }

    #[test]
    fn read_missing_file_returns_error_string() {
        let dir = tempfile::tempdir().unwrap();
        let r = execute(&call("read_file", json!({"path": "nope.txt"})), dir.path());
        assert!(r.starts_with("ERROR:"), "{}", r);
    }

    #[test]
    fn result_summaries_are_tool_aware() {
        assert_eq!(result_summary("list_dir", "a.rs\nb.rs\nc.rs"), "ok — 3 entries");
        assert_eq!(result_summary("list_dir", "(empty directory: x)"), "ok — empty");
        assert_eq!(result_summary("grep", "no matches for 'x'"), "ok — no matches");
        assert_eq!(result_summary("grep", "f.rs:1: a\nf.rs:2: b\n... [more matches truncated]"), "ok — 2 matches");
        assert_eq!(result_summary("run_command", "exit code: 0\nhello"), "ok — exit 0");
        assert_eq!(result_summary("read_file", "one\ntwo"), "ok — 2 lines, 7 bytes");
        assert!(result_summary("read_file", "ERROR: nope").starts_with("error —"));
    }
}
