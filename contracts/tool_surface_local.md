LOCAL TOOL SURFACE (v0 — the floor)

The slimmest tool set + bare schema descriptions for local-model coders. The native
function-calling schema owns only the SIGNATURE (name + params); the contract and the
system map own the DISCIPLINE (ordering, budgets, read-before-edit, verify-before-done).
No advice is stated twice. We add back description text or tools ONLY when a bench
failure demands it.

CORE TOOLS (7) — what a local coder gets at the floor:

- read_file(path, [offset], [limit])   "Read a text file, or a line range with offset+limit."
- list_dir(path)                        "List a directory's entries."
- grep(pattern, [path])                 "Find a literal substring; returns path:line: text."
- edit_file(path, old_string, new_string)  "Replace an exact, unique snippet."
- write_file(path, content)             "Create or overwrite a file."
- run_command(command)                  "Run a shell command from the project root; returns output + exit code."
- project_state()                       "Live workflow stage, phase, plan slice, and git status."

Param descriptions stay minimal too — `path` is "relative to the project root"; the rest
are self-evident from the name and need no gloss at the floor.

DROPPED FROM THE LOCAL SURFACE (add back only if the bench shows it's needed AND wielded well):

- apply_patch  — the Codex patch DSL; small models aren't trained on it. edit_file is the edit path.
- delegate     — sub-agent orchestration; too much surface for a v0 local coder.
- flag_risk    — nice-to-have signalling; not load-bearing for the core loop.

WHAT WAS CUT FROM THE FAT (canonical) DESCRIPTIONS, AND WHERE IT LIVES NOW:

- read_file's "for large files, grep then offset+limit"  -> system_map METHOD + FIND-DON'T-FLAIL clause.
- edit_file's "targeted edits, not whole-file rewrites" + exact/unique match  -> EDIT DISCIPLINE clause.
- run_command's "reserve for build/test/lint"  -> VERIFY-PRECEDENCE clause + the map.
- apply_patch's full format spec  -> gone with the tool.

This is what dialect Generic should advertise() for the local path: the canonical tools,
filtered to these 7, with descriptions slimmed to the lines above. The fat descriptions
remain for frontier dialects (Codex/Anthropic), which don't read a contract.
