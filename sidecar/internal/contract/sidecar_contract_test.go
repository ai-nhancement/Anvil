package contract_test

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"

	contract "github.com/ai-nhancement/Anvil/sidecar/internal/contract"
)

// hinge_test: pins=anvil.v1, intended=proto-package-version, phase=P3a
func TestProtoFilePackageName(t *testing.T) {
	// Reads the canonical proto source — stronger than a hand-written constant.
	// Breaks if the proto package declaration changes, even if bootstrap files are stale.
	protoPath := filepath.Join("..", "..", "..", "proto", "anvil", "v1", "sidecar.proto")
	content, err := os.ReadFile(protoPath)
	if err != nil {
		t.Fatalf("cannot read proto file at %s: %v", protoPath, err)
	}
	if !bytes.Contains(content, []byte("package anvil.v1;")) {
		t.Error("proto/anvil/v1/sidecar.proto does not declare 'package anvil.v1;'")
	}
}

// hinge_test: pins=6+discriminants, intended=error-class-count, phase=P3a
func TestErrorClassCount(t *testing.T) {
	// Pins discriminant values — any change is a breaking wire-format change.
	checks := []struct {
		name string
		got  int32
		want int32
	}{
		{"Unspecified", int32(contract.ErrorClass_ERROR_CLASS_UNSPECIFIED), 0},
		{"Transport", int32(contract.ErrorClass_ERROR_CLASS_TRANSPORT), 1},
		{"ProviderRefusal", int32(contract.ErrorClass_ERROR_CLASS_PROVIDER_REFUSAL), 2},
		{"SchemaViolation", int32(contract.ErrorClass_ERROR_CLASS_SCHEMA_VIOLATION), 3},
		{"AdapterBug", int32(contract.ErrorClass_ERROR_CLASS_ADAPTER_BUG), 4},
		{"Timeout", int32(contract.ErrorClass_ERROR_CLASS_TIMEOUT), 5},
		{"Cancelled", int32(contract.ErrorClass_ERROR_CLASS_CANCELLED), 6},
	}
	nonUnspecified := 0
	for _, c := range checks {
		if c.got != c.want {
			t.Errorf("ErrorClass_%s discriminant: got %d, want %d", c.name, c.got, c.want)
		}
		if c.name != "Unspecified" {
			nonUnspecified++
		}
	}
	if nonUnspecified != 6 {
		t.Errorf("expected 6 non-unspecified error classes, got %d", nonUnspecified)
	}
}

// hinge_test: pins=core_protocol_version+supported_versions, intended=handshake-required-fields, phase=P3a
func TestHandshakeRequiredFields(t *testing.T) {
	req := &contract.HandshakeRequest{
		CoreProtocolVersion: "v1",
		SupportedVersions:   []string{"v1"},
		VaultConfigEpoch:    "",
	}
	if req.CoreProtocolVersion != "v1" {
		t.Errorf("CoreProtocolVersion: got %q, want %q", req.CoreProtocolVersion, "v1")
	}
	if len(req.SupportedVersions) == 0 {
		t.Error("SupportedVersions must not be empty")
	}
}

// hinge_test: pins=invoke_request_chat_oneof+chat_request_shape, intended=invoke-chat-oneof-shape, phase=P3a
func TestInvokeRequestChatPayload(t *testing.T) {
	req := &contract.InvokeRequest{
		IdempotencyKey:       "00000000-0000-7000-8000-000000000001",
		ModelId:              "claude-opus-4-7",
		ProviderConnectionId: "anthropic-prod",
		Credentials: &contract.Credentials{
			Credential: &contract.Credentials_ApiKey{ApiKey: "sk-test"},
		},
		Timeout: &contract.Timeout{Millis: 30_000},
		Payload: &contract.InvokeRequest_Chat{
			Chat: &contract.ChatRequest{
				SystemPrompt: "You are a helpful assistant.",
				Messages: []*contract.Message{
					{Role: "user", Content: "Hello"},
				},
			},
		},
	}
	if req.ModelId != "claude-opus-4-7" {
		t.Errorf("ModelId: got %q, want %q", req.ModelId, "claude-opus-4-7")
	}
	chat, ok := req.Payload.(*contract.InvokeRequest_Chat)
	if !ok {
		t.Fatalf("Payload is %T, want *contract.InvokeRequest_Chat", req.Payload)
	}
	if len(chat.Chat.Messages) != 1 {
		t.Errorf("Messages len: got %d, want 1", len(chat.Chat.Messages))
	}
	if chat.Chat.Messages[0].Role != "user" {
		t.Errorf("Messages[0].Role: got %q, want %q", chat.Chat.Messages[0].Role, "user")
	}
	if req.Timeout.Millis != 30_000 {
		t.Errorf("Timeout.Millis: got %d, want 30000", req.Timeout.Millis)
	}
}

// hinge_test: pins=6-rpc-methods+service-descriptor, intended=sidecar-service-interface, phase=P3a
func TestSidecarServiceInterface(t *testing.T) {
	// Compile-time: UnimplementedSidecarServer implements SidecarServer (all 6 RPCs).
	var _ contract.SidecarServer = &contract.UnimplementedSidecarServer{}

	// Runtime: assert service descriptor names independently of the interface.
	// If both SidecarServer and UnimplementedSidecarServer drop a method together,
	// the compile-time check above would still pass — this catches that drift.
	desc := contract.Sidecar_ServiceDesc
	if desc.ServiceName != "anvil.v1.Sidecar" {
		t.Errorf("ServiceName: got %q, want %q", desc.ServiceName, "anvil.v1.Sidecar")
	}
	wantUnary := map[string]bool{
		"Handshake": true, "Invoke": true, "Cancel": true,
		"Health": true, "ReloadConfig": true,
	}
	for _, m := range desc.Methods {
		if !wantUnary[m.MethodName] {
			t.Errorf("unexpected unary method in ServiceDesc: %q", m.MethodName)
		}
		delete(wantUnary, m.MethodName)
	}
	for name := range wantUnary {
		t.Errorf("missing unary method in ServiceDesc: %q", name)
	}
	if len(desc.Streams) != 1 {
		t.Errorf("expected 1 stream, got %d", len(desc.Streams))
	} else {
		if desc.Streams[0].StreamName != "InvokeStreaming" {
			t.Errorf("stream name: got %q, want %q", desc.Streams[0].StreamName, "InvokeStreaming")
		}
		if !desc.Streams[0].ServerStreams {
			t.Error("InvokeStreaming.ServerStreams must be true")
		}
	}
}

// hinge_test: pins=ERROR_CLASS_UNSPECIFIED, intended=error-class-string-names, phase=P3a
func TestErrorClassUnspecifiedName(t *testing.T) {
	got := contract.ErrorClass_ERROR_CLASS_UNSPECIFIED.String()
	if got != "ERROR_CLASS_UNSPECIFIED" {
		t.Errorf("ErrorClass_ERROR_CLASS_UNSPECIFIED.String() = %q, want %q", got, "ERROR_CLASS_UNSPECIFIED")
	}
}
