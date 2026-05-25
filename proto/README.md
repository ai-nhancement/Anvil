# Anvil Protobuf Schema

This directory contains the versioned protobuf contracts between the Vault (Rust) and the sidecar (Go).

## Current version: `anvil.v1`

- `anvil/v1/sidecar.proto` — `Sidecar` service contract

## Schema versioning policy

- The package is `anvil.v1`. Breaking changes require bumping to `anvil.v2` and a new directory.
- Non-breaking additions (new fields with default values, new RPC methods) are permitted within a version.
- The version handshake RPC (`Handshake`) must be the first RPC called on every connection; the sidecar rejects connections whose `core_protocol_version` has no overlap with its `supported_versions`.
- `just gen` regenerates both Rust and Go bindings from the proto sources. Never edit generated files directly.

## P0 state

The P0 proto contains a single placeholder `Ping` RPC. The full `Sidecar` service — `Handshake`, `Invoke`, `InvokeStreaming`, `Cancel`, `Health`, `ReloadConfig` — is defined in P3a.
