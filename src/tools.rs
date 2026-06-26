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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
            description: "Search the project tree for a literal substring. Returns matching lines as `path:line: text`. Optionally restrict to a subdirectory or a single file.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Literal substring to search for"},
                    "path": {"type": "string", "description": "Optional path to limit the search (relative to root) — a subdirectory OR a single file"}
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
        ToolDef {
            name: "flag_risk".into(),
            description: "Flag a risk or decision that deserves the user's eyes NOW, mid-task — without waiting for a review gate. Use when you're proceeding past real uncertainty (an ambiguous requirement, a risky tradeoff, a possible breaking change, an assumption that could be wrong). It surfaces immediately in the UI and is recorded in .anvil/risks.md. This does NOT block you — note the risk and keep working.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "note": {"type": "string", "description": "The risk/decision, stated concretely (what, and why it matters)"}
                },
                "required": ["note"]
            }),
        },
        ToolDef {
            name: "delegate".into(),
            description: format!(
                "Delegate a focused evidence-gathering task to a read-only specialist sub-agent and get its findings back. Use this when you need information from OUTSIDE this project — the web, or another repository. The specialist returns evidence; you stay the decision-maker and the only one who edits code. Outward actions (fetching a URL, cloning a repo) ask the user for confirmation. Available specialists:\n{}",
                crate::specialist::help_listing()
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "specialist": {"type": "string", "enum": crate::specialist::names(), "description": "Which specialist to delegate to"},
                    "task": {"type": "string", "description": "A clear, self-contained description of what to find out. The specialist cannot see this conversation, so include all the context it needs."}
                },
                "required": ["specialist", "task"]
            }),
        },
    ]
}

/// The read-only subset of tools — safe for an investigating *reviewer*: no
/// writes, no edits, no command execution. Lets a reviewer verify the coder's
/// claims against the actual files instead of trusting the handoff/diff.
pub fn read_only_tool_defs() -> Vec<ToolDef> {
    const READ_ONLY: &[&str] = &["read_file", "list_dir", "grep", "project_state"];
    tool_defs()
        .into_iter()
        .filter(|d| READ_ONLY.contains(&d.name.as_str()))
        .collect()
}

/// Whether a tool must be confirmed by the user before it runs.
pub fn requires_confirmation(name: &str) -> bool {
    name == "run_command"
}

/// The built-in default auto-approve prefixes: read-only inspection + navigation
/// that cannot modify the repo, the filesystem, or anything outside it. Used when
/// the user has never configured an approval list (`approvals.auto_approve == None`).
/// Once they edit the `/approvals` checklist, their explicit list replaces this.
pub fn default_safe_prefixes() -> Vec<String> {
    [
        // read-only git
        "git status",
        "git diff",
        "git log",
        "git show",
        "git blame",
        "git grep",
        "git ls-files",
        "git rev-parse",
        "git describe",
        "git shortlog",
        // navigation / inspection
        "cd",
        "ls",
        "pwd",
        "cat",
        "head",
        "tail",
        "echo",
        "wc",
        "which",
        "where",
        "tree",
        "dir",
        "type",
        "stat",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Extra common commands offered (unchecked) in the `/approvals` checklist so the
/// user can opt into them. These can change state or run arbitrary code, so they
/// are NOT auto-approved by default — they're suggestions, not defaults.
pub fn suggested_command_catalog() -> Vec<String> {
    [
        "cargo build",
        "cargo check",
        "cargo test",
        "cargo clippy",
        "cargo fmt",
        "cargo run",
        "npm install",
        "npm test",
        "npm run build",
        "pnpm build",
        "yarn build",
        "node",
        "python",
        "python3",
        "pytest",
        "make",
        "git add",
        "git commit",
        "git push",
        "git pull",
        "git fetch",
        "git checkout",
        "git stash",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Whether `command` auto-runs without a confirmation prompt, given the user's
/// approved `prefixes`. A command matches when EVERY segment of any pipe/chain
/// matches one of the prefixes (token-aware: prefix "git diff" matches
/// "git diff --stat", "cd" matches "cd src" but not "cdfoo"). Commands containing
/// output redirection, command substitution, or an `--output` flag NEVER auto-run,
/// regardless of the list — those can write files or execute arbitrary code.
pub fn command_matches_prefixes(command: &str, prefixes: &[String]) -> bool {
    let cmd = command.trim();
    if cmd.is_empty() || prefixes.is_empty() {
        return false;
    }
    if cmd.contains('>') || cmd.contains('`') || cmd.contains("$(") || cmd.contains("--output") {
        return false;
    }
    cmd.split(['|', ';', '&'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .all(|seg| prefixes.iter().any(|p| segment_matches_prefix(seg, p)))
}

/// One command segment (split out of any pipe/chain) starts with `prefix`, matched
/// token-by-token. The program token (index 0) is normalized — lowercased, with any
/// leading path and a `.exe` suffix stripped — so `C:\Program Files\git.exe status`
/// still matches the prefix `git status`.
fn segment_matches_prefix(seg: &str, prefix: &str) -> bool {
    let ptoks: Vec<String> = prefix
        .split_whitespace()
        .map(|t| t.to_ascii_lowercase())
        .collect();
    if ptoks.is_empty() {
        return false;
    }
    let stoks: Vec<&str> = seg.split_whitespace().collect();
    if ptoks.len() > stoks.len() {
        return false;
    }
    for (i, pt) in ptoks.iter().enumerate() {
        let st = if i == 0 {
            let base = stoks[0]
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(stoks[0])
                .to_ascii_lowercase();
            base.strip_suffix(".exe").unwrap_or(&base).to_string()
        } else {
            stoks[i].to_ascii_lowercase()
        };
        if &st != pt {
            return false;
        }
    }
    true
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
        "delegate" => arg_str(call, "specialist").unwrap_or_default(),
        "flag_risk" => arg_str(call, "note").unwrap_or_default(),
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
        "flag_risk" => record_risk(root, &arg_str(call, "note")?),
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
    // Fresh interrupt state so Ctrl+B during this search stops it (mirrors
    // run_command). A grep whose pattern is rare/absent has no early-out from the
    // 200-hit cap, so it walks the ENTIRE tree — that must be cancellable and
    // self-limiting or it blocks the whole agent turn for minutes (a big
    // unskipped data file read fully into memory was enough to hang it).
    COMMAND_INTERRUPT.store(false, Ordering::SeqCst);
    let deadline = Instant::now() + GREP_TIME_BUDGET;
    let mut out: Vec<String> = vec![];
    let mut hits = 0usize;
    let mut stopped = false;
    // `path` may be a single file (search it directly) or a directory (recurse).
    // Without this, grepping an explicit file path read_dir'd it, failed, and
    // wrongly returned "no matches".
    if base.is_file() {
        search_file(
            root,
            &base,
            pattern,
            &mut out,
            &mut hits,
            deadline,
            &mut stopped,
        );
    } else {
        collect_matches(
            root,
            &base,
            pattern,
            &mut out,
            &mut hits,
            deadline,
            &mut stopped,
        );
    }
    if stopped {
        out.push(
            "... [search stopped early — interrupted or time budget reached; narrow it with a path argument]".into(),
        );
    }
    if out.is_empty() {
        Ok(format!("no matches for '{}'", pattern))
    } else {
        Ok(out.join("\n"))
    }
}

const MAX_GREP_HITS: usize = 200;

/// Skip files larger than this when grepping. A multi-MB data/log/JSON file read
/// fully into memory is what let a zero-match search hang the agent; real source
/// files are far smaller, and scanning a huge data dump inline isn't worth it.
const MAX_GREP_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Wall-clock cap on a single grep walk, so a rare pattern over a large tree
/// returns partial results with a note instead of blocking the turn.
const GREP_TIME_BUDGET: Duration = Duration::from_secs(15);

/// True when the in-flight grep should stop NOW — either Ctrl+B asked to
/// interrupt (`COMMAND_INTERRUPT`) or the time budget is exhausted. Checked
/// throughout the walk so a synchronous search can't run away.
fn grep_should_stop(deadline: Instant) -> bool {
    COMMAND_INTERRUPT.load(Ordering::SeqCst) || Instant::now() >= deadline
}

/// Search a single file's lines for `pattern`, appending `rel:line: text` hits.
fn search_file(
    root: &Path,
    path: &Path,
    pattern: &str,
    out: &mut Vec<String>,
    hits: &mut usize,
    deadline: Instant,
    stopped: &mut bool,
) {
    if *hits >= MAX_GREP_HITS {
        return;
    }
    if grep_should_stop(deadline) {
        *stopped = true;
        return;
    }
    // Don't pull a huge file into memory just to substring-scan it — that read is
    // what blocked the agent. Skip anything over the cap.
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > MAX_GREP_FILE_BYTES {
            return;
        }
    }
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/");
    for (i, line) in content.lines().enumerate() {
        if line.contains(pattern) {
            out.push(format!("{}:{}: {}", rel, i + 1, line.trim()));
            *hits += 1;
            if *hits >= MAX_GREP_HITS {
                out.push("... [more matches truncated]".into());
                return;
            }
        }
    }
}

fn collect_matches(
    root: &Path,
    dir: &Path,
    pattern: &str,
    out: &mut Vec<String>,
    hits: &mut usize,
    deadline: Instant,
    stopped: &mut bool,
) {
    if *hits >= MAX_GREP_HITS {
        return;
    }
    if grep_should_stop(deadline) {
        *stopped = true;
        return;
    }
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut paths: Vec<PathBuf> = read.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if *hits >= MAX_GREP_HITS {
            return;
        }
        if grep_should_stop(deadline) {
            *stopped = true;
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
            collect_matches(root, &path, pattern, out, hits, deadline, stopped);
        } else {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SKIP_EXTS.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
                    continue;
                }
            }
            search_file(root, &path, pattern, out, hits, deadline, stopped);
        }
    }
}

/// Set by the UI (Ctrl+B) to ask an in-flight `run_command` to stop NOW. The
/// command's poll loop sees it, kills the process tree, and returns promptly —
/// because a synchronous blocking command can't be cancelled by aborting the
/// async task alone (that only takes effect at an await point, and run_command
/// has none while the child runs).
static COMMAND_INTERRUPT: AtomicBool = AtomicBool::new(false);

/// Ask any currently-running `run_command` to abort and kill its child tree.
/// Called from the Ctrl+B handler. Harmless when nothing is running (the flag is
/// reset at the start of each command).
pub fn request_command_interrupt() {
    COMMAND_INTERRUPT.store(true, Ordering::SeqCst);
}

/// Default wall-clock cap on a single `run_command` before it's killed, so a test
/// or server that never returns can't hang the whole agent. Overridable per
/// environment via `ANVIL_COMMAND_TIMEOUT_SECS`.
const DEFAULT_COMMAND_TIMEOUT_SECS: u64 = 300;

fn command_timeout() -> Duration {
    let secs = std::env::var("ANVIL_COMMAND_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_COMMAND_TIMEOUT_SECS);
    Duration::from_secs(secs)
}

/// Drain a child pipe into a shared buffer on a background thread, returning the
/// buffer. We read in chunks and append as we go so the caller can SNAPSHOT the
/// output at any time without joining the thread. Critical for not hanging: if the
/// command spawned a process that survives the kill and keeps the pipe's write end
/// open (a dev server, a daemon), `read` never reaches EOF — but because we never
/// join this thread, that can't block the agent. The orphaned thread simply sits
/// on its blocking read until the pipe finally closes (harmless).
fn drain_to_shared<R: std::io::Read + Send + 'static>(mut r: R) -> Arc<Mutex<Vec<u8>>> {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&buf);
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        loop {
            match r.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Ok(mut b) = sink.lock() {
                        b.extend_from_slice(&chunk[..n]);
                    }
                }
            }
        }
    });
    buf
}

/// Append a coder-flagged risk to `.anvil/risks.md` (a visible, user-readable
/// file). The UI also surfaces it prominently in real time; this is the durable
/// record so it isn't lost when the transcript scrolls.
fn record_risk(root: &Path, note: &str) -> Result<String> {
    let note = note.trim();
    if note.is_empty() {
        bail!("flag_risk needs a non-empty note");
    }
    let path = crate::config::anvil_dir(root).join("risks.md");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let ts = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut out = if existing.trim().is_empty() {
        "# Risks flagged by the coder\n<!-- Mid-task risks/decisions the coder surfaced for your attention. Newest at the bottom. -->\n".to_string()
    } else {
        existing
    };
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("\n- [{}] {}\n", ts, note));
    std::fs::write(&path, out).map_err(|e| anyhow!("could not write risks.md: {}", e))?;
    Ok(format!("risk recorded to .anvil/risks.md: {}", note))
}

fn run_command(root: &Path, command: &str) -> Result<String> {
    // Fresh interrupt state for this command (drop any stale Ctrl+B from before).
    COMMAND_INTERRUPT.store(false, Ordering::SeqCst);

    let mut child =
        spawn_shell(command, root).map_err(|e| anyhow!("failed to launch command: {}", e))?;
    let pid = child.id();

    // Drain stdout/stderr into shared buffers (see drain_to_shared) — captured
    // incrementally and snapshotted WITHOUT joining, so a survivor process holding
    // the pipe open can't deadlock us.
    let out_buf = child.stdout.take().map(drain_to_shared);
    let err_buf = child.stderr.take().map(drain_to_shared);

    // Poll for completion, honoring both a timeout and a Ctrl+B interrupt. On
    // either, kill the whole process *tree* (the command may have spawned its own
    // children — a dev server, a test runner forking workers) and reap the child.
    let deadline = Instant::now() + command_timeout();
    let mut status = None;
    let mut stopped: Option<&str> = None; // Some("timeout") | Some("interrupt")
    loop {
        match child.try_wait() {
            Ok(Some(s)) => {
                status = Some(s);
                break;
            }
            Ok(None) => {}
            Err(_) => break,
        }
        if COMMAND_INTERRUPT.swap(false, Ordering::SeqCst) {
            stopped = Some("interrupt");
            kill_tree(pid);
            let _ = child.wait();
            break;
        }
        if Instant::now() >= deadline {
            stopped = Some("timeout");
            kill_tree(pid);
            let _ = child.wait();
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Let the drain threads flush, then SNAPSHOT (never join). On a normal exit the
    // pipe closes and output settles within a few ms; bound the wait so a killed
    // command whose orphaned grandchild still holds the pipe open can't stall us.
    let snap = |b: &Option<Arc<Mutex<Vec<u8>>>>| -> Vec<u8> {
        b.as_ref()
            .and_then(|m| m.lock().ok().map(|g| g.clone()))
            .unwrap_or_default()
    };
    let snap_len = |b: &Option<Arc<Mutex<Vec<u8>>>>| -> usize {
        b.as_ref()
            .and_then(|m| m.lock().ok().map(|g| g.len()))
            .unwrap_or(0)
    };
    let settle_cap =
        Instant::now() + Duration::from_millis(if stopped.is_some() { 150 } else { 1500 });
    let mut last_len = 0usize;
    loop {
        if Instant::now() >= settle_cap {
            break;
        }
        let cur = snap_len(&out_buf) + snap_len(&err_buf);
        if cur > 0 && cur == last_len {
            break; // output has stopped growing — done flushing
        }
        last_len = cur;
        std::thread::sleep(Duration::from_millis(20));
    }
    let out_bytes = snap(&out_buf);
    let err_bytes = snap(&err_buf);

    let mut combined = String::new();
    combined.push_str(&String::from_utf8_lossy(&out_bytes));
    let stderr = String::from_utf8_lossy(&err_bytes);
    if !stderr.trim().is_empty() {
        combined.push_str("\n[stderr]\n");
        combined.push_str(&stderr);
    }
    if combined.len() > MAX_CMD_OUTPUT {
        combined.truncate(MAX_CMD_OUTPUT);
        combined.push_str("\n... [output truncated]");
    }

    match stopped {
        Some("timeout") => Ok(format!(
            "exit code: -1\n[command timed out after {}s and its process tree was killed — it did NOT finish. If this command runs indefinitely (a dev server, a file watcher, or a test that hangs/loops), do NOT run it again — skip it or note it as deferred in .anvil/decisions.md so the reviewers know. If it just needs longer, raise ANVIL_COMMAND_TIMEOUT_SECS.]\n{}",
            command_timeout().as_secs(),
            combined
        )),
        Some("interrupt") => Ok(format!(
            "exit code: -1\n[command was interrupted by the user (Ctrl+B) and its process tree was killed. Don't blindly re-run it — ask the user how to proceed.]\n{}",
            combined
        )),
        _ => {
            let code = status.and_then(|s| s.code()).unwrap_or(-1);
            Ok(format!("exit code: {}\n{}", code, combined))
        }
    }
}

#[cfg(windows)]
fn spawn_shell(command: &str, cwd: &Path) -> std::io::Result<std::process::Child> {
    use std::process::Stdio;
    Command::new("cmd")
        .arg("/C")
        .arg(command)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

#[cfg(not(windows))]
fn spawn_shell(command: &str, cwd: &Path) -> std::io::Result<std::process::Child> {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Run in its own process group so we can signal the whole tree at once
        // (the child becomes the group leader; killing -pid kills the group).
        .process_group(0)
        .spawn()
}

/// Kill a process and everything it spawned. On Windows, `taskkill /T` walks the
/// child tree; on Unix the child leads its own process group, so a negative pid
/// signals the whole group. Best-effort — failures are ignored.
#[cfg(windows)]
fn kill_tree(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(not(windows))]
fn kill_tree(pid: u32) {
    // SIGKILL the whole process group (negative pid). The child was started as a
    // group leader via process_group(0).
    let _ = Command::new("kill")
        .arg("-KILL")
        .arg(format!("-{pid}"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
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
    fn grep_searches_an_explicit_file_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src/game.js"),
            "const GAME_STATE = {};\nlet waveDelay = 0;\n",
        )
        .unwrap();

        // Path = a FILE (the case that used to read_dir() and return nothing).
        let r = execute(
            &call(
                "grep",
                json!({"pattern": "GAME_STATE", "path": "src/game.js"}),
            ),
            root,
        );
        assert!(r.contains("src/game.js:1:"), "{r}");
        assert!(r.contains("GAME_STATE"), "{r}");

        // Path = a directory still recurses and finds it.
        let r2 = execute(&call("grep", json!({"pattern": "waveDelay"})), root);
        assert!(r2.contains("src/game.js:2:"), "{r2}");
    }

    #[test]
    fn default_safe_set_auto_approves_readonly_not_mutating() {
        let safe = default_safe_prefixes();
        // Read-only inspection + navigation — no prompt under the defaults.
        for ok in [
            "git status",
            "git diff --stat HEAD~1",
            "git log --oneline -5",
            "git show HEAD",
            "cd src",
            "ls -la",
            "pwd",
            "cat Cargo.toml",
            "git status && git diff", // chain of safe segments
            "git log | cat",
        ] {
            assert!(command_matches_prefixes(ok, &safe), "should be safe: {ok}");
        }
        // Mutating / arbitrary-code / unknown — still prompt under the defaults.
        for danger in [
            "cargo build", // executes arbitrary build/test code
            "git push",
            "git reset --hard",
            "git clean -fd",
            "git commit -m x",
            "git branch -D feat", // mutating git subcommand
            "rm -rf target",
            "echo hi > file.txt", // redirection writes a file
            "cat $(whoami)",      // command substitution
            "git diff && rm x",   // one unsafe segment poisons the chain
            "",
        ] {
            assert!(
                !command_matches_prefixes(danger, &safe),
                "should require approval: {danger}"
            );
        }
    }

    #[test]
    fn run_command_happy_path_captures_output_and_exit() {
        // Exercises the spawn + drain-threads + poll-loop path on both platforms.
        let dir = tempfile::tempdir().unwrap();
        let r = execute(
            &call("run_command", json!({"command": "echo hello_anvil"})),
            dir.path(),
        );
        assert!(r.starts_with("exit code: 0"), "{r}");
        assert!(r.contains("hello_anvil"), "{r}");
    }

    #[test]
    fn flag_risk_appends_to_risks_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let r = execute(
            &call("flag_risk", json!({"note": "auth token TTL is a guess"})),
            root,
        );
        assert!(r.contains("risk recorded"), "{r}");
        let risks = std::fs::read_to_string(root.join(".anvil/risks.md")).unwrap();
        assert!(risks.contains("auth token TTL is a guess"), "{risks}");
        // A second flag appends (both retained).
        execute(&call("flag_risk", json!({"note": "second risk"})), root);
        let risks2 = std::fs::read_to_string(root.join(".anvil/risks.md")).unwrap();
        assert!(risks2.contains("auth token TTL is a guess") && risks2.contains("second risk"));
        // Empty note is rejected.
        let bad = execute(&call("flag_risk", json!({"note": "  "})), root);
        assert!(bad.starts_with("ERROR:"), "{bad}");
    }

    #[test]
    fn command_prefix_matching_is_token_aware() {
        let prefixes = vec![
            "git diff".to_string(),
            "cargo build".to_string(),
            "cd".to_string(),
        ];
        // Prefix matches when leading tokens match.
        assert!(command_matches_prefixes(
            "git diff --stat HEAD~1",
            &prefixes
        ));
        assert!(command_matches_prefixes("cargo build --release", &prefixes));
        assert!(command_matches_prefixes("cd src/llm", &prefixes));
        // Not a prefix of an approved entry.
        assert!(!command_matches_prefixes("git push", &prefixes));
        assert!(!command_matches_prefixes("cargo test", &prefixes));
        // Token-aware: "cd" must not match "cdfoo".
        assert!(!command_matches_prefixes("cdfoo", &prefixes));
        // Chain: every segment must match an approved prefix.
        assert!(command_matches_prefixes("cd src && git diff", &prefixes));
        assert!(!command_matches_prefixes("git diff && rm x", &prefixes));
        // Redirection / substitution never auto-run.
        assert!(!command_matches_prefixes("git diff > out.txt", &prefixes));
        assert!(!command_matches_prefixes("git diff --output=x", &prefixes));
        // Empty list never matches.
        assert!(!command_matches_prefixes("git diff", &[]));
    }

    #[test]
    fn read_only_tool_defs_excludes_mutating_tools() {
        let names: Vec<String> = read_only_tool_defs().into_iter().map(|d| d.name).collect();
        for safe in ["read_file", "list_dir", "grep", "project_state"] {
            assert!(names.iter().any(|n| n == safe), "missing {safe}");
        }
        for danger in ["write_file", "edit_file", "apply_patch", "run_command"] {
            assert!(!names.iter().any(|n| n == danger), "leaked {danger}");
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
    fn grep_skips_oversized_files() {
        // A file over the size cap must not be read into memory (that exhaustive
        // read of a huge data file is what hung the agent). The match in it is
        // intentionally skipped; a normal-sized file with the same pattern hits.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut big = String::with_capacity((MAX_GREP_FILE_BYTES as usize) + 64);
        big.push_str("needle\n");
        while big.len() <= MAX_GREP_FILE_BYTES as usize {
            big.push_str("padding padding padding padding\n");
        }
        std::fs::write(root.join("huge.txt"), &big).unwrap();
        std::fs::write(root.join("small.txt"), "needle\n").unwrap();
        let g = execute(&call("grep", json!({"pattern": "needle"})), root);
        assert!(g.contains("small.txt:1:"), "{}", g);
        assert!(
            !g.contains("huge.txt"),
            "oversized file should be skipped: {}",
            g
        );
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
