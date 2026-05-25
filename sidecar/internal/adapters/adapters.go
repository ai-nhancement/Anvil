// Package adapters contains vendor-specific API adapters and shared infrastructure.
package adapters

import (
	"bufio"
	"bytes"
	"context"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	contract "github.com/ai-nhancement/Anvil/sidecar/internal/contract"
	"github.com/ai-nhancement/Anvil/sidecar/internal/config"
	svcerrors "github.com/ai-nhancement/Anvil/sidecar/internal/errors"
)

// EventSender is the callback the server passes to InvokeStreaming to emit stream events.
type EventSender func(*contract.InvokeStreamEvent) error

// ProviderAdapter is the interface each vendor adapter must implement.
type ProviderAdapter interface {
	// Invoke performs a unary request.
	// Transport failures return (nil, err).
	// Vendor-level errors (HTTP 4xx/5xx) return (response, nil) with result.error set.
	Invoke(ctx context.Context, conn *config.ProviderConnection, req *contract.InvokeRequest) (*contract.InvokeResponse, error)

	// InvokeStreaming performs a streaming request.
	// Token events are sent via send as they arrive.
	// On success, sends a FinalResult event and returns nil.
	// On vendor error, sends a StreamError event and returns nil.
	// On transport failure, returns a non-nil error without sending a terminal event.
	InvokeStreaming(ctx context.Context, conn *config.ProviderConnection, req *contract.InvokeRequest, send EventSender) error
}

// Known maps each ProviderType to its singleton adapter instance.
// Adding a new provider requires registering it here (hinge-tested).
var Known = map[config.ProviderType]ProviderAdapter{
	config.ProviderAnthropic:      AnthropicAdapter{},
	config.ProviderOpenAI:         OpenAIAdapter{},
	config.ProviderGoogleAIStudio: GoogleAIAdapter{},
}

// errStreamTerminated is returned from an SSE callback to stop parsing after
// a terminal stream-error event was successfully sent.
var errStreamTerminated = fmt.Errorf("stream terminated by error event")

// P4a: 120-second client timeout caps any per-request context deadline longer than 120s.
// Remove or raise this timeout and let context deadlines govern entirely.
var httpClient = &http.Client{Timeout: 120 * time.Second}

// doHTTP executes an HTTP request with context propagation. The caller must close resp.Body.
func doHTTP(ctx context.Context, method, url string, headers map[string]string, body []byte) (*http.Response, error) {
	var bodyReader io.Reader
	if body != nil {
		bodyReader = bytes.NewReader(body)
	}
	req, err := http.NewRequestWithContext(ctx, method, url, bodyReader)
	if err != nil {
		return nil, fmt.Errorf("build request: %w", err)
	}
	for k, v := range headers {
		req.Header.Set(k, v)
	}
	return httpClient.Do(req)
}

// parseSSE reads an SSE stream from r and calls fn("event", name) and fn("data", value)
// for each field line. Returns when the stream ends or fn returns an error.
func parseSSE(r *bufio.Reader, fn func(field, value string) error) error {
	for {
		line, err := r.ReadString('\n')
		line = strings.TrimRight(line, "\r\n")

		if line != "" {
			if idx := strings.Index(line, ":"); idx >= 0 {
				field := strings.TrimSpace(line[:idx])
				value := strings.TrimPrefix(line[idx+1:], " ")
				if field == "event" || field == "data" {
					if ferr := fn(field, value); ferr != nil {
						return ferr
					}
				}
			}
		}

		if err != nil {
			if err == io.EOF {
				return nil
			}
			return fmt.Errorf("read SSE: %w", err)
		}
	}
}

func apiKey(creds *contract.Credentials) (string, error) {
	if creds == nil {
		return "", fmt.Errorf("credentials absent")
	}
	ak, ok := creds.Credential.(*contract.Credentials_ApiKey)
	if !ok || ak.ApiKey == "" {
		return "", fmt.Errorf("api_key credential required for this provider")
	}
	return ak.ApiKey, nil
}

func resolveEndpoint(conn *config.ProviderConnection, defaultURL string) string {
	if conn.Endpoint != "" {
		return conn.Endpoint
	}
	return defaultURL
}

func schemaErr(msg string) *contract.AnvilError {
	return svcerrors.New(contract.ErrorClass_ERROR_CLASS_SCHEMA_VIOLATION, "", msg)
}

func tokenEvent(key, text string) *contract.InvokeStreamEvent {
	return &contract.InvokeStreamEvent{
		IdempotencyKey: key,
		Event:          &contract.InvokeStreamEvent_Token{Token: &contract.Token{Text: text}},
	}
}

func finalChatEvent(key string, chat *contract.ChatResponse) *contract.InvokeStreamEvent {
	return &contract.InvokeStreamEvent{
		IdempotencyKey: key,
		Event: &contract.InvokeStreamEvent_FinalResult{
			FinalResult: &contract.FinalResult{Result: &contract.FinalResult_Chat{Chat: chat}},
		},
	}
}

func streamErrEvent(key string, ae *contract.AnvilError) *contract.InvokeStreamEvent {
	return &contract.InvokeStreamEvent{
		IdempotencyKey: key,
		Event:          &contract.InvokeStreamEvent_Error{Error: &contract.StreamError{Error: ae}},
	}
}
