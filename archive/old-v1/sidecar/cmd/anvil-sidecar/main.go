package main

import (
	"flag"
	"fmt"
	"log/slog"
	"net"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"
	"time"

	"github.com/ai-nhancement/Anvil/sidecar/internal/config"
	"github.com/ai-nhancement/Anvil/sidecar/internal/daemon"
	"github.com/ai-nhancement/Anvil/sidecar/internal/server"
)

const (
	version    = "0.1.0"
	binaryName = "anvil-sidecar"
)

func main() {
	showVersion := flag.Bool("version", false, "print version and exit")
	port := flag.Int("port", 0, "gRPC listen port (0 = OS-assigned)")
	workspace := flag.String("workspace", ".", "workspace root directory; PID/port files written under {workspace}/.anvil/run/")
	configPath := flag.String("config", "", "path to sidecar config JSON file (required)")
	idleTimeout := flag.Duration("idle-timeout", 0, "shut down after this period of inactivity (0 = never)")
	logLevel := flag.String("log-level", "info", "log level: debug, info, warn, error")
	flag.Parse()

	if *showVersion {
		fmt.Printf("%s %s\n", binaryName, version)
		os.Exit(0)
	}

	var level slog.Level
	if err := level.UnmarshalText([]byte(*logLevel)); err != nil {
		level = slog.LevelInfo
	}
	logger := slog.New(slog.NewJSONHandler(os.Stderr, &slog.HandlerOptions{Level: level}))
	slog.SetDefault(logger)

	if *configPath == "" {
		slog.Error("--config is required")
		os.Exit(1)
	}

	cfg, err := config.Load(*configPath)
	if err != nil {
		slog.Error("load config", "error", err)
		os.Exit(1)
	}
	slog.Info("config loaded", "epoch", cfg.Epoch())

	// Resolve workspace to an absolute path — used as the registry key and file path prefix.
	workspaceAbs, err := filepath.Abs(*workspace)
	if err != nil {
		slog.Error("resolve workspace path", "error", err)
		os.Exit(1)
	}

	// Workspace-scoped runtime files.
	anvilRunDir := filepath.Join(workspaceAbs, ".anvil", "run")
	pidPath := filepath.Join(anvilRunDir, "sidecar.pid")
	portPath := filepath.Join(anvilRunDir, "sidecar.port")

	// Wire idle timer before building the server.
	// Use an indirect reference to break the init cycle with the gRPC server handle.
	var gsRef interface{ GracefulStop() }
	idleTimer := daemon.NewIdleTimer(*idleTimeout, func() {
		slog.Info("idle timeout reached, shutting down")
		if gsRef != nil {
			gsRef.GracefulStop()
		}
	})

	srv := server.New(cfg, *configPath, version, idleTimer.Touch)
	gs := server.NewGRPCServer(srv)
	gsRef = gs

	lis, err := net.Listen("tcp", fmt.Sprintf("127.0.0.1:%d", *port))
	if err != nil {
		slog.Error("listen", "error", err)
		os.Exit(1)
	}
	actualPort := lis.Addr().(*net.TCPAddr).Port
	slog.Info("listening", "port", actualPort)

	// Write workspace-scoped PID and port files.
	if err := daemon.WritePID(pidPath); err != nil {
		slog.Warn("write PID file", "path", pidPath, "error", err)
	}
	if err := daemon.WritePort(portPath, actualPort); err != nil {
		slog.Warn("write port file", "path", portPath, "error", err)
	}
	defer daemon.Remove(pidPath)  //nolint:errcheck
	defer daemon.Remove(portPath) //nolint:errcheck

	// Global registry at ~/.anvil/global-registry.json.
	homeDir, err := os.UserHomeDir()
	if err != nil {
		slog.Warn("cannot determine home dir", "error", err)
		homeDir = "."
	}
	regPath := filepath.Join(homeDir, ".anvil", "global-registry.json")
	reg, regErr := daemon.NewRegistry(regPath)
	var stopHeartbeat chan<- struct{}
	if regErr != nil {
		slog.Warn("open global registry", "error", regErr)
	} else {
		pid := os.Getpid()
		now := time.Now().UTC().Format(time.RFC3339)
		entry := daemon.RegistryEntry{
			PID:           pid,
			Port:          actualPort,
			ConfigEpoch:   cfg.Epoch(),
			ConfigPath:    *configPath,
			WorkspacePath: workspaceAbs,
			StartedAt:     now,
			LastSeenAt:    now,
		}
		if err := reg.Register(workspaceAbs, entry); err != nil {
			slog.Warn("register in global registry", "error", err)
		} else {
			stopHeartbeat = daemon.StartHeartbeat(workspaceAbs, reg, 60*time.Second)
		}
		defer func() {
			if stopHeartbeat != nil {
				close(stopHeartbeat)
			}
			if err := reg.Unregister(workspaceAbs); err != nil {
				slog.Warn("unregister from global registry", "error", err)
			}
		}()
	}

	// Signal handling.
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, os.Interrupt, syscall.SIGTERM)
	go func() {
		sig := <-sigCh
		slog.Info("signal received, shutting down", "signal", sig)
		idleTimer.Stop()
		gs.GracefulStop()
	}()

	slog.Info("server starting", "version", version, "workspace", workspaceAbs)
	if err := gs.Serve(lis); err != nil {
		slog.Error("serve", "error", err)
		os.Exit(1)
	}
	slog.Info("server stopped")
}
