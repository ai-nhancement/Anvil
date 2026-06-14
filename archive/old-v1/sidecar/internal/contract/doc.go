// Package contract contains protobuf/gRPC bindings for the Sidecar service.
// Regenerate bootstrap files with: just gen-go (requires protoc).
package contract

// ProtoPackageName is the canonical protobuf package name of the anvil.v1 contract.
// Handshake version strings are "vN" (e.g. "v1"), not this package name.
// This constant lives in a non-generated file so it survives `just gen-go` regeneration.
const ProtoPackageName = "anvil.v1"
