# Tool-dialect benchmark fixtures

Each fixture is a deterministic edit task with a known-good result, used to measure
**tool-use fidelity per `model × dialect`** — not coding ability. See
`docs/ROADMAP_tool_dialect_bench.md` for the methodology and
`docs/PLAN_dialects_build.md` (Phase 2).

Run the sweep from the Anvil source tree (this is a dev/eval tool):

```
anvil bench                          # coder binding, dialects codex+generic, 3 runs/cell
anvil bench --runs 5 --dialects codex,generic
anvil bench --model some-binding-name
```

The runner copies each fixture's `before/` into a scratch dir, drives the resolved
model under each dialect to follow `instruction`, executes the model's tool calls
against the scratch copy, and scores the result against `after/`.

## Layout

```
<id>/
  task.toml      # edit_type = "<tag>"  +  instruction = "<dialect-neutral task>"
  before/        # input tree (copied to scratch per run)
  after/         # the one correct result the scratch tree is compared against
```

`instruction` must be **dialect-neutral** — describe the change, never the tool
("change X to Y", not "apply a patch" / "str_replace"). Only the advertised tools
differ between arms.

## Scoring notes

- **Comparison normalizes line endings (CRLF→LF) and trailing whitespace**, so a
  model's LF output matches a fixture git checked out as CRLF on Windows. Internal
  indentation is preserved (that's the point of `tricky-whitespace`).
- A cell reads `correct/runs`; `!n` flags `n` errored (network) runs, excluded
  from the rates.

## Corpus

| id | edit_type | exercises |
|---|---|---|
| `single-line-change` | single-line | minimal in-place edit |
| `multi-hunk` | multi-hunk | two changes in one file (one apply_patch vs two edit_file calls) |
| `add-file` | add-file | `write_file` with given multi-line content |
| `insert-middle` | insert-middle | inserting a line between two others |
| `tricky-whitespace` | tricky-whitespace | exact-match on an indented line (apply_patch's classic failure mode) |
| `large-file-targeted` | large-file-targeted | locating one line in a larger file, with a decoy (`total` used elsewhere) |

**Deliberately omitted: `delete-file` and `rename-file`.** The canonical tool set
has no delete/rename op except `apply_patch`'s `Delete File`, so the `Generic`
dialect (no apply_patch) cannot satisfy them — including those cases would score a
*capability gap*, not tool-fit, and skew the Codex-vs-Generic comparison. Add them
once a canonical delete/rename op exists (or restrict them to apply_patch-capable
dialects).
