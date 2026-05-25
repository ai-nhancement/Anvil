// Package daemon manages sidecar process lifecycle: PID/port files, global registry, idle tracking.
package daemon

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// WritePID writes the current process PID to path.
func WritePID(path string) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return fmt.Errorf("mkdir: %w", err)
	}
	return os.WriteFile(path, fmt.Appendf(nil, "%d", os.Getpid()), 0o600)
}

// WritePort writes port to path.
func WritePort(path string, port int) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return fmt.Errorf("mkdir: %w", err)
	}
	return os.WriteFile(path, fmt.Appendf(nil, "%d", port), 0o600)
}

// Remove deletes path, ignoring ENOENT.
func Remove(path string) error {
	err := os.Remove(path)
	if os.IsNotExist(err) {
		return nil
	}
	return err
}

// RegistryEntry describes one sidecar instance in the global registry.
type RegistryEntry struct {
	PID           int    `json:"pid"`
	Port          int    `json:"port"`
	ConfigEpoch   string `json:"config_epoch"`
	ConfigPath    string `json:"config_path"`
	WorkspacePath string `json:"workspace_path"`
	StartedAt     string `json:"started_at"`
	LastSeenAt    string `json:"last_seen_at"`
}

// Registry is a JSON-backed map of key → RegistryEntry.
// It is safe for concurrent use; mutations write atomically via rename.
type Registry struct {
	mu   sync.Mutex
	path string
}

// NewRegistry opens (or creates) the global registry at path.
func NewRegistry(path string) (*Registry, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return nil, fmt.Errorf("registry dir: %w", err)
	}
	return &Registry{path: path}, nil
}

// Register adds or updates the entry for key (typically the workspace path).
func (r *Registry) Register(key string, entry RegistryEntry) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	m := r.load()
	m[key] = entry
	return r.save(m)
}

// UpdateLastSeen refreshes the last_seen_at timestamp for key.
func (r *Registry) UpdateLastSeen(key string) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	m := r.load()
	entry, ok := m[key]
	if !ok {
		return nil
	}
	entry.LastSeenAt = time.Now().UTC().Format(time.RFC3339)
	m[key] = entry
	return r.save(m)
}

// Unregister removes the entry for key.
func (r *Registry) Unregister(key string) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	m := r.load()
	delete(m, key)
	return r.save(m)
}

func (r *Registry) load() map[string]RegistryEntry {
	data, err := os.ReadFile(r.path)
	if err != nil {
		return make(map[string]RegistryEntry)
	}
	var m map[string]RegistryEntry
	if json.Unmarshal(data, &m) != nil {
		return make(map[string]RegistryEntry)
	}
	return m
}

func (r *Registry) save(m map[string]RegistryEntry) error {
	data, err := json.MarshalIndent(m, "", "  ")
	if err != nil {
		return fmt.Errorf("registry marshal: %w", err)
	}
	tmp := r.path + ".tmp"
	if err := os.WriteFile(tmp, data, 0o600); err != nil {
		return fmt.Errorf("registry write: %w", err)
	}
	if err := os.Rename(tmp, r.path); err != nil {
		return fmt.Errorf("registry rename: %w", err)
	}
	return nil
}

// StartHeartbeat launches a goroutine that calls reg.UpdateLastSeen(key) on every interval.
// Close the returned channel to stop the goroutine.
func StartHeartbeat(key string, reg *Registry, interval time.Duration) chan<- struct{} {
	stop := make(chan struct{})
	go func() {
		ticker := time.NewTicker(interval)
		defer ticker.Stop()
		for {
			select {
			case <-ticker.C:
				_ = reg.UpdateLastSeen(key)
			case <-stop:
				return
			}
		}
	}()
	return stop
}

// IdleTimer fires a shutdown callback after a period of inactivity.
// A zero timeout disables the timer.
type IdleTimer struct {
	mu       sync.Mutex
	timeout  time.Duration
	timer    *time.Timer
	shutdown func()
}

// NewIdleTimer creates a timer that calls shutdown after timeout of inactivity.
// A zero timeout is a no-op.
func NewIdleTimer(timeout time.Duration, shutdown func()) *IdleTimer {
	t := &IdleTimer{timeout: timeout, shutdown: shutdown}
	if timeout > 0 {
		t.timer = time.AfterFunc(timeout, t.fire)
	}
	return t
}

// Touch resets the idle countdown.
func (t *IdleTimer) Touch() {
	if t.timeout == 0 {
		return
	}
	t.mu.Lock()
	defer t.mu.Unlock()
	if t.timer != nil {
		t.timer.Reset(t.timeout)
	}
}

// Stop cancels the idle timer permanently.
func (t *IdleTimer) Stop() {
	t.mu.Lock()
	defer t.mu.Unlock()
	if t.timer != nil {
		t.timer.Stop()
	}
}

func (t *IdleTimer) fire() {
	t.shutdown()
}
