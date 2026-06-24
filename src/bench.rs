//! Tool-dialect benchmark — measure tool-use *fidelity* per `model × dialect`
//! over a corpus of deterministic edit fixtures, scored against a known-good
//! `after/` tree. See `docs/ROADMAP_tool_dialect_bench.md`.
//!
//! Methodology (the make-or-break rule): isolate tool-fit from model
//! intelligence. Every fixture has a single correct result, so a cell's score is
//! "did this model, on this dialect, land the known edit cleanly?" — compared
//! WITHIN a model (Codex vs Generic vs …), never as a cross-model leaderboard.
//!
//! The deterministic core (fixture loading, scratch copy, directory comparison,
//! scoring) is pure and unit-tested. Driving a real model needs configured
//! providers + network, so the sweep itself is run by the user via `anvil bench`
//! from the Anvil source tree (where `bench/fixtures/` lives).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

use crate::config::{load_config, load_local_env};
use crate::dialect::Dialect;
use crate::llm::{ChatMessage, LlmClient, ToolCall};

/// Safety cap on tool-call iterations per benchmark run (a single edit task).
const MAX_STEPS: usize = 12;

/// Neutral, dialect-agnostic coder prompt. The dialect's own addendum is appended
/// at the gateway (`chat_turn_stream`), so each arm gets a fair family baseline
/// without the *task* favoring one surface.
const BENCH_SYSTEM: &str = "You are a coding agent operating on a small project via file tools. Make exactly the change the user describes by calling tools, then stop (reply with a one-line confirmation and no further tool call). Read a file before editing it. Keep the change minimal and precisely as requested — do not reformat or touch unrelated lines.";

#[derive(Deserialize)]
struct TaskToml {
    edit_type: String,
    instruction: String,
    /// LIVE fixture: a command the harness actually runs in the scratch tree so the
    /// model can verify (and so we score by whether the tests pass, not exact text).
    #[serde(default)]
    check: Option<String>,
}

/// One benchmark case: an instruction plus a `before/` tree to mutate and an
/// `after/` tree to score against.
pub struct Fixture {
    pub id: String,
    pub edit_type: String,
    pub instruction: String,
    pub dir: PathBuf,
    /// When set, this is a LIVE (multi-step) fixture: the harness runs `check` in the
    /// scratch on every run_command, and scores by whether it passes — see `run_one`.
    /// `after/` then holds only the PROTECTED files (e.g. the test) that must stay
    /// byte-intact; the edited code's exact text is unconstrained (the test judges it).
    pub check: Option<String>,
}

/// Outcome of a single run of one fixture under one dialect.
#[derive(Debug, Clone, Default)]
struct RunOutcome {
    /// The turn errored (network/provider) — excluded from fidelity rates.
    errored: bool,
    /// The first assistant turn produced at least one tool call.
    first_call_valid: bool,
    /// At least one edit/write tool applied without an `ERROR:` result.
    edit_landed: bool,
    /// Tool-call iterations used.
    steps: usize,
    /// The resulting tree exactly matches `after/`.
    correct: bool,
    /// The transport error message if the turn failed (for diagnostics).
    error: Option<String>,
    /// `(tool_name, is_error)` for each executed call — powers the per-tool report
    /// ("which tools does this model struggle with?").
    tool_events: Vec<(String, bool)>,
    /// Assistant text on turn 1 when it made NO tool call — diagnoses a model that
    /// answers in prose instead of driving the tools.
    no_call_text: Option<String>,
    /// For a FAILED run: what the model actually produced vs. what was expected (or
    /// the live check's failure output). Powers the "why did it fail?" diagnostic.
    fail_detail: Option<String>,
    /// The ordered tool calls this run made (name + a short arg summary). Captured so
    /// `--trace` can show HOW two arms approach the same fixture differently.
    trace: Vec<String>,
}

// ── deterministic core (pure, unit-tested) ───────────────────────────────────

/// Recursively map a directory to `relative path -> normalized content` (sorted
/// for determinism). Line endings are normalized (CRLF → LF) and trailing
/// whitespace is trimmed, so a model's LF output still matches a fixture that git
/// checked out as CRLF on Windows — internal indentation (the tricky-whitespace
/// case) is preserved. Returns an empty map if the directory does not exist.
fn collect_files(base: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip build/cache artifacts a live `check` may create (e.g. Python's
            // __pycache__/*.pyc) so they never count as a tree difference.
            if name == "__pycache__" || name == ".pytest_cache" || name.ends_with(".pyc") {
                continue;
            }
            if path.is_dir() {
                walk(base, &path, out);
            } else if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(rel) = path.strip_prefix(base) {
                    // Normalize per-line trailing whitespace (and file-end), so a
                    // stray trailing space on an edited line isn't a false failure.
                    // Leading indentation (the tricky-whitespace signal) is kept.
                    let norm = String::from_utf8_lossy(&bytes)
                        .replace("\r\n", "\n")
                        .lines()
                        .map(|l| l.trim_end())
                        .collect::<Vec<_>>()
                        .join("\n")
                        .trim_end()
                        .to_string();
                    out.insert(rel.to_string_lossy().replace('\\', "/"), norm);
                }
            }
        }
    }
    walk(base, base, &mut out);
    out
}

/// True when two directory trees have the same files with identical contents.
fn dirs_equal(a: &Path, b: &Path) -> bool {
    collect_files(a) == collect_files(b)
}

/// Recursively copy `src` into `dst` (creating `dst`).
fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Load every fixture under `bench/fixtures/<id>/` (each needs `task.toml`, a
/// `before/` dir, and an `after/` dir).
fn load_fixtures(fixtures_root: &Path) -> Result<Vec<Fixture>> {
    if !fixtures_root.is_dir() {
        bail!(
            "no fixtures at {} — run `anvil bench` from the Anvil source tree",
            fixtures_root.display()
        );
    }
    let mut fixtures = vec![];
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(fixtures_root)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let task_path = dir.join("task.toml");
        if !task_path.exists() {
            continue; // not a fixture dir (e.g. a README)
        }
        let raw = std::fs::read_to_string(&task_path)
            .with_context(|| format!("reading {}", task_path.display()))?;
        let task: TaskToml =
            toml::from_str(&raw).with_context(|| format!("parsing {}", task_path.display()))?;
        let id = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !dir.join("before").is_dir() || !dir.join("after").is_dir() {
            bail!("fixture {} is missing before/ or after/", id);
        }
        fixtures.push(Fixture {
            id,
            edit_type: task.edit_type,
            instruction: task.instruction,
            dir,
            check: task.check,
        });
    }
    if fixtures.is_empty() {
        bail!("no fixtures found under {}", fixtures_root.display());
    }
    Ok(fixtures)
}

/// The system prompt for a dialect arm. When `use_contract` is set, Generic is driven
/// by Anvil's operational CONTRACT (the shared system map + the slim coder contract
/// from `contracts/`). With `--no-contract`, every arm gets the neutral baseline (+ the
/// dialect's addendum) — which ISOLATES the tool-surface effect from the contract
/// effect: a Generic arm run neutral is "slim tools + neutral prompt", directly
/// comparable to the contract arm to see which variable moved the score.
fn dialect_system(dialect: Dialect, root: &Path, use_contract: bool) -> String {
    if use_contract {
        if let Dialect::Generic = dialect {
            let map = std::fs::read_to_string(root.join("contracts").join("system_map.md"))
                .unwrap_or_default();
            let contract =
                std::fs::read_to_string(root.join("contracts").join("coder_local_base.md"))
                    .unwrap_or_default();
            if !contract.trim().is_empty() {
                // The map is "prepended to" the contract, per the contract's own pointer.
                return format!("{}\n\n{}", map.trim(), contract.trim());
            }
            // Contract files missing (running outside the source tree) — fall through
            // to the baseline so the sweep still produces numbers.
        }
    }
    let add = dialect.prompt_addendum();
    if add.is_empty() {
        BENCH_SYSTEM.to_string()
    } else {
        format!("{}\n\n{}", BENCH_SYSTEM, add)
    }
}

/// Actually run a LIVE fixture's `check` command in the scratch tree and return its
/// real combined output + exit status (capped). The command is FIXTURE-defined, never
/// the model's arbitrary input, so this is safe — the model can only ever trigger this
/// one predefined check.
fn run_check(check_cmd: &str, scratch: &Path) -> String {
    let output = if cfg!(windows) {
        std::process::Command::new("cmd")
            .args(["/C", check_cmd])
            .current_dir(scratch)
            .output()
    } else {
        std::process::Command::new("sh")
            .args(["-c", check_cmd])
            .current_dir(scratch)
            .output()
    };
    match output {
        Ok(o) => {
            let code = o.status.code().unwrap_or(-1);
            let mut s = format!("exit status: {code}\n");
            let out = String::from_utf8_lossy(&o.stdout);
            let err = String::from_utf8_lossy(&o.stderr);
            if !out.trim().is_empty() {
                s.push_str(out.trim_end());
                s.push('\n');
            }
            if !err.trim().is_empty() {
                s.push_str("[stderr] ");
                s.push_str(err.trim_end());
                s.push('\n');
            }
            s.chars().take(4000).collect()
        }
        Err(e) => format!("ERROR: could not run check `{check_cmd}`: {e}"),
    }
}

/// A compact one-line summary of a tool call for the `--trace` view — shows the args
/// that matter (which edit tool, which path, the old→new snippet), so two arms' edit
/// strategies are directly comparable.
fn summarize_call(call: &ToolCall) -> String {
    let a = &call.arguments;
    let s = |k: &str| a.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let cap = |x: &str| -> String {
        let t: String = x.chars().take(50).collect();
        format!("{:?}", t) // escape newlines → one line
    };
    match call.name.as_str() {
        "edit_file" => format!(
            "edit_file {} old={} new={}",
            s("path"),
            cap(&s("old_string")),
            cap(&s("new_string"))
        ),
        "write_file" => format!("write_file {} content={}", s("path"), cap(&s("content"))),
        "read_file" => format!("read_file {}", s("path")),
        "run_command" => format!("run_command {}", cap(&s("command"))),
        other => format!("{} {}", other, cap(&a.to_string())),
    }
}

/// Describe WHY a run failed: for a live fixture, the check's failure output; for a
/// static one, each file that differs (got vs want) plus any stray files the model
/// created. Newlines are shown escaped so each file is one readable line.
fn describe_failure(fixture: &Fixture, scratch: &Path) -> String {
    let cap = |s: &str| -> String {
        let one: String = s.chars().take(120).collect();
        format!("{:?}", one) // {:?} escapes newlines → one line
    };
    if let Some(cmd) = &fixture.check {
        let out = run_check(cmd, scratch);
        return format!("[{}] check failed:\n    {}", fixture.id, out.trim());
    }
    let want = collect_files(&fixture.dir.join("after"));
    let got = collect_files(scratch);
    let mut s = format!("[{}]", fixture.id);
    for (path, w) in &want {
        match got.get(path) {
            Some(g) if g == w => {}
            Some(g) => s.push_str(&format!("\n    {path}: got {} want {}", cap(g), cap(w))),
            None => s.push_str(&format!("\n    {path}: MISSING, want {}", cap(w))),
        }
    }
    for (path, g) in &got {
        if !want.contains_key(path) {
            s.push_str(&format!("\n    +{path} (unexpected): {}", cap(g)));
        }
    }
    s
}

/// True if a live check's output reports a passing (exit 0) run.
fn check_passed(output: &str) -> bool {
    output.starts_with("exit status: 0")
}

/// True if every file in `after/` (the PROTECTED set for a live fixture — e.g. the
/// test) is present and byte-identical in the scratch tree. Doesn't require the
/// scratch to contain ONLY those files, so the edited code is unconstrained.
fn protected_files_intact(after: &Path, scratch: &Path) -> bool {
    let want = collect_files(after);
    let have = collect_files(scratch);
    want.iter().all(|(k, v)| have.get(k) == Some(v))
}

/// Execute a tool call against the scratch tree. `delegate` is disabled (the bench
/// scores edit fidelity in a sandbox). `run_command` is STUBBED with a benign pass:
/// the fixtures have no real build, but our contract tells the coder to verify with
/// run_command — a passing stub lets a contract-following model terminate cleanly.
/// It cannot inflate the score: correctness is judged directly against `after/`.
fn bench_execute(call: &ToolCall, scratch: &Path) -> String {
    match call.name.as_str() {
        "run_command" => "exit status: 0\n[bench sandbox] commands are not executed here; your edit is scored directly against the expected result.".to_string(),
        "delegate" => "ERROR: delegate is disabled in the dialect benchmark".to_string(),
        _ => crate::tools::execute(call, scratch),
    }
}

fn is_edit_tool(name: &str) -> bool {
    matches!(
        name,
        "write_file" | "edit_file" | "apply_patch" | "insert_lines"
    )
}

// ── model-driven sweep (network) ─────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn run_one(
    client: &LlmClient,
    conn: &crate::config::ProviderConnection,
    model: &str,
    api_key: &str,
    dialect: Dialect,
    fixture: &Fixture,
    scratch: &Path,
    expects_change: bool,
    root: &Path,
    use_contract: bool,
) -> RunOutcome {
    // Apply the dialect at the BENCH layer — production `chat_turn_stream` is left
    // untouched. The advertised tool surface and the per-dialect system prompt are
    // what differ between arms (Generic is driven by our operational contract, unless
    // --no-contract isolates the tool surface with the neutral prompt).
    let advertised = dialect.advertise(&crate::tools::tool_defs());
    let system = dialect_system(dialect, root, use_contract);
    let mut history = vec![ChatMessage::user(fixture.instruction.clone())];
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let mut outcome = RunOutcome::default();

    for step in 0..MAX_STEPS {
        let turn = match client
            .chat_turn_stream(
                conn,
                model,
                api_key,
                &system,
                &history,
                &advertised,
                tx.clone(),
            )
            .await
        {
            Ok(t) => t,
            Err(e) => {
                outcome.errored = true;
                outcome.error = Some(e.to_string());
                break;
            }
        };
        if step == 0 {
            outcome.first_call_valid = !turn.tool_calls.is_empty();
            if turn.tool_calls.is_empty() {
                let t = turn.text.trim();
                if !t.is_empty() {
                    outcome.no_call_text = Some(t.chars().take(400).collect());
                }
            }
        }
        history.push(ChatMessage::assistant(
            turn.text.clone(),
            turn.tool_calls.clone(),
        ));
        if turn.tool_calls.is_empty() {
            break;
        }
        for call in &turn.tool_calls {
            // Normalize the model-emitted call to canonical before executing
            // (identity for Codex/Generic today; the Anthropic native arm will map).
            let canonical = dialect.to_canonical(call.clone());
            // On a LIVE fixture, a run_command means "verify" — actually run the
            // fixture's check and hand back the real pass/fail, so the model can see
            // its mistake and fix it (the whole point of the multi-step fixtures).
            let result = match (&fixture.check, canonical.name.as_str()) {
                (Some(cmd), "run_command") => run_check(cmd, scratch),
                _ => bench_execute(&canonical, scratch),
            };
            let is_err = result.starts_with("ERROR");
            outcome.trace.push(format!(
                "{}{}",
                summarize_call(&canonical),
                if is_err { "  -> ERROR" } else { "" }
            ));
            outcome.tool_events.push((canonical.name.clone(), is_err));
            if is_edit_tool(&canonical.name) && !is_err {
                outcome.edit_landed = true;
            }
            history.push(ChatMessage::tool_result(call.id.clone(), result));
        }
        outcome.steps = step + 1;
    }

    if !outcome.errored {
        outcome.correct = match &fixture.check {
            // LIVE fixture: the test is the oracle. Correct = the check PASSES on the
            // final tree AND the protected files (everything in after/, i.e. the test)
            // are byte-intact — so a model can't "pass" by editing the test away. The
            // edited code's exact text is irrelevant; the test judges it.
            Some(cmd) => {
                check_passed(&run_check(cmd, scratch))
                    && protected_files_intact(&fixture.dir.join("after"), scratch)
            }
            // Static fixture: exact match to after/, AND (when a change is expected) an
            // edit actually landed — so "did nothing" can't pass a no-op-looking diff.
            None => {
                dirs_equal(scratch, &fixture.dir.join("after"))
                    && (outcome.edit_landed || !expects_change)
            }
        };
        if !outcome.correct {
            outcome.fail_detail = Some(describe_failure(fixture, scratch));
        }
    }
    outcome
}

/// Create a fresh temp scratch dir seeded with the fixture's `before/` tree. The
/// returned `TempDir` auto-removes on drop; a unique per-run path avoids the
/// Windows file-lock crash of removing and reusing one shared scratch dir.
fn make_scratch(fixture: &Fixture) -> Result<tempfile::TempDir> {
    let scratch = tempfile::Builder::new()
        .prefix("anvil-bench-")
        .tempdir()
        .context("creating scratch dir")?;
    copy_dir(&fixture.dir.join("before"), scratch.path())?;
    Ok(scratch)
}

/// Run the benchmark: sweep `dialects` over every fixture for the resolved
/// model, `runs` times per cell, and print a fidelity report.
pub fn run_bench(
    root: &Path,
    runs: usize,
    dialects: &[Dialect],
    binding_key: Option<&str>,
    delay_ms: u64,
    use_contract: bool,
    fixture_filter: Option<&str>,
    trace: bool,
) -> Result<()> {
    load_local_env(root);
    let cfg = load_config(root)?;
    let client = LlmClient::new();

    // Resolve the target. Normally a role keyword or binding name; but a
    // `<provider>/<model>` spec (e.g. `local-ollama/gemma4:e2b`) targets a raw model
    // directly, so any pulled model can be benched without first configuring a binding.
    let key = binding_key.unwrap_or("coder");
    let (label, model, provider, api_key) = match key.split_once('/') {
        Some((prov, raw_model)) if cfg.providers.contains_key(prov) => {
            let provider = &cfg.providers[prov];
            let api_key = client.get_credential(prov, provider)?;
            (key.to_string(), raw_model.to_string(), provider, api_key)
        }
        _ => {
            let (binding_name, binding, provider) =
                cfg.resolve_role_or_binding(key).map_err(|_| {
                    anyhow!(
                        "'{}' is not a configured role/binding (or a <provider>/<model> spec)",
                        key
                    )
                })?;
            let api_key = client.get_credential(&binding.provider, provider)?;
            (
                binding_name.to_string(),
                binding.model.clone(),
                provider,
                api_key,
            )
        }
    };

    let mut fixtures = load_fixtures(&root.join("bench").join("fixtures"))?;
    if let Some(only) = fixture_filter {
        fixtures.retain(|f| f.id == only);
        if fixtures.is_empty() {
            bail!("no fixture named '{}' under bench/fixtures/", only);
        }
    }

    println!(
        "Dialect benchmark — target '{}' (model {}), {} run(s)/cell\nDialects: {}\nContract: {}\n",
        label,
        model,
        runs,
        dialects
            .iter()
            .map(|d| format!("{:?}", d))
            .collect::<Vec<_>>()
            .join(", "),
        if use_contract {
            "ON (Generic arm uses the operational contract)"
        } else {
            "OFF (--no-contract: all arms use the neutral baseline — isolates the tool surface)"
        }
    );

    // results[fixture_index][dialect_index] = Vec<RunOutcome>
    let mut results: Vec<Vec<Vec<RunOutcome>>> = Vec::new();

    LlmClient::block_on(async {
        for fixture in &fixtures {
            // A fixture "expects a change" unless before/ already equals after/.
            // Used to reject false positives where a model does nothing.
            let expects_change =
                !dirs_equal(&fixture.dir.join("before"), &fixture.dir.join("after"));
            let mut per_dialect = Vec::new();
            for &dialect in dialects {
                let mut outcomes = Vec::new();
                for _ in 0..runs {
                    let scratch = make_scratch(fixture)?;
                    outcomes.push(
                        run_one(
                            &client,
                            provider,
                            &model,
                            &api_key,
                            dialect,
                            fixture,
                            scratch.path(),
                            expects_change,
                            root,
                            use_contract,
                        )
                        .await,
                    );
                    if delay_ms > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                }
                per_dialect.push(outcomes);
            }
            results.push(per_dialect);
        }
        Ok::<_, anyhow::Error>(())
    })?;

    print_report(&fixtures, dialects, &results);

    // Surface why runs errored — otherwise an all-errored sweep is opaque (the
    // very failure mode v0.5.5 bug #1 was about). Show each distinct first line.
    let mut seen = std::collections::BTreeSet::new();
    for cells in &results {
        for outcomes in cells {
            for o in outcomes {
                if let Some(e) = &o.error {
                    // Flatten whitespace/newlines so a pretty-printed JSON error
                    // body shows on one readable line.
                    let flat: String = e.split_whitespace().collect::<Vec<_>>().join(" ");
                    seen.insert(flat.chars().take(400).collect::<String>());
                }
            }
        }
    }
    if !seen.is_empty() {
        println!(
            "\n{} distinct error(s) (check the binding's provider/credential/network):",
            seen.len()
        );
        for line in &seen {
            println!("  - {}", line);
        }
    }

    // Per-tool usage across the whole sweep — answers "which tools does this model
    // struggle with?" (calls attempted vs. ones that returned an ERROR result).
    let mut tool_stats: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for cells in &results {
        for outcomes in cells {
            for o in outcomes {
                for (name, is_err) in &o.tool_events {
                    let e = tool_stats.entry(name.clone()).or_insert((0, 0));
                    e.0 += 1;
                    if *is_err {
                        e.1 += 1;
                    }
                }
            }
        }
    }
    if tool_stats.is_empty() {
        println!("\nTool usage: none — the model made no tool calls at all.");
    } else {
        println!("\nTool usage (calls / failed):");
        for (name, (calls, errs)) in &tool_stats {
            println!("  {:<16} {:>4} calls, {:>3} failed", name, calls, errs);
        }
    }

    // When a model answered in prose instead of calling a tool, show what it said —
    // a "coder" that never tool-calls is producing text, not driving the repo.
    let mut samples = std::collections::BTreeSet::new();
    for cells in &results {
        for outcomes in cells {
            for o in outcomes {
                if let Some(t) = &o.no_call_text {
                    let flat: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !flat.is_empty() {
                        samples.insert(flat.chars().take(300).collect::<String>());
                    }
                }
            }
        }
    }
    if !samples.is_empty() {
        println!("\nNO tool call on turn 1 — sample of what the model said instead:");
        for s in samples.iter().take(3) {
            println!("  - {}", s);
        }
    }

    // Why did failing runs fail? Concrete got-vs-want samples — the fastest way to
    // spot a SYSTEMATIC corruption (e.g. a contract that breaks a trivial insert).
    let mut fails: Vec<String> = Vec::new();
    for cells in &results {
        for outcomes in cells {
            for o in outcomes {
                if let Some(d) = &o.fail_detail {
                    if !fails.contains(d) {
                        fails.push(d.clone());
                    }
                }
            }
        }
    }
    if !fails.is_empty() {
        println!("\nFAILURES — what the model produced vs expected (deduped):");
        for d in fails.iter().take(8) {
            println!("  {}", d);
        }
    }

    // --trace: the full tool-call sequence per run, so two arms' strategies on the
    // same fixture are directly comparable (e.g. write_file rewrite vs edit_file snippet).
    if trace {
        println!("\nTRACE — tool calls per run:");
        for (fi, fixture) in fixtures.iter().enumerate() {
            for (di, dialect) in dialects.iter().enumerate() {
                for (ri, o) in results[fi][di].iter().enumerate() {
                    let status = if o.errored {
                        "ERRORED"
                    } else if o.correct {
                        "ok"
                    } else {
                        "FAIL"
                    };
                    println!(
                        "  [{} / {:?} / run {}] {}",
                        fixture.id,
                        dialect,
                        ri + 1,
                        status
                    );
                    if o.trace.is_empty() {
                        println!("      (no tool calls)");
                    }
                    for line in &o.trace {
                        println!("      {}", line);
                    }
                }
            }
        }
    }
    Ok(())
}

fn print_report(fixtures: &[Fixture], dialects: &[Dialect], results: &[Vec<Vec<RunOutcome>>]) {
    let dialect_cols: Vec<String> = dialects.iter().map(|d| format!("{:?}", d)).collect();
    let id_w = fixtures
        .iter()
        .map(|f| f.id.len())
        .max()
        .unwrap_or(8)
        .max(8);
    let type_w = fixtures
        .iter()
        .map(|f| f.edit_type.len())
        .max()
        .unwrap_or(9)
        .max(9);

    // Header.
    print!("{:<id_w$}  {:<type_w$}", "fixture", "edit_type");
    for col in &dialect_cols {
        print!("  {:>10}", col);
    }
    println!();

    // Per-fixture rows (correct/runs).
    for (fi, fixture) in fixtures.iter().enumerate() {
        print!("{:<id_w$}  {:<type_w$}", fixture.id, fixture.edit_type);
        for (di, _) in dialects.iter().enumerate() {
            let outcomes = &results[fi][di];
            let runs = outcomes.len();
            let correct = outcomes.iter().filter(|o| o.correct).count();
            let errored = outcomes.iter().filter(|o| o.errored).count();
            let cell = if errored > 0 {
                format!("{}/{}!{}", correct, runs, errored)
            } else {
                format!("{}/{}", correct, runs)
            };
            print!("  {:>10}", cell);
        }
        println!();
    }

    // Totals.
    let sep_len = id_w + 2 + type_w + dialect_cols.len() * 12;
    println!("{}", "-".repeat(sep_len));
    print_total_row("TOTAL correct", id_w, type_w, dialects, results, |o| {
        o.correct
    });
    print_total_row("first-call-valid", id_w, type_w, dialects, results, |o| {
        o.first_call_valid
    });
    print_total_row("edit-landed", id_w, type_w, dialects, results, |o| {
        o.edit_landed
    });

    // Average steps row.
    print!("{:<id_w$}  {:<type_w$}", "avg-steps", "");
    for (di, _) in dialects.iter().enumerate() {
        let mut total = 0usize;
        let mut n = 0usize;
        for cells in results {
            for o in &cells[di] {
                if !o.errored {
                    total += o.steps;
                    n += 1;
                }
            }
        }
        let avg = if n > 0 {
            format!("{:.1}", total as f64 / n as f64)
        } else {
            "-".to_string()
        };
        print!("  {:>10}", avg);
    }
    println!();
    println!("\n(cell = correct/runs; `!n` = n errored runs excluded from rates)");
}

fn print_total_row(
    label: &str,
    id_w: usize,
    type_w: usize,
    dialects: &[Dialect],
    results: &[Vec<Vec<RunOutcome>>],
    pred: impl Fn(&RunOutcome) -> bool,
) {
    print!("{:<id_w$}  {:<type_w$}", label, "");
    for di in 0..dialects.len() {
        let mut hits = 0usize;
        let mut total = 0usize;
        for cells in results {
            for o in &cells[di] {
                if o.errored {
                    continue;
                }
                total += 1;
                if pred(o) {
                    hits += 1;
                }
            }
        }
        print!("  {:>10}", format!("{}/{}", hits, total));
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, body: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    #[test]
    fn copy_then_compare_roundtrips() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        write(&src.join("a.txt"), "alpha");
        write(&src.join("sub/b.txt"), "beta");

        let dst = tmp.path().join("dst");
        copy_dir(&src, &dst).unwrap();
        assert!(dirs_equal(&src, &dst), "copy should be byte-identical");

        // A content change breaks equality.
        std::fs::write(dst.join("a.txt"), "ALPHA").unwrap();
        assert!(!dirs_equal(&src, &dst));

        // A missing file breaks equality.
        let dst2 = tmp.path().join("dst2");
        copy_dir(&src, &dst2).unwrap();
        std::fs::remove_file(dst2.join("sub/b.txt")).unwrap();
        assert!(!dirs_equal(&src, &dst2));
    }

    #[test]
    fn loads_a_well_formed_fixture() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("bench/fixtures/demo");
        write(
            &root.join("task.toml"),
            "edit_type = \"single-line\"\ninstruction = \"change x to y\"\n",
        );
        write(&root.join("before/f.txt"), "x");
        write(&root.join("after/f.txt"), "y");

        let fixtures = load_fixtures(&tmp.path().join("bench/fixtures")).unwrap();
        assert_eq!(fixtures.len(), 1);
        assert_eq!(fixtures[0].id, "demo");
        assert_eq!(fixtures[0].edit_type, "single-line");
        assert!(fixtures[0].instruction.contains("change x to y"));
    }

    #[test]
    fn comparison_ignores_trailing_whitespace_and_crlf_noise() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        write(&a.join("f.txt"), "line one\nline two\n");
        // Same content, but with a trailing space on line one and CRLF endings.
        write(&b.join("f.txt"), "line one \r\nline two\r\n");
        assert!(dirs_equal(&a, &b), "trailing ws / CRLF must not matter");
        // A real content difference still fails.
        std::fs::write(b.join("f.txt"), "line ONE\nline two\n").unwrap();
        assert!(!dirs_equal(&a, &b));
    }

    #[test]
    fn live_fixture_loads_check_and_protects_test_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("bench/fixtures/live");
        write(
            &root.join("task.toml"),
            "edit_type = \"fix\"\ninstruction = \"fix it\"\ncheck = \"python -B check.py\"\n",
        );
        write(&root.join("before/check.py"), "assert False\n");
        write(&root.join("after/check.py"), "assert False\n");

        let fixtures = load_fixtures(&tmp.path().join("bench/fixtures")).unwrap();
        assert_eq!(fixtures[0].check.as_deref(), Some("python -B check.py"));

        // protected_files_intact: only the after/ files must match; extra scratch files
        // (the edited code, __pycache__) are allowed.
        let scratch = tmp.path().join("scratch");
        write(&scratch.join("check.py"), "assert False\n");
        write(&scratch.join("stats.py"), "whatever\n");
        assert!(protected_files_intact(&root.join("after"), &scratch));
        // Tampering with the protected test file is caught.
        std::fs::write(scratch.join("check.py"), "assert True\n").unwrap();
        assert!(!protected_files_intact(&root.join("after"), &scratch));
    }

    #[test]
    fn check_passed_reads_exit_status() {
        assert!(check_passed("exit status: 0\nall tests passed\n"));
        assert!(!check_passed("exit status: 1\n[stderr] AssertionError\n"));
        assert!(!check_passed("ERROR: could not run check"));
    }

    #[test]
    fn run_command_is_stubbed_and_delegate_disabled() {
        let tmp = tempfile::tempdir().unwrap();
        // run_command is a benign passing stub (not an error), so a contract-driven
        // model that verifies can terminate cleanly.
        let rc = ToolCall {
            id: "1".into(),
            name: "run_command".into(),
            arguments: serde_json::json!({"command": "echo hi"}),
        };
        let out = bench_execute(&rc, tmp.path());
        assert!(
            !out.starts_with("ERROR"),
            "run_command should be a stub: {out}"
        );
        assert!(out.contains("exit status: 0"), "{out}");
        // delegate stays disabled.
        let dg = ToolCall {
            id: "2".into(),
            name: "delegate".into(),
            arguments: serde_json::json!({}),
        };
        assert!(bench_execute(&dg, tmp.path()).starts_with("ERROR"));
    }
}
