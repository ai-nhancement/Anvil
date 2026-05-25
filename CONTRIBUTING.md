# Contributing to Anvil

Thank you for contributing. Anvil is Apache 2.0 licensed and uses the Developer Certificate of Origin (DCO) for all contributions.

## Developer Certificate of Origin (DCO)

By making a contribution to this project, you certify that:

1. The contribution was created in whole or in part by you and you have the right to submit it under the Apache 2.0 license; or
2. The contribution is based upon previous work that, to the best of your knowledge, is covered under an appropriate open-source license and you have the right under that license to submit that work with modifications; or
3. The contribution was provided directly to you by some other person who certified (1), (2), or (3) and you have not modified it.

Sign off each commit with:

```
git commit -s -m "Your commit message"
```

This appends a `Signed-off-by: Your Name <your@email.com>` trailer to the commit.

## DCO Extension: AI-Assisted and Derived-From Trailers

Anvil's CI enforces two additional optional trailers for transparency:

### `AI-Assisted-By:`

Required on commits where ≥20 lines of new code were produced with AI assistance, or where any new file or code in `crates/anvil-core/`, `crates/anvil-audit/`, `crates/anvil-graph/`, `crates/anvil-sidecar-client/`, or `sidecar/internal/` was AI-assisted.

Format:
```
AI-Assisted-By: <model-name> (<tool-or-interface>)
```

Example:
```
AI-Assisted-By: Claude Sonnet 4.6 (Claude Code)
```

The trailer is informational, not a disclaimer of authorship. The committer remains the author and is responsible for the code.

### `Derived-From:`

Required when code is adapted from a third-party source (open-source snippet, Stack Overflow answer, blog post, etc.).

Format:
```
Derived-From: <URL-or-description> (license: <SPDX-identifier>)
```

Example:
```
Derived-From: https://example.com/some-snippet (license: MIT)
```

Only include snippets whose license is compatible with Apache 2.0. If in doubt, do not include the snippet.

## Commit message format

```
<type>(<scope>): <short summary>

<body — explain the WHY, not the WHAT>

Signed-off-by: Name <email>
AI-Assisted-By: ... (if applicable)
Derived-From: ... (if applicable)
```

Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `build`.

## Pull requests

- One logical change per PR.
- All tests must pass: `just test`.
- Lint must be clean: `just lint`.
- Format must be clean: `just fmt-check`.
- The PR description should explain the motivation, not restate the diff.

## Questions

Open a GitHub Discussion or file an issue at https://github.com/ai-nhancement/Anvil/issues.
