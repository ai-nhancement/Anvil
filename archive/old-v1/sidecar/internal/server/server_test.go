package server_test

import (
	"testing"

	contract "github.com/ai-nhancement/Anvil/sidecar/internal/contract"
	"github.com/ai-nhancement/Anvil/sidecar/internal/server"
)

// hinge_test: pins=AnvilServer-implements-SidecarServer, intended=server-implements-sidecar-server, phase=P3c
func TestServerImplementsSidecarServer(t *testing.T) {
	// Compile-time: AnvilServer must implement the full contract.SidecarServer interface.
	// Uses a typed nil pointer to avoid calling New — no runtime side effects.
	// If a new RPC is added to the proto, this test breaks until the server implements it.
	var _ contract.SidecarServer = (*server.AnvilServer)(nil)
}

// hinge_test: pins=serverSupportedVersions=v1-only, intended=server-protocol-version-list, phase=P3c
func TestServerSupportedVersions(t *testing.T) {
	// Verifies that the server's supported version list matches the expected constant.
	// Changing this list is a breaking protocol change requiring a coordinated Vault update.
	got := server.ServerSupportedVersions()
	if len(got) != 1 {
		t.Fatalf("expected 1 supported version, got %d: %v", len(got), got)
	}
	if got[0] != "v1" {
		t.Errorf("supported version: got %q, want %q", got[0], "v1")
	}
}
