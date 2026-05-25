// Code generated from proto/anvil/v1/sidecar.proto — bootstrap for P3a (no protoc installed).
// Regenerate with `just gen-go` when the proto changes (requires protoc + protoc-gen-go + protoc-gen-go-grpc).
// @generated
//
// P3A SHAPE-ONLY BOOTSTRAP — NOT RUNTIME-USABLE PROTOBUF CODE.
// This file defines message structs for type-level testing only. It does NOT include
// the rawDesc binary (serialized FileDescriptorProto), so proto reflection, JSON
// marshaling, and actual gRPC wire encoding will panic at runtime. Regenerate with
// `just gen-go` (requires protoc) before P3c implementation.

package contract

import "fmt"

// ── ErrorClass ────────────────────────────────────────────────────────────────

// ErrorClass is the six-value error taxonomy. The zero value is a sentinel.
// Adding or removing a class is a breaking contract change (hinge-tested).
type ErrorClass int32

const (
	ErrorClass_ERROR_CLASS_UNSPECIFIED      ErrorClass = 0
	ErrorClass_ERROR_CLASS_TRANSPORT        ErrorClass = 1
	ErrorClass_ERROR_CLASS_PROVIDER_REFUSAL ErrorClass = 2
	ErrorClass_ERROR_CLASS_SCHEMA_VIOLATION ErrorClass = 3
	ErrorClass_ERROR_CLASS_ADAPTER_BUG      ErrorClass = 4
	ErrorClass_ERROR_CLASS_TIMEOUT          ErrorClass = 5
	ErrorClass_ERROR_CLASS_CANCELLED        ErrorClass = 6
)

var ErrorClass_name = map[int32]string{
	0: "ERROR_CLASS_UNSPECIFIED",
	1: "ERROR_CLASS_TRANSPORT",
	2: "ERROR_CLASS_PROVIDER_REFUSAL",
	3: "ERROR_CLASS_SCHEMA_VIOLATION",
	4: "ERROR_CLASS_ADAPTER_BUG",
	5: "ERROR_CLASS_TIMEOUT",
	6: "ERROR_CLASS_CANCELLED",
}

var ErrorClass_value = map[string]int32{
	"ERROR_CLASS_UNSPECIFIED":      0,
	"ERROR_CLASS_TRANSPORT":        1,
	"ERROR_CLASS_PROVIDER_REFUSAL": 2,
	"ERROR_CLASS_SCHEMA_VIOLATION": 3,
	"ERROR_CLASS_ADAPTER_BUG":      4,
	"ERROR_CLASS_TIMEOUT":          5,
	"ERROR_CLASS_CANCELLED":        6,
}

func (x ErrorClass) Enum() *ErrorClass {
	p := new(ErrorClass)
	*p = x
	return p
}

func (x ErrorClass) String() string {
	if name, ok := ErrorClass_name[int32(x)]; ok {
		return name
	}
	return fmt.Sprintf("ErrorClass(%d)", int32(x))
}

// ── Handshake ─────────────────────────────────────────────────────────────────

type HandshakeRequest struct {
	CoreProtocolVersion string   `protobuf:"bytes,1,opt,name=core_protocol_version,json=coreProtocolVersion,proto3"`
	SupportedVersions   []string `protobuf:"bytes,2,rep,name=supported_versions,json=supportedVersions,proto3"`
	VaultConfigEpoch    string   `protobuf:"bytes,3,opt,name=vault_config_epoch,json=vaultConfigEpoch,proto3"`
}

func (*HandshakeRequest) ProtoMessage()        {}
func (x *HandshakeRequest) Reset()             { *x = HandshakeRequest{} }
func (x *HandshakeRequest) String() string     { return fmt.Sprintf("%+v", *x) }

type HandshakeResponse struct {
	NegotiatedVersion  string `protobuf:"bytes,1,opt,name=negotiated_version,json=negotiatedVersion,proto3"`
	SidecarVersion     string `protobuf:"bytes,2,opt,name=sidecar_version,json=sidecarVersion,proto3"`
	SidecarBuildInfo   string `protobuf:"bytes,3,opt,name=sidecar_build_info,json=sidecarBuildInfo,proto3"`
	SidecarConfigEpoch string `protobuf:"bytes,4,opt,name=sidecar_config_epoch,json=sidecarConfigEpoch,proto3"`
}

func (*HandshakeResponse) ProtoMessage()        {}
func (x *HandshakeResponse) Reset()             { *x = HandshakeResponse{} }
func (x *HandshakeResponse) String() string     { return fmt.Sprintf("%+v", *x) }

// ── Invoke ────────────────────────────────────────────────────────────────────

type InvokeRequest struct {
	IdempotencyKey       string                  `protobuf:"bytes,1,opt,name=idempotency_key,json=idempotencyKey,proto3"`
	ModelId              string                  `protobuf:"bytes,2,opt,name=model_id,json=modelId,proto3"`
	ProviderConnectionId string                  `protobuf:"bytes,3,opt,name=provider_connection_id,json=providerConnectionId,proto3"`
	Credentials          *Credentials            `protobuf:"bytes,4,opt,name=credentials,proto3"`
	Timeout              *Timeout                `protobuf:"bytes,7,opt,name=timeout,proto3"`
	// Payload is the request body: either a ChatRequest or an EmbedRequest.
	// Types that are assignable to Payload:
	//   *InvokeRequest_Chat
	//   *InvokeRequest_Embed
	Payload isInvokeRequest_Payload `protobuf_oneof:"payload"`
}

func (*InvokeRequest) ProtoMessage()        {}
func (x *InvokeRequest) Reset()             { *x = InvokeRequest{} }
func (x *InvokeRequest) String() string     { return fmt.Sprintf("%+v", *x) }

type isInvokeRequest_Payload interface{ isInvokeRequest_Payload() }

type InvokeRequest_Chat struct {
	Chat *ChatRequest `protobuf:"bytes,5,opt,name=chat,proto3,oneof"`
}
type InvokeRequest_Embed struct {
	Embed *EmbedRequest `protobuf:"bytes,6,opt,name=embed,proto3,oneof"`
}

func (*InvokeRequest_Chat) isInvokeRequest_Payload()  {}
func (*InvokeRequest_Embed) isInvokeRequest_Payload() {}

type InvokeResponse struct {
	IdempotencyKey string                  `protobuf:"bytes,1,opt,name=idempotency_key,json=idempotencyKey,proto3"`
	// Result is the response body or an error.
	// Types that are assignable to Result:
	//   *InvokeResponse_Chat
	//   *InvokeResponse_Embed
	//   *InvokeResponse_Error
	Result isInvokeResponse_Result `protobuf_oneof:"result"`
}

func (*InvokeResponse) ProtoMessage()        {}
func (x *InvokeResponse) Reset()             { *x = InvokeResponse{} }
func (x *InvokeResponse) String() string     { return fmt.Sprintf("%+v", *x) }

type isInvokeResponse_Result interface{ isInvokeResponse_Result() }

type InvokeResponse_Chat struct {
	Chat *ChatResponse `protobuf:"bytes,2,opt,name=chat,proto3,oneof"`
}
type InvokeResponse_Embed struct {
	Embed *EmbedResponse `protobuf:"bytes,3,opt,name=embed,proto3,oneof"`
}
type InvokeResponse_Error struct {
	Error *AnvilError `protobuf:"bytes,4,opt,name=error,proto3,oneof"`
}

func (*InvokeResponse_Chat) isInvokeResponse_Result()  {}
func (*InvokeResponse_Embed) isInvokeResponse_Result() {}
func (*InvokeResponse_Error) isInvokeResponse_Result() {}

// ── Streaming ─────────────────────────────────────────────────────────────────

type InvokeStreamEvent struct {
	IdempotencyKey string                     `protobuf:"bytes,1,opt,name=idempotency_key,json=idempotencyKey,proto3"`
	// Event is the stream payload. Exactly one FinalResult or one Error terminates the stream.
	// Types that are assignable to Event:
	//   *InvokeStreamEvent_Token
	//   *InvokeStreamEvent_FinalResult
	//   *InvokeStreamEvent_Error
	//   *InvokeStreamEvent_Heartbeat
	Event isInvokeStreamEvent_Event `protobuf_oneof:"event"`
}

func (*InvokeStreamEvent) ProtoMessage()        {}
func (x *InvokeStreamEvent) Reset()             { *x = InvokeStreamEvent{} }
func (x *InvokeStreamEvent) String() string     { return fmt.Sprintf("%+v", *x) }

type isInvokeStreamEvent_Event interface{ isInvokeStreamEvent_Event() }

type InvokeStreamEvent_Token struct {
	Token *Token `protobuf:"bytes,2,opt,name=token,proto3,oneof"`
}
type InvokeStreamEvent_FinalResult struct {
	FinalResult *FinalResult `protobuf:"bytes,3,opt,name=final_result,json=finalResult,proto3,oneof"`
}
type InvokeStreamEvent_Error struct {
	Error *StreamError `protobuf:"bytes,4,opt,name=error,proto3,oneof"`
}
type InvokeStreamEvent_Heartbeat struct {
	Heartbeat *Heartbeat `protobuf:"bytes,5,opt,name=heartbeat,proto3,oneof"`
}

func (*InvokeStreamEvent_Token) isInvokeStreamEvent_Event()       {}
func (*InvokeStreamEvent_FinalResult) isInvokeStreamEvent_Event() {}
func (*InvokeStreamEvent_Error) isInvokeStreamEvent_Event()       {}
func (*InvokeStreamEvent_Heartbeat) isInvokeStreamEvent_Event()   {}

type Token struct {
	Text string `protobuf:"bytes,1,opt,name=text,proto3"`
}

func (*Token) ProtoMessage()        {}
func (x *Token) Reset()             { *x = Token{} }
func (x *Token) String() string     { return fmt.Sprintf("%+v", *x) }

type FinalResult struct {
	// Result is the authoritative response.
	// Types that are assignable to Result:
	//   *FinalResult_Chat
	//   *FinalResult_Embed
	Result isFinalResult_Result `protobuf_oneof:"result"`
}

func (*FinalResult) ProtoMessage()        {}
func (x *FinalResult) Reset()             { *x = FinalResult{} }
func (x *FinalResult) String() string     { return fmt.Sprintf("%+v", *x) }

type isFinalResult_Result interface{ isFinalResult_Result() }

type FinalResult_Chat struct {
	Chat *ChatResponse `protobuf:"bytes,1,opt,name=chat,proto3,oneof"`
}
type FinalResult_Embed struct {
	Embed *EmbedResponse `protobuf:"bytes,2,opt,name=embed,proto3,oneof"`
}

func (*FinalResult_Chat) isFinalResult_Result()  {}
func (*FinalResult_Embed) isFinalResult_Result() {}

type StreamError struct {
	Error *AnvilError `protobuf:"bytes,1,opt,name=error,proto3"`
}

func (*StreamError) ProtoMessage()        {}
func (x *StreamError) Reset()             { *x = StreamError{} }
func (x *StreamError) String() string     { return fmt.Sprintf("%+v", *x) }

type Heartbeat struct {
	TimestampMillis int64 `protobuf:"varint,1,opt,name=timestamp_millis,json=timestampMillis,proto3"`
}

func (*Heartbeat) ProtoMessage()        {}
func (x *Heartbeat) Reset()             { *x = Heartbeat{} }
func (x *Heartbeat) String() string     { return fmt.Sprintf("%+v", *x) }

// ── Cancel ────────────────────────────────────────────────────────────────────

type CancelRequest struct {
	IdempotencyKey string `protobuf:"bytes,1,opt,name=idempotency_key,json=idempotencyKey,proto3"`
}

func (*CancelRequest) ProtoMessage()        {}
func (x *CancelRequest) Reset()             { *x = CancelRequest{} }
func (x *CancelRequest) String() string     { return fmt.Sprintf("%+v", *x) }

type CancelResponse struct {
	Cancelled bool `protobuf:"varint,1,opt,name=cancelled,proto3"`
}

func (*CancelResponse) ProtoMessage()        {}
func (x *CancelResponse) Reset()             { *x = CancelResponse{} }
func (x *CancelResponse) String() string     { return fmt.Sprintf("%+v", *x) }

// ── Health ────────────────────────────────────────────────────────────────────

type HealthRequest struct{}

func (*HealthRequest) ProtoMessage()        {}
func (x *HealthRequest) Reset()             { *x = HealthRequest{} }
func (x *HealthRequest) String() string     { return fmt.Sprintf("%+v", *x) }

type HealthResponse struct {
	Healthy bool   `protobuf:"varint,1,opt,name=healthy,proto3"`
	Version string `protobuf:"bytes,2,opt,name=version,proto3"`
}

func (*HealthResponse) ProtoMessage()        {}
func (x *HealthResponse) Reset()             { *x = HealthResponse{} }
func (x *HealthResponse) String() string     { return fmt.Sprintf("%+v", *x) }

// ── ReloadConfig ──────────────────────────────────────────────────────────────

type ReloadConfigRequest struct {
	NewConfigEpoch    string `protobuf:"bytes,1,opt,name=new_config_epoch,json=newConfigEpoch,proto3"`
	NewProviderConfig []byte `protobuf:"bytes,2,opt,name=new_provider_config,json=newProviderConfig,proto3"`
}

func (*ReloadConfigRequest) ProtoMessage()        {}
func (x *ReloadConfigRequest) Reset()             { *x = ReloadConfigRequest{} }
func (x *ReloadConfigRequest) String() string     { return fmt.Sprintf("%+v", *x) }

type ReloadConfigResponse struct {
	Success           bool        `protobuf:"varint,1,opt,name=success,proto3"`
	Error             *AnvilError `protobuf:"bytes,2,opt,name=error,proto3"`
	ActiveConfigEpoch string      `protobuf:"bytes,3,opt,name=active_config_epoch,json=activeConfigEpoch,proto3"`
}

func (*ReloadConfigResponse) ProtoMessage()        {}
func (x *ReloadConfigResponse) Reset()             { *x = ReloadConfigResponse{} }
func (x *ReloadConfigResponse) String() string     { return fmt.Sprintf("%+v", *x) }

// ── Payloads ──────────────────────────────────────────────────────────────────

type ChatRequest struct {
	SystemPrompt string     `protobuf:"bytes,1,opt,name=system_prompt,json=systemPrompt,proto3"`
	Messages     []*Message `protobuf:"bytes,2,rep,name=messages,proto3"`
	// MaxTokens is nil when unset (uses provider/model default).
	MaxTokens   *int32   `protobuf:"varint,3,opt,name=max_tokens,json=maxTokens,proto3,oneof"`
	// Temperature is nil when unset (uses provider/model default).
	Temperature *float32 `protobuf:"fixed32,4,opt,name=temperature,proto3,oneof"`
}

func (*ChatRequest) ProtoMessage()        {}
func (x *ChatRequest) Reset()             { *x = ChatRequest{} }
func (x *ChatRequest) String() string     { return fmt.Sprintf("%+v", *x) }

type Message struct {
	Role    string `protobuf:"bytes,1,opt,name=role,proto3"`
	Content string `protobuf:"bytes,2,opt,name=content,proto3"`
}

func (*Message) ProtoMessage()        {}
func (x *Message) Reset()             { *x = Message{} }
func (x *Message) String() string     { return fmt.Sprintf("%+v", *x) }

type ChatResponse struct {
	Content      string  `protobuf:"bytes,1,opt,name=content,proto3"`
	Model        string  `protobuf:"bytes,2,opt,name=model,proto3"`
	Usage        *Usage  `protobuf:"bytes,3,opt,name=usage,proto3"`
	FinishReason string  `protobuf:"bytes,4,opt,name=finish_reason,json=finishReason,proto3"`
}

func (*ChatResponse) ProtoMessage()        {}
func (x *ChatResponse) Reset()             { *x = ChatResponse{} }
func (x *ChatResponse) String() string     { return fmt.Sprintf("%+v", *x) }

type EmbedRequest struct {
	Text string `protobuf:"bytes,1,opt,name=text,proto3"`
}

func (*EmbedRequest) ProtoMessage()        {}
func (x *EmbedRequest) Reset()             { *x = EmbedRequest{} }
func (x *EmbedRequest) String() string     { return fmt.Sprintf("%+v", *x) }

type EmbedResponse struct {
	Embedding []float32 `protobuf:"fixed32,1,rep,packed,name=embedding,proto3"`
	Model     string    `protobuf:"bytes,2,opt,name=model,proto3"`
}

func (*EmbedResponse) ProtoMessage()        {}
func (x *EmbedResponse) Reset()             { *x = EmbedResponse{} }
func (x *EmbedResponse) String() string     { return fmt.Sprintf("%+v", *x) }

type Usage struct {
	InputTokens  int32 `protobuf:"varint,1,opt,name=input_tokens,json=inputTokens,proto3"`
	OutputTokens int32 `protobuf:"varint,2,opt,name=output_tokens,json=outputTokens,proto3"`
}

func (*Usage) ProtoMessage()        {}
func (x *Usage) Reset()             { *x = Usage{} }
func (x *Usage) String() string     { return fmt.Sprintf("%+v", *x) }

// ── Credentials ───────────────────────────────────────────────────────────────

type Credentials struct {
	// Credential is the per-call secret material.
	// Types that are assignable to Credential:
	//   *Credentials_ApiKey
	//   *Credentials_BearerToken
	Credential isCredentials_Credential `protobuf_oneof:"credential"`
}

func (*Credentials) ProtoMessage()        {}
func (x *Credentials) Reset()             { *x = Credentials{} }
func (x *Credentials) String() string     { return fmt.Sprintf("%+v", *x) }

type isCredentials_Credential interface{ isCredentials_Credential() }

type Credentials_ApiKey struct {
	ApiKey string `protobuf:"bytes,1,opt,name=api_key,json=apiKey,proto3,oneof"`
}
type Credentials_BearerToken struct {
	BearerToken string `protobuf:"bytes,2,opt,name=bearer_token,json=bearerToken,proto3,oneof"`
}

func (*Credentials_ApiKey) isCredentials_Credential()     {}
func (*Credentials_BearerToken) isCredentials_Credential() {}

// ── Timeout ───────────────────────────────────────────────────────────────────

type Timeout struct {
	Millis uint64 `protobuf:"varint,1,opt,name=millis,proto3"`
}

func (*Timeout) ProtoMessage()        {}
func (x *Timeout) Reset()             { *x = Timeout{} }
func (x *Timeout) String() string     { return fmt.Sprintf("%+v", *x) }

// ── Errors ────────────────────────────────────────────────────────────────────

type AnvilError struct {
	Class      ErrorClass        `protobuf:"varint,1,opt,name=class,proto3,enum=anvil.v1.ErrorClass"`
	VendorCode string            `protobuf:"bytes,2,opt,name=vendor_code,json=vendorCode,proto3"`
	Message    string            `protobuf:"bytes,3,opt,name=message,proto3"`
	Details    map[string]string `protobuf:"bytes,4,rep,name=details,proto3" protobuf_key:"bytes,1,opt,name=key,proto3" protobuf_val:"bytes,2,opt,name=value,proto3"`
}

func (*AnvilError) ProtoMessage()        {}
func (x *AnvilError) Reset()             { *x = AnvilError{} }
func (x *AnvilError) String() string     { return fmt.Sprintf("%+v", *x) }
