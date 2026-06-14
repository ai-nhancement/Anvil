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

// hinge_test: pins=comment-parser, intended=test_hinge_comment_metadata_required, phase=P10b
func TestHingeCommentMetadataRequired(t *testing.T) {
	// Pins: a valid // hinge_test: annotation must supply all of pins, intended, and phase.
	// Flipping requires changing the annotation format and updating the Rust scanner together.
	sample := "// hinge_test: pins=v1, intended=my-hinge, phase=P5"
	fields := map[string]bool{"pins": false, "intended": false, "phase": false}
	rest, ok := strings.CutPrefix(strings.TrimSpace(sample), "// hinge_test:")
	if !ok {
		t.Fatal("sample is not a hinge_test comment")
	}
	for _, part := range strings.Split(rest, ",") {
		part = strings.TrimSpace(part)
		for key := range fields {
			if strings.HasPrefix(part, key+"=") {
				val := strings.TrimSpace(strings.TrimPrefix(part, key+"="))
				if val != "" {
					fields[key] = true
				}
			}
		}
	}
	for key, found := range fields {
		if !found {
			t.Errorf("hinge_test annotation missing required field %q", key)
		}
	}
}
