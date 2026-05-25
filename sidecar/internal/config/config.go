package config

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
)

type ProviderType string

const (
	ProviderAnthropic      ProviderType = "anthropic"
	ProviderOpenAI         ProviderType = "openai"
	ProviderGoogleAIStudio ProviderType = "google_ai_studio"
)

var defaultEndpoints = map[ProviderType]string{
	ProviderAnthropic:      "https://api.anthropic.com",
	ProviderOpenAI:         "https://api.openai.com",
	ProviderGoogleAIStudio: "https://generativelanguage.googleapis.com",
}

type ProviderConnection struct {
	ID       string       `json:"id"`
	Provider ProviderType `json:"provider"`
	Endpoint string       `json:"endpoint,omitempty"` // empty → use provider default
}

type jsonConfig struct {
	Version     int                  `json:"version"`
	Connections []ProviderConnection `json:"connections"`
}

type Config struct {
	connections []ProviderConnection
	epoch       string // hex SHA-256 of the raw config bytes
}

// Load reads a JSON config file from path.
func Load(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("config: read %q: %w", path, err)
	}
	return ParseBytes(data)
}

// ParseBytes parses JSON config bytes and computes the SHA-256 epoch.
// P4a: epoch is SHA-256 of raw bytes; both Vault and sidecar must use identical bytes.
// Format is JSON (not TOML as noted in the proto README). Canonicalize before P4a ships.
func ParseBytes(data []byte) (*Config, error) {
	var raw jsonConfig
	if err := json.Unmarshal(data, &raw); err != nil {
		return nil, fmt.Errorf("config: parse: %w", err)
	}
	sum := sha256.Sum256(data)
	return &Config{
		connections: raw.Connections,
		epoch:       fmt.Sprintf("%x", sum),
	}, nil
}

// Epoch returns the hex SHA-256 of the config bytes used to load this Config.
func (c *Config) Epoch() string { return c.epoch }

// ConnectionByID returns the ProviderConnection with the given ID, if found.
func (c *Config) ConnectionByID(id string) (*ProviderConnection, bool) {
	for i := range c.connections {
		if c.connections[i].ID == id {
			return &c.connections[i], true
		}
	}
	return nil, false
}

// ResolvedEndpoint returns the base URL for conn, applying the provider default when Endpoint is empty.
func (c *Config) ResolvedEndpoint(conn *ProviderConnection) string {
	if conn.Endpoint != "" {
		return conn.Endpoint
	}
	return defaultEndpoints[conn.Provider]
}
