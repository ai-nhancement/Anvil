//! Git bootstrap — Anvil's review gates are built entirely on git: a phase review is
//! `git diff <base>..worktree`, and `/review` diffs the working tree. So a project
//! MUST be a git repo with at least a baseline commit, or the reviewers get an empty
//! diff and (reasonably) conclude nothing was built. This module makes that true
//! automatically the first time Anvil opens a project, so the workflow can't silently
//! run on a non-git folder.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

/// What [`ensure_repo_ready`] actually did, so the caller can tell the user.
pub enum GitBootstrap {
    /// Already a git repo with at least one commit — nothing to do (the common case).
    AlreadyReady,
    /// Ran `git init` and committed a baseline of the existing files.
    InitializedWithBaseline,
    /// Ran `git init`, but the project was empty so there's no baseline commit yet.
    InitializedEmpty,
    /// An existing repo that had no commits — made a baseline commit.
    BaselineCommitted,
    /// `git` isn't installed / not on PATH — Anvil can't bootstrap; caller warns.
    GitUnavailable,
    /// A git step failed; carries a short reason for the user.
    Failed(String),
}

fn git(root: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("git").args(args).current_dir(root).output()
}

/// Run a git command, returning its stdout on success or a short stderr message.
fn git_ok(root: &Path, args: &[&str]) -> Result<String, String> {
    match git(root, args) {
        Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).into_owned()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn is_repo(root: &Path) -> bool {
    git(root, &["rev-parse", "--is-inside-work-tree"])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn has_head(root: &Path) -> bool {
    git(root, &["rev-parse", "--verify", "-q", "HEAD"])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Ensure `root` is a git repo with a baseline commit, initializing and committing if
/// needed. Idempotent and safe to call on every launch — an already-ready repo is left
/// completely untouched. Only the bootstrap paths touch `.gitignore` / create a commit.
pub fn ensure_repo_ready(root: &Path) -> GitBootstrap {
    if !git_available() {
        return GitBootstrap::GitUnavailable;
    }
    let was_repo = is_repo(root);
    if !was_repo {
        if let Err(e) = git_ok(root, &["init", "-q"]) {
            return GitBootstrap::Failed(format!("git init failed: {e}"));
        }
    }
    // Existing repo with history → it already works; never touch the user's setup.
    if has_head(root) {
        return GitBootstrap::AlreadyReady;
    }

    // We're establishing the baseline (fresh init, or an existing repo with no
    // commits). Seed .gitignore FIRST so the baseline `git add -A` doesn't sweep in
    // dependencies/build output/secrets (.anvil/, node_modules, target, .env, …).
    seed_gitignore(root);

    if let Err(e) = git_ok(root, &["add", "-A"]) {
        return GitBootstrap::Failed(format!("git add failed: {e}"));
    }
    let has_staged = git_ok(root, &["diff", "--cached", "--name-only"])
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if !has_staged {
        return GitBootstrap::InitializedEmpty;
    }
    if let Err(e) = commit_baseline(root) {
        return GitBootstrap::Failed(format!("baseline commit failed: {e}"));
    }
    if was_repo {
        GitBootstrap::BaselineCommitted
    } else {
        GitBootstrap::InitializedWithBaseline
    }
}

/// Commit the staged baseline. Uses the user's configured git identity when present;
/// otherwise supplies a fallback identity for THIS commit only (via `-c`), so the
/// bootstrap still works on a machine where `git config user.*` was never set —
/// without writing anything to the user's git config.
fn commit_baseline(root: &Path) -> Result<(), String> {
    let msg = "chore: baseline commit (Anvil)";
    if has_identity(root) {
        git_ok(root, &["commit", "-q", "-m", msg]).map(|_| ())
    } else {
        git_ok(
            root,
            &[
                "-c",
                "user.name=Anvil",
                "-c",
                "user.email=anvil@localhost",
                "commit",
                "-q",
                "-m",
                msg,
            ],
        )
        .map(|_| ())
    }
}

fn has_identity(root: &Path) -> bool {
    let set = |key: &str| {
        git_ok(root, &["config", key])
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    };
    set("user.name") && set("user.email")
}

/// Normalize a `.gitignore` line for set membership: drop comments/blanks, trim, and
/// strip leading/trailing `/` so `target`, `/target`, and `target/` all compare equal.
fn normalize_ignore(line: &str) -> String {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return String::new();
    }
    t.trim_start_matches('/').trim_end_matches('/').to_string()
}

/// Seed `.gitignore` (before the baseline commit) so Anvil's own state, plus the
/// detected stack's dependency/build dirs and common secret/log files, never land in
/// the baseline. Only ever APPENDS patterns that aren't already present — it never
/// rewrites or removes what the project already ignores — and the gate artifacts
/// (`plan.md`, `REVIEW_*.md`) live at the repo ROOT, so they stay tracked.
fn seed_gitignore(root: &Path) {
    let gi = root.join(".gitignore");
    let existing = std::fs::read_to_string(&gi).unwrap_or_default();
    // Patterns already covered (by the file, and accumulated as we add) so we never
    // duplicate — across the existing file AND between our own groups.
    let mut have: HashSet<String> = existing
        .lines()
        .map(normalize_ignore)
        .filter(|s| !s.is_empty())
        .collect();

    // (section label, patterns). The first few apply to every project; the rest are
    // gated on a detected manifest so we only seed ignores relevant to this stack.
    let mut groups: Vec<(&str, Vec<&str>)> = vec![
        ("Anvil session/state (local only)", vec![".anvil/"]),
        (
            "Secrets & local env",
            vec![".env", ".env.local", ".env.*.local"],
        ),
        ("Logs & OS cruft", vec!["*.log", ".DS_Store", "Thumbs.db"]),
    ];
    let has = |name: &str| root.join(name).exists();
    if has("package.json") {
        groups.push((
            "Node",
            vec!["node_modules/", "dist/", "build/", "npm-debug.log*"],
        ));
    }
    if has("Cargo.toml") {
        groups.push(("Rust", vec!["/target/"]));
    }
    if has("pyproject.toml") || has("requirements.txt") || has("setup.py") {
        groups.push((
            "Python",
            vec!["__pycache__/", "*.pyc", ".venv/", "venv/", ".pytest_cache/"],
        ));
    }
    if has("go.mod") {
        groups.push(("Go", vec!["/bin/"]));
    }
    if has("pom.xml") || has("build.gradle") || has("build.gradle.kts") {
        groups.push(("JVM", vec!["target/", "build/", ".gradle/"]));
    }

    let mut block = String::new();
    for (label, patterns) in groups {
        let mut missing: Vec<&str> = Vec::new();
        for p in patterns {
            let n = normalize_ignore(p);
            if n.is_empty() || have.contains(&n) {
                continue;
            }
            have.insert(n);
            missing.push(p);
        }
        if missing.is_empty() {
            continue;
        }
        block.push_str(&format!("\n# {label} (added by Anvil)\n"));
        for p in missing {
            block.push_str(p);
            block.push('\n');
        }
    }
    if block.is_empty() {
        return; // everything relevant is already ignored — leave the file untouched
    }

    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&block);
    let _ = std::fs::write(&gi, out);
}

/// A user-facing line describing what the bootstrap did (None when nothing happened).
pub fn bootstrap_message(outcome: &GitBootstrap) -> Option<String> {
    match outcome {
        GitBootstrap::AlreadyReady => None,
        GitBootstrap::InitializedWithBaseline => Some(
            "This wasn't a git repository — Anvil ran `git init`, seeded a `.gitignore` (your stack's \
             build/dependency dirs, common secrets like `.env`, and Anvil's own `.anvil/`), and \
             committed a baseline, so the review gates (phase diffs and /review) can diff your \
             changes. Your plan.md and REVIEW_* files stay tracked. Review the `.gitignore` if you \
             keep anything unusual."
                .to_string(),
        ),
        GitBootstrap::BaselineCommitted => Some(
            "This repository had no commits — Anvil seeded a `.gitignore` (build/dependency dirs, \
             secrets, and `.anvil/`) and made a baseline commit so the review gates have something \
             to diff your work against."
                .to_string(),
        ),
        GitBootstrap::InitializedEmpty => Some(
            "Initialized an empty git repository here (`git init`). Once you add files they become \
             the baseline the review gates diff against."
                .to_string(),
        ),
        GitBootstrap::GitUnavailable => Some(
            "⚠ git isn't installed or isn't on your PATH — but Anvil's review gates (phase diffs and \
             /review) are built on git and can't work without it. Install git, then restart Anvil."
                .to_string(),
        ),
        GitBootstrap::Failed(e) => Some(format!(
            "⚠ Anvil couldn't set up git automatically ({e}). The review gates need a git repo with \
             a baseline commit — run `git init` and commit a baseline manually."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_present() -> bool {
        super::git_available()
    }

    #[test]
    fn bootstrap_initializes_non_git_project_with_baseline() {
        if !git_present() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("game.js"), "export const x = 1;\n").unwrap();

        let outcome = ensure_repo_ready(root);
        assert!(matches!(outcome, GitBootstrap::InitializedWithBaseline));
        assert!(root.join(".git").exists(), "git repo created");
        assert!(has_head(root), "baseline commit exists");
        // .anvil/ is ignored, and the source file is tracked in the baseline.
        let gi = std::fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(gi.contains(".anvil/"), "{gi}");
        let tracked = git_ok(root, &["ls-files"]).unwrap();
        assert!(tracked.contains("game.js"), "{tracked}");

        // Idempotent: a second call is a no-op on the now-ready repo.
        assert!(matches!(
            ensure_repo_ready(root),
            GitBootstrap::AlreadyReady
        ));
    }

    #[test]
    fn bootstrap_of_empty_project_still_establishes_a_baseline() {
        if !git_present() {
            return;
        }
        // Even an empty folder gets a working baseline: Anvil writes a .gitignore
        // (excluding .anvil/) and commits it, so there's always a HEAD to diff against.
        let dir = tempfile::tempdir().unwrap();
        let outcome = ensure_repo_ready(dir.path());
        assert!(matches!(outcome, GitBootstrap::InitializedWithBaseline));
        assert!(dir.path().join(".git").exists());
        assert!(has_head(dir.path()), "baseline commit exists");
        assert!(dir.path().join(".gitignore").exists());
    }

    #[test]
    fn seed_gitignore_detects_stack_and_dedupes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // A Node project, with a partial .gitignore the user already wrote.
        std::fs::write(root.join("package.json"), "{}\n").unwrap();
        std::fs::write(root.join(".gitignore"), "node_modules/\n# mine\ncustom/\n").unwrap();

        seed_gitignore(root);
        let gi = std::fs::read_to_string(root.join(".gitignore")).unwrap();

        // Stack-specific + universal ignores were added.
        assert!(gi.contains(".anvil/"), "{gi}");
        assert!(gi.contains(".env"), "{gi}");
        assert!(gi.contains("dist/"), "{gi}");
        // The user's existing lines are preserved...
        assert!(gi.contains("custom/"), "{gi}");
        // ...and an already-present pattern is NOT duplicated.
        assert_eq!(gi.matches("node_modules/").count(), 1, "{gi}");

        // A Rust project gets /target/, not Node's node_modules.
        let rdir = tempfile::tempdir().unwrap();
        std::fs::write(rdir.path().join("Cargo.toml"), "[package]\n").unwrap();
        seed_gitignore(rdir.path());
        let rgi = std::fs::read_to_string(rdir.path().join(".gitignore")).unwrap();
        assert!(rgi.contains("/target/"), "{rgi}");
        assert!(!rgi.contains("node_modules"), "{rgi}");
    }

    #[test]
    fn bootstrap_leaves_existing_repo_untouched() {
        if !git_present() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let run = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(root)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .output()
                .unwrap()
        };
        run(&["init", "-q"]);
        std::fs::write(root.join("a.txt"), "hi\n").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-qm", "first"]);

        // Already has history → AlreadyReady, and we must NOT create a .gitignore.
        assert!(matches!(
            ensure_repo_ready(root),
            GitBootstrap::AlreadyReady
        ));
        assert!(
            !root.join(".gitignore").exists(),
            "must not touch an established repo's .gitignore"
        );
    }
}
