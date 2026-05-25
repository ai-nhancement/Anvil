package server_test

import (
	"context"
	"net"
	"testing"

	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/status"
	"google.golang.org/grpc/test/bufconn"

	"github.com/ai-nhancement/Anvil/sidecar/internal/config"
	contract "github.com/ai-nhancement/Anvil/sidecar/internal/contract"
	"github.com/ai-nhancement/Anvil/sidecar/internal/server"
)

const bufSize = 1024 * 1024

var minimalConfigJSON = []byte(`{"version":1,"connections":[]}`)

// newTestServer starts an AnvilServer over a bufconn listener and returns a
// connected SidecarClient plus a cleanup function. Each call creates an
// independent gRPC server, so tests share no state.
func newTestServer(t *testing.T) (contract.SidecarClient, func()) {
	t.Helper()
	cfg, err := config.ParseBytes(minimalConfigJSON)
	if err != nil {
		t.Fatalf("ParseBytes: %v", err)
	}

	lis := bufconn.Listen(bufSize)
	srv := server.New(cfg, "", "test", nil)
	gs := server.NewGRPCServer(srv)
	go gs.Serve(lis) //nolint:errcheck

	conn, err := grpc.NewClient(
		"passthrough:///bufnet",
		grpc.WithContextDialer(func(ctx context.Context, _ string) (net.Conn, error) {
			return lis.DialContext(ctx)
		}),
		grpc.WithTransportCredentials(insecure.NewCredentials()),
	)
	if err != nil {
		t.Fatalf("grpc.NewClient: %v", err)
	}

	client := contract.NewSidecarClient(conn)
	cleanup := func() {
		conn.Close()
		gs.Stop()
		lis.Close()
	}
	return client, cleanup
}

// newTestConn opens an additional gRPC connection to an already-running server.
// Used for multi-connection isolation tests.
func newTestConn(t *testing.T, lis *bufconn.Listener) (contract.SidecarClient, func()) {
	t.Helper()
	conn, err := grpc.NewClient(
		"passthrough:///bufnet",
		grpc.WithContextDialer(func(ctx context.Context, _ string) (net.Conn, error) {
			return lis.DialContext(ctx)
		}),
		grpc.WithTransportCredentials(insecure.NewCredentials()),
	)
	if err != nil {
		t.Fatalf("grpc.NewClient: %v", err)
	}
	return contract.NewSidecarClient(conn), func() { conn.Close() }
}

// integration_test: pins=health-exempt-from-handshake, intended=health-rpc-no-handshake-required, phase=P3c
func TestHealthNoHandshake(t *testing.T) {
	client, cleanup := newTestServer(t)
	defer cleanup()

	resp, err := client.Health(context.Background(), &contract.HealthRequest{})
	if err != nil {
		t.Fatalf("Health: %v", err)
	}
	if !resp.Healthy {
		t.Errorf("Health.Healthy = false, want true")
	}
}

// integration_test: pins=invoke-requires-handshake, intended=invoke-blocked-without-handshake, phase=P3c
func TestInvokeNoHandshake(t *testing.T) {
	client, cleanup := newTestServer(t)
	defer cleanup()

	_, err := client.Invoke(context.Background(), &contract.InvokeRequest{
		IdempotencyKey:       "00000000-0000-7000-8000-000000000001",
		ModelId:              "test-model",
		ProviderConnectionId: "conn-1",
	})
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if code := status.Code(err); code != codes.FailedPrecondition {
		t.Errorf("status code: got %v, want FailedPrecondition", code)
	}
}

// integration_test: pins=cancel-requires-handshake, intended=cancel-blocked-without-handshake, phase=P3c
func TestCancelNoHandshake(t *testing.T) {
	client, cleanup := newTestServer(t)
	defer cleanup()

	_, err := client.Cancel(context.Background(), &contract.CancelRequest{
		IdempotencyKey: "00000000-0000-7000-8000-000000000001",
	})
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if code := status.Code(err); code != codes.FailedPrecondition {
		t.Errorf("status code: got %v, want FailedPrecondition", code)
	}
}

// integration_test: pins=handshake-epoch-match, intended=handshake-round-trip-with-epoch-match, phase=P3c
func TestHandshakeSuccessEpochMatch(t *testing.T) {
	client, cleanup := newTestServer(t)
	defer cleanup()

	// Compute the expected epoch from the same bytes the server was initialized with.
	cfg, _ := config.ParseBytes(minimalConfigJSON)
	epoch := cfg.Epoch()

	resp, err := client.Handshake(context.Background(), &contract.HandshakeRequest{
		CoreProtocolVersion: "v1",
		SupportedVersions:   []string{"v1"},
		VaultConfigEpoch:    epoch,
	})
	if err != nil {
		t.Fatalf("Handshake: %v", err)
	}
	if resp.NegotiatedVersion != "v1" {
		t.Errorf("NegotiatedVersion: got %q, want %q", resp.NegotiatedVersion, "v1")
	}
	if resp.SidecarConfigEpoch != epoch {
		t.Errorf("SidecarConfigEpoch: got %q, want %q", resp.SidecarConfigEpoch, epoch)
	}
}

// integration_test: pins=two-connection-independent-state, intended=per-connection-handshake-isolation, phase=P3c
func TestTwoConnectionsIndependentHandshakeState(t *testing.T) {
	cfg, err := config.ParseBytes(minimalConfigJSON)
	if err != nil {
		t.Fatalf("ParseBytes: %v", err)
	}

	lis := bufconn.Listen(bufSize)
	srv := server.New(cfg, "", "test", nil)
	gs := server.NewGRPCServer(srv)
	go gs.Serve(lis) //nolint:errcheck
	defer gs.Stop()
	defer lis.Close()

	// conn1: perform handshake.
	client1, cleanup1 := newTestConn(t, lis)
	defer cleanup1()

	epoch := cfg.Epoch()
	if _, err := client1.Handshake(context.Background(), &contract.HandshakeRequest{
		CoreProtocolVersion: "v1",
		SupportedVersions:   []string{"v1"},
		VaultConfigEpoch:    epoch,
	}); err != nil {
		t.Fatalf("conn1 Handshake: %v", err)
	}

	// conn2: no handshake — must be blocked independently of conn1.
	client2, cleanup2 := newTestConn(t, lis)
	defer cleanup2()

	_, err = client2.Cancel(context.Background(), &contract.CancelRequest{
		IdempotencyKey: "00000000-0000-7000-8000-000000000002",
	})
	if err == nil {
		t.Fatal("conn2 Cancel: expected FailedPrecondition, got nil")
	}
	if code := status.Code(err); code != codes.FailedPrecondition {
		t.Errorf("conn2 Cancel status: got %v, want FailedPrecondition", code)
	}

	// conn1 can still call Health successfully after its handshake.
	if _, err := client1.Health(context.Background(), &contract.HealthRequest{}); err != nil {
		t.Errorf("conn1 Health after handshake: %v", err)
	}
}
