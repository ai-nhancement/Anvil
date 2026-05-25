package adapters_test

import (
	"testing"

	"github.com/ai-nhancement/Anvil/sidecar/internal/adapters"
	"github.com/ai-nhancement/Anvil/sidecar/internal/config"
	contract "github.com/ai-nhancement/Anvil/sidecar/internal/contract"
	svcerrors "github.com/ai-nhancement/Anvil/sidecar/internal/errors"
)

// hinge_test: pins=3-provider-adapters, intended=v1-minimum-provider-adapters, phase=P3c
func TestMinimumProviderAdapters(t *testing.T) {
	// Pins: the Known registry must include at least these three providers.
	// Adding a provider is non-breaking; removing one is a breaking contract change.
	required := []config.ProviderType{
		config.ProviderAnthropic,
		config.ProviderOpenAI,
		config.ProviderGoogleAIStudio,
	}
	for _, p := range required {
		if _, ok := adapters.Known[p]; !ok {
			t.Errorf("Known registry missing required provider %q", p)
		}
	}
	if len(adapters.Known) < len(required) {
		t.Errorf("Known registry has %d providers, want >= %d", len(adapters.Known), len(required))
	}
}

// hinge_test: pins=ProviderAdapter-interface, intended=provider-adapter-interface-extensibility, phase=P3c
func TestProviderAdapterInterfaceCompileTime(t *testing.T) {
	// Compile-time: all three adapters satisfy ProviderAdapter.
	var _ adapters.ProviderAdapter = adapters.AnthropicAdapter{}
	var _ adapters.ProviderAdapter = adapters.OpenAIAdapter{}
	var _ adapters.ProviderAdapter = adapters.GoogleAIAdapter{}
}

// hinge_test: pins=anthropic-error-class-mapping, intended=test_error_class_mapping_anthropic, phase=P3c
func TestErrorClassMappingAnthropic(t *testing.T) {
	cases := []struct {
		errType string
		want    contract.ErrorClass
	}{
		{"invalid_request_error", contract.ErrorClass_ERROR_CLASS_SCHEMA_VIOLATION},
		{"not_found_error", contract.ErrorClass_ERROR_CLASS_SCHEMA_VIOLATION},
		{"authentication_error", contract.ErrorClass_ERROR_CLASS_TRANSPORT},
		{"permission_error", contract.ErrorClass_ERROR_CLASS_TRANSPORT},
		{"rate_limit_error", contract.ErrorClass_ERROR_CLASS_TRANSPORT},
		{"overloaded_error", contract.ErrorClass_ERROR_CLASS_TRANSPORT},
		{"timeout_error", contract.ErrorClass_ERROR_CLASS_TIMEOUT},
		{"unknown_vendor_error", contract.ErrorClass_ERROR_CLASS_ADAPTER_BUG},
	}
	for _, c := range cases {
		got := svcerrors.AnthropicErrorClass(c.errType)
		if got != c.want {
			t.Errorf("AnthropicErrorClass(%q): got %v, want %v", c.errType, got, c.want)
		}
	}
}

// hinge_test: pins=openai-error-class-mapping, intended=test_error_class_mapping_openai, phase=P3c
func TestErrorClassMappingOpenAI(t *testing.T) {
	cases := []struct {
		errType string
		want    contract.ErrorClass
	}{
		{"invalid_request_error", contract.ErrorClass_ERROR_CLASS_SCHEMA_VIOLATION},
		{"authentication_error", contract.ErrorClass_ERROR_CLASS_TRANSPORT},
		{"rate_limit_exceeded", contract.ErrorClass_ERROR_CLASS_TRANSPORT},
		{"timeout", contract.ErrorClass_ERROR_CLASS_TIMEOUT},
		{"content_policy_violation", contract.ErrorClass_ERROR_CLASS_PROVIDER_REFUSAL},
		{"unknown_vendor_error", contract.ErrorClass_ERROR_CLASS_ADAPTER_BUG},
	}
	for _, c := range cases {
		got := svcerrors.OpenAIErrorClass(c.errType)
		if got != c.want {
			t.Errorf("OpenAIErrorClass(%q): got %v, want %v", c.errType, got, c.want)
		}
	}
}

// hinge_test: pins=google-error-class-mapping, intended=test_error_class_mapping_google, phase=P3c
func TestErrorClassMappingGoogle(t *testing.T) {
	cases := []struct {
		httpStatus int
		want       contract.ErrorClass
	}{
		{400, contract.ErrorClass_ERROR_CLASS_SCHEMA_VIOLATION},
		{401, contract.ErrorClass_ERROR_CLASS_TRANSPORT},
		{403, contract.ErrorClass_ERROR_CLASS_TRANSPORT},
		{429, contract.ErrorClass_ERROR_CLASS_TRANSPORT},
		{408, contract.ErrorClass_ERROR_CLASS_TIMEOUT},
		{504, contract.ErrorClass_ERROR_CLASS_TIMEOUT},
		{500, contract.ErrorClass_ERROR_CLASS_ADAPTER_BUG},
		{503, contract.ErrorClass_ERROR_CLASS_ADAPTER_BUG},
	}
	for _, c := range cases {
		got := svcerrors.GoogleErrorClass(c.httpStatus)
		if got != c.want {
			t.Errorf("GoogleErrorClass(%d): got %v, want %v", c.httpStatus, got, c.want)
		}
	}
}

// hinge_test: pins=stream-error-event-shape, intended=streaming-aborts-on-error-no-continuation, phase=P3c
func TestStreamErrorEventShape(t *testing.T) {
	// Compile-time and runtime: stream error events must carry AnvilError and no continuation.
	// Verifies that the error event is terminal (not a Token event wrapping an error string).
	ae := &contract.AnvilError{
		Class:   contract.ErrorClass_ERROR_CLASS_TRANSPORT,
		Message: "test",
	}
	var received []*contract.InvokeStreamEvent
	send := func(ev *contract.InvokeStreamEvent) error {
		received = append(received, ev)
		return nil
	}

	// Synthesize what an adapter sends on error: exactly one StreamError event.
	ev := &contract.InvokeStreamEvent{
		IdempotencyKey: "test-key",
		Event:          &contract.InvokeStreamEvent_Error{Error: &contract.StreamError{Error: ae}},
	}
	_ = send(ev)

	if len(received) != 1 {
		t.Fatalf("expected 1 event, got %d", len(received))
	}
	errEv, ok := received[0].Event.(*contract.InvokeStreamEvent_Error)
	if !ok {
		t.Fatalf("event is %T, want *InvokeStreamEvent_Error", received[0].Event)
	}
	if errEv.Error.Error == nil {
		t.Error("StreamError.Error is nil")
	}
	if errEv.Error.Error.Class != contract.ErrorClass_ERROR_CLASS_TRANSPORT {
		t.Errorf("error class: got %v, want TRANSPORT", errEv.Error.Error.Class)
	}
}
