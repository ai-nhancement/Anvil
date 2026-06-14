package config_test

import (
	"testing"

	"github.com/ai-nhancement/Anvil/sidecar/internal/config"
)

// hinge_test: pins=config-epoch-sha256-algorithm, intended=epoch-computation-stability, phase=P3c
func TestConfigEpochComputation(t *testing.T) {
	// Pins the exact SHA-256 hex value for a known JSON input.
	// If the epoch algorithm changes (normalization, encoding, etc.) this test breaks,
	// signaling that Vault and sidecar must be updated together to stay in sync.
	const input = `{"version":1,"connections":[]}`
	const wantEpoch = "978741969065f5be40b642a7a4eba801218a64516989f30e16a7f1c28f257138"

	cfg, err := config.ParseBytes([]byte(input))
	if err != nil {
		t.Fatalf("ParseBytes: %v", err)
	}
	if got := cfg.Epoch(); got != wantEpoch {
		t.Errorf("Epoch() = %q, want %q", got, wantEpoch)
	}
}

// hinge_test: pins=config-epoch-format-hex-lowercase, intended=epoch-is-lowercase-hex, phase=P3c
func TestConfigEpochIsLowercaseHex(t *testing.T) {
	// Epoch must be lowercase hex — the Vault comparison is case-sensitive.
	cfg, err := config.ParseBytes([]byte(`{"version":1,"connections":[]}`))
	if err != nil {
		t.Fatalf("ParseBytes: %v", err)
	}
	epoch := cfg.Epoch()
	if len(epoch) != 64 {
		t.Errorf("epoch length: got %d, want 64", len(epoch))
	}
	for i, c := range epoch {
		if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')) {
			t.Errorf("epoch[%d] = %q: not lowercase hex", i, c)
		}
	}
}
