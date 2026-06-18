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
    "target",
    ".git",
    "node_modules",
    ".anvil",
    "dist",
    "build",
    "out",
    "bin",
    "obj",
    "__pycache__",
    ".next",
    ".cache",
    "debug",
    "release",
    "archive",
];

/// File extensions treated as binary/uninteresting for search + listing.
pub const SKIP_EXTS: &[&str] = &[
    "exe", "dll", "so", "dylib", "o", "a", "lib", "pdb", "rlib", "rmeta", "d", "png", "jpg",
    "jpeg", "gif", "ico", "svg", "webp", "bmp", "pdf", "zip", "tar", "gz", "bz2", "7z", "rar",
    "xz", "bin", "dat", "db", "sqlite", "lock", "log",
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
            description: "Read a UTF-8 text file. By default returns the whole file (truncated if very large). For large files (hundreds of lines), prefer reading a section: pass offset (1-based start line) and limit (number of lines) — e.g. after grep finds a symbol's line.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path relative to the project root, e.g. src/llm.rs"},
                    "offset": {"type": "integer", "description": "Optional: 1-based line number to start reading from"},
                    "limit": {"type": "integer", "description": "Optional: maximum number of lines to return"}
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
            name: "apply_patch".into(),
            description: "PREFERRED way to edit existing files: apply a context-located patch to one or more files in a single call. More reliable than edit_file because it finds each change by its surrounding lines rather than an exact blob. Format:\n*** Begin Patch\n*** Update File: relative/path.rs\n@@ optional line to help locate the change\n unchanged context line (leading space)\n-removed line\n+added line\n*** Add File: relative/new.rs\n+first line of the new file\n+second line\n*** Delete File: relative/old.rs\n*** End Patch\nRules: include a few unchanged context lines (prefixed with a single space) around each change so it can be located; '-' removes, '+' adds. Context and removed lines must match the file EXACTLY, including indentation. You may edit multiple files in one patch. The whole patch is validated before anything is written.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "patch": {"type": "string", "description": "The full patch text, from '*** Begin Patch' to '*** End Patch'"}
                },
                "required": ["patch"]
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
            name: "project_state".into(),
            description: "Get a live snapshot of where the project stands: workflow stage, current phase and its plan excerpt, shipped phases, and git status/diff stat. Call this to re-ground yourself instead of guessing — it reads disk + git directly.".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
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
        "apply_patch" => {
            let p = arg_str(call, "patch").unwrap_or_default();
            let files = p
                .lines()
                .filter(|l| {
                    l.starts_with("*** Update File:")
                        || l.starts_with("*** Add File:")
                        || l.starts_with("*** Delete File:")
                })
                .count();
            format!("{} file(s)", files)
        }
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
        "read_file" => format!(
            "ok — {} lines, {} bytes",
            result.lines().count(),
            result.len()
        ),
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
        "project_state" => "ok — snapshot".to_string(),
        "run_command" => {
            // Result begins with "exit code: N".
            let code = result
                .lines()
                .next()
                .and_then(|l| l.strip_prefix("exit code:"))
                .map(|c| c.trim())
                .unwrap_or("?");
            // Don't label a non-zero exit "ok" — it reads like success in the UI.
            if code == "0" {
                "exit 0 (ok)".to_string()
            } else {
                format!("exit {} (FAILED)", code)
            }
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
        "read_file" => read_file(
            root,
            &arg_str(call, "path")?,
            arg_usize(call, "offset"),
            arg_usize(call, "limit"),
        ),
        "write_file" => write_file(root, &arg_str(call, "path")?, &arg_str(call, "content")?),
        "edit_file" => edit_file(
            root,
            &arg_str(call, "path")?,
            &arg_str(call, "old_string")?,
            &arg_str(call, "new_string")?,
        ),
        "apply_patch" => apply_patch(root, &arg_str(call, "patch")?),
        "list_dir" => list_dir(root, &arg_str(call, "path").unwrap_or_else(|_| ".".into())),
        "grep" => grep(root, &arg_str(call, "pattern")?, arg_opt(call, "path")),
        "project_state" => Ok(crate::reality::snapshot(root)),
        "run_command" => run_command(root, &arg_str(call, "command")?),
        other => bail!("unknown tool '{}'", other),
    }
}

// ── individual tools ─────────────────────────────────────────────────────────

fn read_file(
    root: &Path,
    rel: &str,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<String> {
    let path = resolve(root, rel)?;
    let bytes = std::fs::read(&path).map_err(|e| anyhow!("could not read {}: {}", rel, e))?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();

    // Line-range read: return just the requested slice (good for large files).
    if offset.is_some() || limit.is_some() {
        let lines: Vec<&str> = text.lines().collect();
        let total = lines.len();
        let start = offset.unwrap_or(1).max(1) - 1; // 1-based → 0-based
        let start = start.min(total);
        let count = limit.unwrap_or(total.saturating_sub(start));
        let end = (start + count).min(total);
        let slice = lines[start..end].join("\n");
        return Ok(format!(
            "[lines {}-{} of {} in {}]\n{}",
            start + 1,
            end,
            total,
            rel,
            slice
        ));
    }

    if text.len() > MAX_READ_BYTES {
        text.truncate(MAX_READ_BYTES);
        text.push_str("\n... [truncated — read a section with offset/limit for the rest]");
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
    let content =
        std::fs::read_to_string(&path).map_err(|e| anyhow!("could not read {}: {}", rel, e))?;
    let count = content.matches(old).count();
    if count == 0 {
        bail!(
            "old_string not found in {} — it must match exactly (including whitespace)",
            rel
        );
    }
    if count > 1 {
        bail!(
            "old_string occurs {} times in {} — make it unique (add surrounding context)",
            count,
            rel
        );
    }
    let updated = content.replacen(old, new, 1);
    std::fs::write(&path, updated).map_err(|e| anyhow!("could not write {}: {}", rel, e))?;
    Ok(format!("edited {} (1 replacement)", rel))
}

// ── apply_patch: context-located multi-file diffs (Codex-style; ROADMAP #3) ────

/// One change region inside an Update File: the lines to find (context + removed)
/// and the lines to put in their place (context + added).
struct Hunk {
    old: Vec<String>,
    new: Vec<String>,
}

enum PatchOp {
    Add { path: String, content: String },
    Delete { path: String },
    Update { path: String, hunks: Vec<Hunk> },
}

fn flush_hunk(hunks: &mut Vec<Hunk>, old: &mut Vec<String>, new: &mut Vec<String>) {
    if !old.is_empty() || !new.is_empty() {
        hunks.push(Hunk {
            old: std::mem::take(old),
            new: std::mem::take(new),
        });
    }
}

/// Parse the `*** Begin Patch` … `*** End Patch` envelope into file operations.
fn parse_patch(patch: &str) -> Result<Vec<PatchOp>> {
    let mut lines = patch.lines().peekable();

    // Skip leading blank lines, then require the Begin sentinel.
    let mut started = false;
    while let Some(l) = lines.peek() {
        if l.trim().is_empty() {
            lines.next();
            continue;
        }
        if l.trim_start() == "*** Begin Patch" {
            lines.next();
            started = true;
        }
        break;
    }
    if !started {
        bail!("patch must start with '*** Begin Patch'");
    }

    let mut ops: Vec<PatchOp> = Vec::new();
    while let Some(line) = lines.next() {
        if line.trim_start() == "*** End Patch" {
            return Ok(ops);
        }
        if let Some(rel) = line.strip_prefix("*** Add File: ") {
            let mut content: Vec<String> = Vec::new();
            while let Some(peek) = lines.peek() {
                if peek.starts_with("*** ") {
                    break;
                }
                let l = lines.next().unwrap();
                match l.strip_prefix('+') {
                    Some(rest) => content.push(rest.to_string()),
                    None if l.trim().is_empty() => content.push(String::new()),
                    None => bail!("Add File {}: lines must be '+' prefixed", rel.trim()),
                }
            }
            let body = if content.is_empty() {
                String::new()
            } else {
                format!("{}\n", content.join("\n"))
            };
            ops.push(PatchOp::Add {
                path: rel.trim().to_string(),
                content: body,
            });
        } else if let Some(rel) = line.strip_prefix("*** Delete File: ") {
            ops.push(PatchOp::Delete {
                path: rel.trim().to_string(),
            });
        } else if let Some(rel) = line.strip_prefix("*** Update File: ") {
            let mut hunks: Vec<Hunk> = Vec::new();
            let mut old: Vec<String> = Vec::new();
            let mut new: Vec<String> = Vec::new();
            while let Some(peek) = lines.peek() {
                if peek.starts_with("*** ") {
                    break;
                }
                let l = lines.next().unwrap();
                if l.starts_with("@@") {
                    // A new locator section starts a fresh hunk.
                    flush_hunk(&mut hunks, &mut old, &mut new);
                    continue;
                }
                match l.chars().next() {
                    Some(' ') => {
                        old.push(l[1..].to_string());
                        new.push(l[1..].to_string());
                    }
                    Some('-') => old.push(l[1..].to_string()),
                    Some('+') => new.push(l[1..].to_string()),
                    None => {
                        // Bare empty line — treat as a blank context line.
                        old.push(String::new());
                        new.push(String::new());
                    }
                    Some(_) => bail!(
                        "Update File {}: each change line must start with ' ', '-', '+', or '@@' (got: {})",
                        rel.trim(),
                        l
                    ),
                }
            }
            flush_hunk(&mut hunks, &mut old, &mut new);
            if hunks.is_empty() {
                bail!("Update File {}: no change lines", rel.trim());
            }
            ops.push(PatchOp::Update {
                path: rel.trim().to_string(),
                hunks,
            });
        } else if line.trim().is_empty() {
            continue;
        } else {
            bail!(
                "unexpected line in patch (want '*** Add/Update/Delete File:' or '*** End Patch'): {}",
                line
            );
        }
    }
    bail!("patch is missing '*** End Patch'")
}

/// Find `block` as a contiguous run of lines in `lines`, at or after `from`.
fn find_block(lines: &[String], block: &[String], from: usize) -> Option<usize> {
    if block.is_empty() || block.len() > lines.len() {
        return None;
    }
    (from..=lines.len() - block.len()).find(|&i| lines[i..i + block.len()] == block[..])
}

/// Apply an Update File's hunks to the original text, locating each by its
/// context+removed lines. Preserves the file's dominant newline style.
fn apply_hunks(original: &str, hunks: &[Hunk]) -> Result<String> {
    let uses_crlf = original.contains("\r\n");
    let had_trailing_nl = original.ends_with('\n');
    // str::lines() strips both \n and \r\n terminators, so lines carry no \r —
    // and the patch text was split the same way, so matching is newline-agnostic.
    let mut lines: Vec<String> = original.lines().map(|s| s.to_string()).collect();

    let mut search_from = 0usize;
    for (i, hunk) in hunks.iter().enumerate() {
        if hunk.old.is_empty() {
            bail!(
                "hunk {} has no context or removed lines, so the change can't be located — include a few surrounding ' ' lines",
                i + 1
            );
        }
        let pos = find_block(&lines, &hunk.old, search_from).ok_or_else(|| {
            anyhow!(
                "hunk {} did not match — its ' ' and '-' lines must equal the file exactly (check indentation/whitespace)",
                i + 1
            )
        })?;
        lines.splice(pos..pos + hunk.old.len(), hunk.new.iter().cloned());
        search_from = pos + hunk.new.len();
    }

    let nl = if uses_crlf { "\r\n" } else { "\n" };
    let mut result = lines.join(nl);
    if had_trailing_nl {
        result.push_str(nl);
    }
    Ok(result)
}

/// Apply a multi-file patch. Validates everything (resolves paths, locates all
/// hunks, computes new contents) BEFORE writing, so a bad hunk can't half-apply.
fn apply_patch(root: &Path, patch: &str) -> Result<String> {
    let ops = parse_patch(patch)?;
    if ops.is_empty() {
        bail!("empty patch — no file operations found");
    }

    enum Planned {
        Write(PathBuf, String, String),
        Delete(PathBuf, String),
    }
    let mut planned: Vec<Planned> = Vec::new();

    for op in &ops {
        match op {
            PatchOp::Add { path, content } => {
                let p = resolve(root, path)?;
                if p.exists() {
                    bail!(
                        "Add File {}: already exists (use Update File to modify it)",
                        path
                    );
                }
                planned.push(Planned::Write(
                    p,
                    content.clone(),
                    format!("added {}", path),
                ));
            }
            PatchOp::Delete { path } => {
                let p = resolve(root, path)?;
                if !p.exists() {
                    bail!("Delete File {}: does not exist", path);
                }
                planned.push(Planned::Delete(p, format!("deleted {}", path)));
            }
            PatchOp::Update { path, hunks } => {
                let p = resolve(root, path)?;
                let orig = std::fs::read_to_string(&p)
                    .map_err(|e| anyhow!("Update File {}: could not read: {}", path, e))?;
                let updated = apply_hunks(&orig, hunks)
                    .map_err(|e| anyhow!("Update File {}: {}", path, e))?;
                let n = hunks.len();
                planned.push(Planned::Write(
                    p,
                    updated,
                    format!(
                        "updated {} ({} hunk{})",
                        path,
                        n,
                        if n == 1 { "" } else { "s" }
                    ),
                ));
            }
        }
    }

    let mut summaries: Vec<String> = Vec::new();
    for pl in planned {
        match pl {
            Planned::Write(p, content, summary) => {
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                std::fs::write(&p, content)
                    .map_err(|e| anyhow!("could not write {}: {}", p.display(), e))?;
                summaries.push(summary);
            }
            Planned::Delete(p, summary) => {
                std::fs::remove_file(&p)
                    .map_err(|e| anyhow!("could not delete {}: {}", p.display(), e))?;
                summaries.push(summary);
            }
        }
    }
    Ok(format!("apply_patch ok — {}", summaries.join(", ")))
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

fn collect_matches(
    root: &Path,
    dir: &Path,
    pattern: &str,
    out: &mut Vec<String>,
    hits: &mut usize,
) {
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
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
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
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string()
                    .replace('\\', "/");
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
    Command::new("cmd")
        .arg("/C")
        .arg(command)
        .current_dir(cwd)
        .output()
}

#[cfg(not(windows))]
fn shell(command: &str, cwd: &Path) -> std::io::Result<std::process::Output> {
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .output()
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

/// Optional unsigned-integer argument (accepts a JSON number or numeric string).
fn arg_usize(call: &ToolCall, key: &str) -> Option<usize> {
    let v = call.arguments.get(key)?;
    v.as_u64()
        .map(|n| n as usize)
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

fn arg_opt(call: &ToolCall, key: &str) -> Option<String> {
    call.arguments
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ToolCall;
    use serde_json::{json, Value};

    fn call(name: &str, args: Value) -> ToolCall {
        ToolCall {
            id: "t".into(),
            name: name.into(),
            arguments: args,
        }
    }

    #[test]
    fn write_then_read_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let w = execute(
            &call("write_file", json!({"path": "a/b.txt", "content": "hello"})),
            root,
        );
        assert!(w.starts_with("wrote"), "{}", w);
        let r = execute(&call("read_file", json!({"path": "a/b.txt"})), root);
        assert_eq!(r, "hello");
    }

    #[test]
    fn apply_patch_updates_with_context() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("m.rs"), "fn main() {\n    println!(\"hi\");\n}\n").unwrap();
        let patch = "*** Begin Patch\n*** Update File: m.rs\n@@ fn main() {\n fn main() {\n-    println!(\"hi\");\n+    println!(\"hello\");\n }\n*** End Patch\n";
        let r = execute(&call("apply_patch", json!({ "patch": patch })), root);
        assert!(r.starts_with("apply_patch ok"), "{}", r);
        assert_eq!(
            std::fs::read_to_string(root.join("m.rs")).unwrap(),
            "fn main() {\n    println!(\"hello\");\n}\n"
        );
    }

    #[test]
    fn apply_patch_adds_and_deletes_in_one_call() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("old.txt"), "bye\n").unwrap();
        let patch = "*** Begin Patch\n*** Add File: new.txt\n+line one\n+line two\n*** Delete File: old.txt\n*** End Patch\n";
        let r = execute(&call("apply_patch", json!({ "patch": patch })), root);
        assert!(r.starts_with("apply_patch ok"), "{}", r);
        assert_eq!(
            std::fs::read_to_string(root.join("new.txt")).unwrap(),
            "line one\nline two\n"
        );
        assert!(!root.join("old.txt").exists());
    }

    #[test]
    fn apply_patch_mismatch_errors_and_leaves_file_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("m.rs"), "let x = 1;\n").unwrap();
        // Context/removed lines that don't exist in the file → validation fails
        // before any write, so the file must be untouched.
        let patch = "*** Begin Patch\n*** Update File: m.rs\n let y = 2;\n-let z = 3;\n+let z = 4;\n*** End Patch\n";
        let r = execute(&call("apply_patch", json!({ "patch": patch })), root);
        assert!(r.starts_with("ERROR:"), "{}", r);
        assert_eq!(
            std::fs::read_to_string(root.join("m.rs")).unwrap(),
            "let x = 1;\n"
        );
    }

    #[test]
    fn edit_requires_unique_match() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("f.txt"), "x x").unwrap();
        let dup = execute(
            &call(
                "edit_file",
                json!({"path": "f.txt", "old_string": "x", "new_string": "y"}),
            ),
            root,
        );
        assert!(dup.contains("occurs 2 times"), "{}", dup);
        let ok = execute(
            &call(
                "edit_file",
                json!({"path": "f.txt", "old_string": "x x", "new_string": "y y"}),
            ),
            root,
        );
        assert!(ok.starts_with("edited"), "{}", ok);
        assert_eq!(std::fs::read_to_string(root.join("f.txt")).unwrap(), "y y");
    }

    #[test]
    fn grep_finds_substring_with_location() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("code.rs").as_path(),
            "fn main() {}\nlet needle = 1;\n",
        )
        .unwrap();
        let g = execute(&call("grep", json!({"pattern": "needle"})), root);
        assert!(g.contains("code.rs:2:"), "{}", g);
    }

    #[test]
    fn path_escape_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let r = execute(
            &call("read_file", json!({"path": "../../etc/passwd"})),
            root,
        );
        assert!(
            r.starts_with("ERROR:") && r.contains("outside the project root"),
            "{}",
            r
        );
    }

    #[test]
    fn read_missing_file_returns_error_string() {
        let dir = tempfile::tempdir().unwrap();
        let r = execute(&call("read_file", json!({"path": "nope.txt"})), dir.path());
        assert!(r.starts_with("ERROR:"), "{}", r);
    }

    #[test]
    fn result_summaries_are_tool_aware() {
        assert_eq!(
            result_summary("list_dir", "a.rs\nb.rs\nc.rs"),
            "ok — 3 entries"
        );
        assert_eq!(
            result_summary("list_dir", "(empty directory: x)"),
            "ok — empty"
        );
        assert_eq!(
            result_summary("grep", "no matches for 'x'"),
            "ok — no matches"
        );
        assert_eq!(
            result_summary("grep", "f.rs:1: a\nf.rs:2: b\n... [more matches truncated]"),
            "ok — 2 matches"
        );
        assert_eq!(
            result_summary("run_command", "exit code: 0\nhello"),
            "exit 0 (ok)"
        );
        assert_eq!(
            result_summary("run_command", "exit code: 101\nerror[E0425]"),
            "exit 101 (FAILED)"
        );
        assert_eq!(
            result_summary("read_file", "one\ntwo"),
            "ok — 2 lines, 7 bytes"
        );
        assert!(result_summary("read_file", "ERROR: nope").starts_with("error —"));
    }
}
