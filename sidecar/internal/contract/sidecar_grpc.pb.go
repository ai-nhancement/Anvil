// Code generated from proto/anvil/v1/sidecar.proto — bootstrap for P3a (no protoc installed).
// Regenerate with `just gen-go` when the proto changes (requires protoc + protoc-gen-go + protoc-gen-go-grpc).
// @generated

package contract

import (
	context "context"
	grpc "google.golang.org/grpc"
	codes "google.golang.org/grpc/codes"
	status "google.golang.org/grpc/status"
)

// This is a compile-time assertion to ensure that this generated file
// is compatible with the grpc package it is being compiled against.
// Requires gRPC-Go v1.64.0 or later.
const _ = grpc.SupportPackageIsVersion9

const (
	Sidecar_Handshake_FullMethodName      = "/anvil.v1.Sidecar/Handshake"
	Sidecar_Invoke_FullMethodName         = "/anvil.v1.Sidecar/Invoke"
	Sidecar_InvokeStreaming_FullMethodName = "/anvil.v1.Sidecar/InvokeStreaming"
	Sidecar_Cancel_FullMethodName         = "/anvil.v1.Sidecar/Cancel"
	Sidecar_Health_FullMethodName         = "/anvil.v1.Sidecar/Health"
	Sidecar_ReloadConfig_FullMethodName   = "/anvil.v1.Sidecar/ReloadConfig"
)

// ── Client ────────────────────────────────────────────────────────────────────

// SidecarClient is the client API for the Sidecar service.
//
// For semantics around ctx use and closing/ending streaming RPCs, please refer to
// https://pkg.go.dev/google.golang.org/grpc/?tab=doc#ClientConn.NewStream.
type SidecarClient interface {
	Handshake(ctx context.Context, in *HandshakeRequest, opts ...grpc.CallOption) (*HandshakeResponse, error)
	Invoke(ctx context.Context, in *InvokeRequest, opts ...grpc.CallOption) (*InvokeResponse, error)
	InvokeStreaming(ctx context.Context, in *InvokeRequest, opts ...grpc.CallOption) (grpc.ServerStreamingClient[InvokeStreamEvent], error)
	Cancel(ctx context.Context, in *CancelRequest, opts ...grpc.CallOption) (*CancelResponse, error)
	Health(ctx context.Context, in *HealthRequest, opts ...grpc.CallOption) (*HealthResponse, error)
	ReloadConfig(ctx context.Context, in *ReloadConfigRequest, opts ...grpc.CallOption) (*ReloadConfigResponse, error)
}

type sidecarClient struct {
	cc grpc.ClientConnInterface
}

func NewSidecarClient(cc grpc.ClientConnInterface) SidecarClient {
	return &sidecarClient{cc}
}

func (c *sidecarClient) Handshake(ctx context.Context, in *HandshakeRequest, opts ...grpc.CallOption) (*HandshakeResponse, error) {
	cOpts := append([]grpc.CallOption{grpc.StaticMethod()}, opts...)
	out := new(HandshakeResponse)
	err := c.cc.Invoke(ctx, Sidecar_Handshake_FullMethodName, in, out, cOpts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *sidecarClient) Invoke(ctx context.Context, in *InvokeRequest, opts ...grpc.CallOption) (*InvokeResponse, error) {
	cOpts := append([]grpc.CallOption{grpc.StaticMethod()}, opts...)
	out := new(InvokeResponse)
	err := c.cc.Invoke(ctx, Sidecar_Invoke_FullMethodName, in, out, cOpts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *sidecarClient) InvokeStreaming(ctx context.Context, in *InvokeRequest, opts ...grpc.CallOption) (grpc.ServerStreamingClient[InvokeStreamEvent], error) {
	cOpts := append([]grpc.CallOption{grpc.StaticMethod()}, opts...)
	stream, err := c.cc.NewStream(ctx, &Sidecar_ServiceDesc.Streams[0], Sidecar_InvokeStreaming_FullMethodName, cOpts...)
	if err != nil {
		return nil, err
	}
	x := &grpc.GenericClientStream[InvokeRequest, InvokeStreamEvent]{ClientStream: stream}
	if err := x.ClientStream.SendMsg(in); err != nil {
		return nil, err
	}
	if err := x.ClientStream.CloseSend(); err != nil {
		return nil, err
	}
	return x, nil
}

func (c *sidecarClient) Cancel(ctx context.Context, in *CancelRequest, opts ...grpc.CallOption) (*CancelResponse, error) {
	cOpts := append([]grpc.CallOption{grpc.StaticMethod()}, opts...)
	out := new(CancelResponse)
	err := c.cc.Invoke(ctx, Sidecar_Cancel_FullMethodName, in, out, cOpts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *sidecarClient) Health(ctx context.Context, in *HealthRequest, opts ...grpc.CallOption) (*HealthResponse, error) {
	cOpts := append([]grpc.CallOption{grpc.StaticMethod()}, opts...)
	out := new(HealthResponse)
	err := c.cc.Invoke(ctx, Sidecar_Health_FullMethodName, in, out, cOpts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *sidecarClient) ReloadConfig(ctx context.Context, in *ReloadConfigRequest, opts ...grpc.CallOption) (*ReloadConfigResponse, error) {
	cOpts := append([]grpc.CallOption{grpc.StaticMethod()}, opts...)
	out := new(ReloadConfigResponse)
	err := c.cc.Invoke(ctx, Sidecar_ReloadConfig_FullMethodName, in, out, cOpts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

// ── Server ────────────────────────────────────────────────────────────────────

// SidecarServer is the server API for the Sidecar service.
// All implementations must embed UnimplementedSidecarServer for forward compatibility.
type SidecarServer interface {
	Handshake(context.Context, *HandshakeRequest) (*HandshakeResponse, error)
	Invoke(context.Context, *InvokeRequest) (*InvokeResponse, error)
	InvokeStreaming(*InvokeRequest, grpc.ServerStreamingServer[InvokeStreamEvent]) error
	Cancel(context.Context, *CancelRequest) (*CancelResponse, error)
	Health(context.Context, *HealthRequest) (*HealthResponse, error)
	ReloadConfig(context.Context, *ReloadConfigRequest) (*ReloadConfigResponse, error)
	mustEmbedUnimplementedSidecarServer()
}

// UnimplementedSidecarServer must be embedded to have forward-compatible implementations.
//
// NOTE: embed by value instead of pointer to avoid nil pointer dereferences when methods are called.
type UnimplementedSidecarServer struct{}

func (UnimplementedSidecarServer) Handshake(context.Context, *HandshakeRequest) (*HandshakeResponse, error) {
	return nil, status.Error(codes.Unimplemented, "method Handshake not implemented")
}
func (UnimplementedSidecarServer) Invoke(context.Context, *InvokeRequest) (*InvokeResponse, error) {
	return nil, status.Error(codes.Unimplemented, "method Invoke not implemented")
}
func (UnimplementedSidecarServer) InvokeStreaming(*InvokeRequest, grpc.ServerStreamingServer[InvokeStreamEvent]) error {
	return status.Error(codes.Unimplemented, "method InvokeStreaming not implemented")
}
func (UnimplementedSidecarServer) Cancel(context.Context, *CancelRequest) (*CancelResponse, error) {
	return nil, status.Error(codes.Unimplemented, "method Cancel not implemented")
}
func (UnimplementedSidecarServer) Health(context.Context, *HealthRequest) (*HealthResponse, error) {
	return nil, status.Error(codes.Unimplemented, "method Health not implemented")
}
func (UnimplementedSidecarServer) ReloadConfig(context.Context, *ReloadConfigRequest) (*ReloadConfigResponse, error) {
	return nil, status.Error(codes.Unimplemented, "method ReloadConfig not implemented")
}
func (UnimplementedSidecarServer) mustEmbedUnimplementedSidecarServer() {}
func (UnimplementedSidecarServer) testEmbeddedByValue()                  {}

// UnsafeSidecarServer may be embedded to opt out of forward compatibility for this service.
// Use of this interface is not recommended, as added methods to SidecarServer will
// result in compilation errors.
type UnsafeSidecarServer interface {
	mustEmbedUnimplementedSidecarServer()
}

func RegisterSidecarServer(s grpc.ServiceRegistrar, srv SidecarServer) {
	// If the following call panics, it indicates UnimplementedSidecarServer was
	// embedded by pointer and is nil.
	if t, ok := srv.(interface{ testEmbeddedByValue() }); ok {
		t.testEmbeddedByValue()
	}
	s.RegisterService(&Sidecar_ServiceDesc, srv)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

func _Sidecar_Handshake_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(HandshakeRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(SidecarServer).Handshake(ctx, in)
	}
	info := &grpc.UnaryServerInfo{Server: srv, FullMethod: Sidecar_Handshake_FullMethodName}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(SidecarServer).Handshake(ctx, req.(*HandshakeRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _Sidecar_Invoke_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(InvokeRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(SidecarServer).Invoke(ctx, in)
	}
	info := &grpc.UnaryServerInfo{Server: srv, FullMethod: Sidecar_Invoke_FullMethodName}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(SidecarServer).Invoke(ctx, req.(*InvokeRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _Sidecar_InvokeStreaming_Handler(srv interface{}, stream grpc.ServerStream) error {
	m := new(InvokeRequest)
	if err := stream.RecvMsg(m); err != nil {
		return err
	}
	return srv.(SidecarServer).InvokeStreaming(m, &grpc.GenericServerStream[InvokeRequest, InvokeStreamEvent]{ServerStream: stream})
}

func _Sidecar_Cancel_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(CancelRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(SidecarServer).Cancel(ctx, in)
	}
	info := &grpc.UnaryServerInfo{Server: srv, FullMethod: Sidecar_Cancel_FullMethodName}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(SidecarServer).Cancel(ctx, req.(*CancelRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _Sidecar_Health_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(HealthRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(SidecarServer).Health(ctx, in)
	}
	info := &grpc.UnaryServerInfo{Server: srv, FullMethod: Sidecar_Health_FullMethodName}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(SidecarServer).Health(ctx, req.(*HealthRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _Sidecar_ReloadConfig_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(ReloadConfigRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(SidecarServer).ReloadConfig(ctx, in)
	}
	info := &grpc.UnaryServerInfo{Server: srv, FullMethod: Sidecar_ReloadConfig_FullMethodName}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(SidecarServer).ReloadConfig(ctx, req.(*ReloadConfigRequest))
	}
	return interceptor(ctx, in, info, handler)
}

// Sidecar_ServiceDesc is the grpc.ServiceDesc for the Sidecar service.
var Sidecar_ServiceDesc = grpc.ServiceDesc{
	ServiceName: "anvil.v1.Sidecar",
	HandlerType: (*SidecarServer)(nil),
	Methods: []grpc.MethodDesc{
		{MethodName: "Handshake", Handler: _Sidecar_Handshake_Handler},
		{MethodName: "Invoke", Handler: _Sidecar_Invoke_Handler},
		{MethodName: "Cancel", Handler: _Sidecar_Cancel_Handler},
		{MethodName: "Health", Handler: _Sidecar_Health_Handler},
		{MethodName: "ReloadConfig", Handler: _Sidecar_ReloadConfig_Handler},
	},
	Streams: []grpc.StreamDesc{
		{
			StreamName:    "InvokeStreaming",
			Handler:       _Sidecar_InvokeStreaming_Handler,
			ServerStreams: true,
		},
	},
	Metadata: "anvil/v1/sidecar.proto",
}
