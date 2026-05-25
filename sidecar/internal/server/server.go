// Package server implements the gRPC Sidecar service.
package server

import (
	"context"
	"fmt"
	"sync"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/stats"
	"google.golang.org/grpc/status"

	"github.com/ai-nhancement/Anvil/sidecar/internal/adapters"
	"github.com/ai-nhancement/Anvil/sidecar/internal/config"
	contract "github.com/ai-nhancement/Anvil/sidecar/internal/contract"
	svcerrors "github.com/ai-nhancement/Anvil/sidecar/internal/errors"
)

// serverSupportedVersions is the ordered list of protocol versions this sidecar accepts.
// Changing this list is a breaking protocol change — coordinate with the Vault.
var serverSupportedVersions = []string{"v1"}

// ServerSupportedVersions returns the protocol versions this sidecar accepts (hinge-tested).
func ServerSupportedVersions() []string { return serverSupportedVersions }

// maxTimeoutMillis is the largest timeout the sidecar will accept.
// Oversized values must be rejected with SCHEMA_VIOLATION (per proto/README.md).
const maxTimeoutMillis = uint64(10 * 60 * 1000) // 10 minutes

// connKey is the context key for the per-connection ID.
type connKey struct{}

// connState is the mutable per-connection handshake state.
type connState struct {
	mu         sync.Mutex
	handshaked bool
	epochMatch bool // true when vault_config_epoch matched sidecar_config_epoch at Handshake
}

// AnvilServer implements the contract.SidecarServer gRPC interface.
type AnvilServer struct {
	contract.UnimplementedSidecarServer

	cfgMu   sync.RWMutex
	cfg     *config.Config
	cfgPath string

	conns   sync.Map // connID (string) → *connState
	cancels sync.Map // idempotencyKey (string) → context.CancelFunc

	version string
	touch   func() // reset idle timer; may be nil
}

// New creates an AnvilServer. cfg must not be nil. touch is called on each incoming request
// to reset the idle timer; pass nil to disable idle tracking.
func New(cfg *config.Config, cfgPath, version string, touch func()) *AnvilServer {
	if cfg == nil {
		panic("server.New: cfg must not be nil")
	}
	return &AnvilServer{cfg: cfg, cfgPath: cfgPath, version: version, touch: touch}
}

// NewGRPCServer returns a grpc.Server with the stats handler and SidecarServer already wired.
// The stats handler is required for per-connection handshake state; omitting it causes all
// guarded RPCs to return "connection state not found."
func NewGRPCServer(srv *AnvilServer, opts ...grpc.ServerOption) *grpc.Server {
	sh := NewStatsHandler(srv)
	allOpts := append([]grpc.ServerOption{grpc.StatsHandler(sh)}, opts...)
	gs := grpc.NewServer(allOpts...)
	Register(gs, srv)
	return gs
}

// Register registers srv with the gRPC service registrar.
func Register(gs grpc.ServiceRegistrar, srv *AnvilServer) {
	contract.RegisterSidecarServer(gs, srv)
}

// NewStatsHandler returns the grpc/stats.Handler that attaches per-connection state to each RPC context.
func NewStatsHandler(srv *AnvilServer) stats.Handler {
	return &connStatsHandler{server: srv}
}

// connStatsHandler implements grpc/stats.Handler for per-connection state tracking.
type connStatsHandler struct {
	server *AnvilServer
	mu     sync.Mutex
	nextID uint64
}

func (h *connStatsHandler) TagRPC(ctx context.Context, _ *stats.RPCTagInfo) context.Context {
	return ctx
}

func (h *connStatsHandler) HandleRPC(_ context.Context, _ stats.RPCStats) {}

func (h *connStatsHandler) TagConn(ctx context.Context, _ *stats.ConnTagInfo) context.Context {
	h.mu.Lock()
	h.nextID++
	id := fmt.Sprintf("conn-%d", h.nextID)
	h.mu.Unlock()

	h.server.conns.Store(id, &connState{})
	return context.WithValue(ctx, connKey{}, id)
}

func (h *connStatsHandler) HandleConn(ctx context.Context, s stats.ConnStats) {
	if _, ok := s.(*stats.ConnEnd); ok {
		if id, ok := ctx.Value(connKey{}).(string); ok {
			h.server.conns.Delete(id)
		}
	}
}

func (s *AnvilServer) getConnState(ctx context.Context) *connState {
	id, _ := ctx.Value(connKey{}).(string)
	if id == "" {
		return nil
	}
	v, ok := s.conns.Load(id)
	if !ok {
		return nil
	}
	return v.(*connState)
}

func (s *AnvilServer) requireHandshake(ctx context.Context) (*connState, error) {
	cs := s.getConnState(ctx)
	if cs == nil {
		return nil, status.Error(codes.Internal, "connection state not found")
	}
	cs.mu.Lock()
	h := cs.handshaked
	cs.mu.Unlock()
	if !h {
		return nil, status.Error(codes.FailedPrecondition, "Handshake must be called before this RPC")
	}
	return cs, nil
}

func (s *AnvilServer) requireReady(ctx context.Context) (*connState, error) {
	cs, err := s.requireHandshake(ctx)
	if err != nil {
		return nil, err
	}
	cs.mu.Lock()
	e := cs.epochMatch
	cs.mu.Unlock()
	if !e {
		return nil, status.Error(codes.FailedPrecondition, "config epoch mismatch: call ReloadConfig before Invoke")
	}
	return cs, nil
}

func (s *AnvilServer) touchActivity() {
	if s.touch != nil {
		s.touch()
	}
}

// isValidUUIDv7 returns true if s is a valid UUIDv7 string.
func isValidUUIDv7(s string) bool {
	if len(s) != 36 {
		return false
	}
	for i := 0; i < 36; i++ {
		c := s[i]
		switch i {
		case 8, 13, 18, 23:
			if c != '-' {
				return false
			}
		default:
			if !isHexByte(c) {
				return false
			}
		}
	}
	return s[14] == '7' // version nibble
}

func isHexByte(c byte) bool {
	return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')
}

func validateRequest(key, modelID, connID string, creds *contract.Credentials, payload any) *contract.AnvilError {
	if !isValidUUIDv7(key) {
		return schemaErr("idempotency_key must be a non-empty UUIDv7 string (format: xxxxxxxx-xxxx-7xxx-yxxx-xxxxxxxxxxxx)")
	}
	if modelID == "" {
		return schemaErr("model_id is required")
	}
	if connID == "" {
		return schemaErr("provider_connection_id is required")
	}
	if creds == nil {
		return schemaErr("credentials are required")
	}
	if payload == nil {
		return schemaErr("payload is required")
	}
	return nil
}

func applyTimeout(ctx context.Context, timeout *contract.Timeout) (context.Context, context.CancelFunc, *contract.AnvilError) {
	if timeout == nil || timeout.Millis == 0 {
		return ctx, func() {}, nil
	}
	if timeout.Millis > maxTimeoutMillis {
		return ctx, func() {}, schemaErr(fmt.Sprintf(
			"timeout %d ms exceeds maximum %d ms (10 minutes)", timeout.Millis, maxTimeoutMillis))
	}
	ctx, cancel := context.WithTimeout(ctx, time.Duration(timeout.Millis)*time.Millisecond)
	return ctx, cancel, nil
}

// Handshake negotiates the protocol version and records the config-epoch comparison result.
func (s *AnvilServer) Handshake(ctx context.Context, req *contract.HandshakeRequest) (*contract.HandshakeResponse, error) {
	if req.CoreProtocolVersion == "" || len(req.SupportedVersions) == 0 {
		return nil, status.Error(codes.InvalidArgument, "core_protocol_version and supported_versions are required")
	}

	cs := s.getConnState(ctx)
	if cs == nil {
		return nil, status.Error(codes.Internal, "connection state not found")
	}

	negotiated := ""
	for _, offered := range req.SupportedVersions {
		for _, supported := range serverSupportedVersions {
			if offered == supported {
				negotiated = offered
				break
			}
		}
		if negotiated != "" {
			break
		}
	}
	if negotiated == "" {
		return nil, status.Errorf(codes.FailedPrecondition,
			"no supported protocol version: client offered %v, server supports %v",
			req.SupportedVersions, serverSupportedVersions)
	}

	s.cfgMu.RLock()
	epoch := s.cfg.Epoch()
	s.cfgMu.RUnlock()

	cs.mu.Lock()
	cs.handshaked = true
	cs.epochMatch = req.VaultConfigEpoch == epoch
	cs.mu.Unlock()

	return &contract.HandshakeResponse{
		NegotiatedVersion:  negotiated,
		SidecarVersion:     s.version,
		SidecarConfigEpoch: epoch,
	}, nil
}

// Invoke executes a unary model call.
func (s *AnvilServer) Invoke(ctx context.Context, req *contract.InvokeRequest) (*contract.InvokeResponse, error) {
	if _, err := s.requireReady(ctx); err != nil {
		return nil, err
	}
	s.touchActivity()

	key := req.IdempotencyKey

	if ae := validateRequest(key, req.ModelId, req.ProviderConnectionId, req.Credentials, req.GetPayload()); ae != nil {
		return invokeErrResp(key, ae), nil
	}
	if req.GetEmbed() != nil {
		return invokeErrResp(key, schemaErr("embed not supported in this version; use chat payload")), nil
	}

	// Register cancel so the Cancel RPC can abort this unary call.
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()
	s.cancels.Store(key, cancel)
	defer s.cancels.Delete(key)

	ctx, timeoutCancel, ae := applyTimeout(ctx, req.Timeout)
	if ae != nil {
		return invokeErrResp(key, ae), nil
	}
	defer timeoutCancel()

	s.cfgMu.RLock()
	cfg := s.cfg
	s.cfgMu.RUnlock()

	conn, ok := cfg.ConnectionByID(req.ProviderConnectionId)
	if !ok {
		return invokeErrResp(key, schemaErr(fmt.Sprintf("provider_connection_id %q not found in config", req.ProviderConnectionId))), nil
	}

	adapter, ok := adapters.Known[conn.Provider]
	if !ok {
		return invokeErrResp(key, svcerrors.New(contract.ErrorClass_ERROR_CLASS_ADAPTER_BUG, "",
			fmt.Sprintf("no adapter registered for provider %q", conn.Provider))), nil
	}

	resp, err := adapter.Invoke(ctx, conn, req)
	if err != nil {
		if ctx.Err() == context.DeadlineExceeded {
			return invokeErrResp(key, svcerrors.New(contract.ErrorClass_ERROR_CLASS_TIMEOUT, "", "request timed out")), nil
		}
		if ctx.Err() == context.Canceled {
			return nil, status.Error(codes.Canceled, "request canceled")
		}
		return invokeErrResp(key, svcerrors.New(contract.ErrorClass_ERROR_CLASS_TRANSPORT, "", err.Error())), nil
	}
	return resp, nil
}

// InvokeStreaming executes a streaming model call.
func (s *AnvilServer) InvokeStreaming(req *contract.InvokeRequest, stream grpc.ServerStreamingServer[contract.InvokeStreamEvent]) error {
	ctx := stream.Context()

	if _, err := s.requireReady(ctx); err != nil {
		return err
	}
	s.touchActivity()

	key := req.IdempotencyKey

	if ae := validateRequest(key, req.ModelId, req.ProviderConnectionId, req.Credentials, req.GetPayload()); ae != nil {
		return stream.Send(streamErrEvent(key, ae))
	}
	if req.GetEmbed() != nil {
		return stream.Send(streamErrEvent(key, schemaErr("embed not supported; use chat payload")))
	}

	// Register cancel so the Cancel RPC can abort this stream.
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()
	s.cancels.Store(key, cancel)
	defer s.cancels.Delete(key)

	ctx, timeoutCancel, ae := applyTimeout(ctx, req.Timeout)
	if ae != nil {
		return stream.Send(streamErrEvent(key, ae))
	}
	defer timeoutCancel()

	s.cfgMu.RLock()
	cfg := s.cfg
	s.cfgMu.RUnlock()

	conn, ok := cfg.ConnectionByID(req.ProviderConnectionId)
	if !ok {
		return stream.Send(streamErrEvent(key, schemaErr(fmt.Sprintf("provider_connection_id %q not found in config", req.ProviderConnectionId))))
	}

	adapter, ok := adapters.Known[conn.Provider]
	if !ok {
		return stream.Send(streamErrEvent(key, svcerrors.New(contract.ErrorClass_ERROR_CLASS_ADAPTER_BUG, "",
			fmt.Sprintf("no adapter registered for provider %q", conn.Provider))))
	}

	send := func(ev *contract.InvokeStreamEvent) error { return stream.Send(ev) }

	err := adapter.InvokeStreaming(ctx, conn, req, send)
	if err != nil {
		// Context canceled (by Cancel RPC or client disconnect): no terminal event possible.
		if ctx.Err() == context.Canceled {
			return status.Error(codes.Canceled, "streaming canceled")
		}
		// Timeout: send terminal StreamError so the state machine is upheld.
		if ctx.Err() == context.DeadlineExceeded {
			_ = stream.Send(streamErrEvent(key, svcerrors.New(contract.ErrorClass_ERROR_CLASS_TIMEOUT, "", "streaming timed out")))
			return nil
		}
		// Generic transport failure: send terminal StreamError — do not close the stream with
		// a bare gRPC error, which would violate the streaming state machine invariant.
		_ = stream.Send(streamErrEvent(key, svcerrors.New(contract.ErrorClass_ERROR_CLASS_TRANSPORT, "", err.Error())))
		return nil
	}
	return nil
}

// Cancel signals an in-flight unary or streaming call to abort.
func (s *AnvilServer) Cancel(ctx context.Context, req *contract.CancelRequest) (*contract.CancelResponse, error) {
	// Handshake guard: Cancel is an application RPC (not a liveness probe).
	if _, err := s.requireHandshake(ctx); err != nil {
		return nil, err
	}
	cancelled := false
	if v, ok := s.cancels.Load(req.IdempotencyKey); ok {
		v.(context.CancelFunc)()
		cancelled = true
	}
	return &contract.CancelResponse{Cancelled: cancelled}, nil
}

// Health reports liveness. Health is exempt from the Handshake-first requirement
// to support liveness/readiness probing before any client session is established.
func (s *AnvilServer) Health(_ context.Context, _ *contract.HealthRequest) (*contract.HealthResponse, error) {
	return &contract.HealthResponse{Healthy: true, Version: s.version}, nil
}

// ReloadConfig atomically replaces the in-memory provider config.
// The Vault sends the full config bytes; the sidecar verifies the SHA-256 epoch before applying.
func (s *AnvilServer) ReloadConfig(ctx context.Context, req *contract.ReloadConfigRequest) (*contract.ReloadConfigResponse, error) {
	cs, err := s.requireHandshake(ctx)
	if err != nil {
		return nil, err
	}

	newCfg, parseErr := config.ParseBytes(req.NewProviderConfig)
	if parseErr != nil {
		s.cfgMu.RLock()
		activeEpoch := s.cfg.Epoch()
		s.cfgMu.RUnlock()
		return &contract.ReloadConfigResponse{
			Success:           false,
			Error:             svcerrors.New(contract.ErrorClass_ERROR_CLASS_SCHEMA_VIOLATION, "", fmt.Sprintf("parse config: %v", parseErr)),
			ActiveConfigEpoch: activeEpoch,
		}, nil
	}

	if req.NewConfigEpoch != "" && newCfg.Epoch() != req.NewConfigEpoch {
		s.cfgMu.RLock()
		activeEpoch := s.cfg.Epoch()
		s.cfgMu.RUnlock()
		return &contract.ReloadConfigResponse{
			Success:           false,
			Error:             svcerrors.New(contract.ErrorClass_ERROR_CLASS_SCHEMA_VIOLATION, "", "config epoch mismatch: SHA-256 of payload does not match new_config_epoch"),
			ActiveConfigEpoch: activeEpoch,
		}, nil
	}

	s.cfgMu.Lock()
	s.cfg = newCfg
	s.cfgMu.Unlock()

	cs.mu.Lock()
	cs.epochMatch = true
	cs.mu.Unlock()

	return &contract.ReloadConfigResponse{
		Success:           true,
		ActiveConfigEpoch: newCfg.Epoch(),
	}, nil
}

func invokeErrResp(key string, ae *contract.AnvilError) *contract.InvokeResponse {
	return &contract.InvokeResponse{
		IdempotencyKey: key,
		Result:         &contract.InvokeResponse_Error{Error: ae},
	}
}

func streamErrEvent(key string, ae *contract.AnvilError) *contract.InvokeStreamEvent {
	return &contract.InvokeStreamEvent{
		IdempotencyKey: key,
		Event:          &contract.InvokeStreamEvent_Error{Error: &contract.StreamError{Error: ae}},
	}
}

func schemaErr(msg string) *contract.AnvilError {
	return svcerrors.New(contract.ErrorClass_ERROR_CLASS_SCHEMA_VIOLATION, "", msg)
}
