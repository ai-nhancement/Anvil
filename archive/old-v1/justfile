# Anvil build orchestration
# Requires: rust (cargo), go, protoc, protoc-gen-go, protoc-gen-go-grpc, golangci-lint, just

# Use Git's sh on Windows; sh on Linux/macOS.
set windows-shell := ["C:/Program Files/Git/bin/sh.exe", "-cu"]

# List available recipes
default:
    @just --list

# Build both Rust workspace and Go sidecar
build:
    cargo build --workspace
    cd sidecar && go build ./...

# Run all tests (Rust + Go)
test:
    cargo test --workspace
    cd sidecar && go test ./...

# Generate Go protobuf bindings via protoc
gen-go:
    protoc \
        -I proto \
        --go_out=sidecar \
        --go_opt=module=github.com/ai-nhancement/Anvil/sidecar \
        --go-grpc_out=sidecar \
        --go-grpc_opt=module=github.com/ai-nhancement/Anvil/sidecar \
        proto/anvil/v1/sidecar.proto

# Generate Rust protobuf bindings via cargo build (requires protoc; commits src/gen/anvil.v1.rs)
gen-rust:
    ANVIL_REGEN_PROTO=1 cargo build -p anvil-sidecar-client --quiet

# Generate all protobuf bindings (Go + Rust)
gen: gen-go gen-rust

# Lint: clippy (deny warnings) + golangci-lint
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cd sidecar && golangci-lint run ./...

# Format: rustfmt + gofmt
fmt:
    cargo fmt --all
    cd sidecar && gofmt -w .

# Check formatting without modifying files (exits non-zero if anything is unformatted)
fmt-check:
    cargo fmt --all -- --check
    cd sidecar && test -z "$(gofmt -l .)"

# Launch the sidecar in dev mode (prints version and exits in P0)
dev-sidecar:
    cd sidecar && go run ./cmd/anvil-sidecar --version
