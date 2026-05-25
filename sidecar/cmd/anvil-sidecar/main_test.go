package main

import (
	"fmt"
	"runtime"
	"strings"
	"testing"
)

// hinge_test: pins=1.22, intended=go-stable-floor, phase=P0
func TestGoToolchainVersionFloor(t *testing.T) {
	// Pins: Go ≥1.22. Enforced at build time via go.mod.
	// Flipping requires updating go.mod and this annotation together.
	goVersion := runtime.Version() // e.g. "go1.26.3"
	var major, minor int
	if _, err := fmt.Sscanf(strings.TrimPrefix(goVersion, "go"), "%d.%d", &major, &minor); err != nil {
		t.Fatalf("could not parse Go version %q: %v", goVersion, err)
	}
	const floorMajor, floorMinor = 1, 22
	if major < floorMajor || (major == floorMajor && minor < floorMinor) {
		t.Errorf("Go version floor is %d.%d; got %s", floorMajor, floorMinor, goVersion)
	}
}

// hinge_test: pins=anvil-sidecar, intended=binary-entry-point, phase=P0
func TestSidecarEntryPointExists(t *testing.T) {
	// Pins: the sidecar binary is named "anvil-sidecar" and lives in cmd/anvil-sidecar.
	// Flipping requires changing the binaryName constant and the module layout.
	if binaryName != "anvil-sidecar" {
		t.Errorf("expected binary name anvil-sidecar; got %s", binaryName)
	}
}
