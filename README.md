# Anvil

**Structure for vibe coding.** Anvil is a human-gated workflow tool for AI-assisted development: adversarial cross-vendor review at every gate, provenance on every artifact, and explicit workflow discipline replacing unstructured agent loops.

> **Status:** Pre-release (P0 Bootstrap). Repository is private through v1 implementation.

## What it is

Anvil enforces a review discipline — Discuss → Plan → Build → Ship — where every artifact is reviewed by models from different families before it advances. The workflow is the product.

See the [Charter](Anvil%20Plan/new_project_charter.md) and [Plan](Anvil%20Plan/ANVIL_PLAN.md) for the full design.

## Prerequisites

- Rust stable ≥1.80 (via [rustup](https://rustup.rs))
- Go ≥1.22
- [protoc](https://github.com/protocolbuffers/protobuf/releases) (Protocol Buffers compiler)
- [just](https://github.com/casey/just) (command runner)

## Build

```sh
just build       # build anvil + anvil-sidecar binaries
just test        # run Rust + Go test suites
just gen         # regenerate protobuf bindings
just lint        # clippy + golangci-lint
just fmt         # format all code
```

## Version

```sh
anvil --version
anvil-sidecar --version
```

## License

Apache 2.0 — see [LICENSE](LICENSE).
