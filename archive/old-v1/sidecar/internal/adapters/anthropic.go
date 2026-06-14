package adapters

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"

	contract "github.com/ai-nhancement/Anvil/sidecar/internal/contract"
	"github.com/ai-nhancement/Anvil/sidecar/internal/config"
	svcerrors "github.com/ai-nhancement/Anvil/sidecar/internal/errors"
)

// AnthropicAdapter implements ProviderAdapter for the Anthropic Messages API.
type AnthropicAdapter struct{}

func (a AnthropicAdapter) Invoke(ctx context.Context, conn *config.ProviderConnection, req *contract.InvokeRequest) (*contract.InvokeResponse, error) {
	key := req.IdempotencyKey

	chat := req.GetChat()
	if chat == nil {
		return &contract.InvokeResponse{
			IdempotencyKey: key,
			Result:         &contract.InvokeResponse_Error{Error: schemaErr("chat payload required for Anthropic")},
		}, nil
	}

	ak, err := apiKey(req.Credentials)
	if err != nil {
		return &contract.InvokeResponse{
			IdempotencyKey: key,
			Result:         &contract.InvokeResponse_Error{Error: schemaErr(err.Error())},
		}, nil
	}

	body, err := anthropicBody(req.ModelId, chat, false)
	if err != nil {
		return &contract.InvokeResponse{
			IdempotencyKey: key,
			Result:         &contract.InvokeResponse_Error{Error: schemaErr(err.Error())},
		}, nil
	}

	endpoint := resolveEndpoint(conn, "https://api.anthropic.com")
	resp, err := doHTTP(ctx, http.MethodPost, endpoint+"/v1/messages", anthropicHeaders(ak), body)
	if err != nil {
		return nil, fmt.Errorf("anthropic HTTP: %w", err)
	}
	defer resp.Body.Close()

	data, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("anthropic read body: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return &contract.InvokeResponse{
			IdempotencyKey: key,
			Result:         &contract.InvokeResponse_Error{Error: anthropicErrFromBody(resp.StatusCode, data)},
		}, nil
	}

	var msg struct {
		Content []struct {
			Type string `json:"type"`
			Text string `json:"text"`
		} `json:"content"`
		Model      string `json:"model"`
		StopReason string `json:"stop_reason"`
		Usage      struct {
			InputTokens  int32 `json:"input_tokens"`
			OutputTokens int32 `json:"output_tokens"`
		} `json:"usage"`
	}
	if err := json.Unmarshal(data, &msg); err != nil {
		return nil, fmt.Errorf("anthropic parse response: %w", err)
	}

	var sb bytes.Buffer
	for _, c := range msg.Content {
		if c.Type == "text" {
			sb.WriteString(c.Text)
		}
	}

	return &contract.InvokeResponse{
		IdempotencyKey: key,
		Result: &contract.InvokeResponse_Chat{Chat: &contract.ChatResponse{
			Content:      sb.String(),
			Model:        msg.Model,
			FinishReason: msg.StopReason,
			Usage:        &contract.Usage{InputTokens: msg.Usage.InputTokens, OutputTokens: msg.Usage.OutputTokens},
		}},
	}, nil
}

func (a AnthropicAdapter) InvokeStreaming(ctx context.Context, conn *config.ProviderConnection, req *contract.InvokeRequest, send EventSender) error {
	key := req.IdempotencyKey

	chat := req.GetChat()
	if chat == nil {
		return send(streamErrEvent(key, schemaErr("chat payload required for Anthropic")))
	}

	ak, err := apiKey(req.Credentials)
	if err != nil {
		return send(streamErrEvent(key, schemaErr(err.Error())))
	}

	body, err := anthropicBody(req.ModelId, chat, true)
	if err != nil {
		return send(streamErrEvent(key, schemaErr(err.Error())))
	}

	endpoint := resolveEndpoint(conn, "https://api.anthropic.com")
	resp, err := doHTTP(ctx, http.MethodPost, endpoint+"/v1/messages", anthropicHeaders(ak), body)
	if err != nil {
		return fmt.Errorf("anthropic HTTP: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		data, _ := io.ReadAll(resp.Body)
		return send(streamErrEvent(key, anthropicErrFromBody(resp.StatusCode, data)))
	}

	var (
		currentEvent  string
		inputTokens   int32
		outputTokens  int32
		model         string
		finishReason  string
		contentBuf    bytes.Buffer
	)

	err = parseSSE(bufio.NewReader(resp.Body), func(field, value string) error {
		if field == "event" {
			currentEvent = value
			return nil
		}
		// field == "data"
		switch currentEvent {
		case "message_start":
			var p struct {
				Message struct {
					Model string `json:"model"`
					Usage struct {
						InputTokens int32 `json:"input_tokens"`
					} `json:"usage"`
				} `json:"message"`
			}
			if json.Unmarshal([]byte(value), &p) == nil {
				model = p.Message.Model
				inputTokens = p.Message.Usage.InputTokens
			}
		case "content_block_delta":
			var p struct {
				Delta struct {
					Type string `json:"type"`
					Text string `json:"text"`
				} `json:"delta"`
			}
			if json.Unmarshal([]byte(value), &p) == nil && p.Delta.Type == "text_delta" && p.Delta.Text != "" {
				contentBuf.WriteString(p.Delta.Text)
				if err := send(tokenEvent(key, p.Delta.Text)); err != nil {
					return err
				}
			}
		case "message_delta":
			var p struct {
				Delta struct {
					StopReason string `json:"stop_reason"`
				} `json:"delta"`
				Usage struct {
					OutputTokens int32 `json:"output_tokens"`
				} `json:"usage"`
			}
			if json.Unmarshal([]byte(value), &p) == nil {
				finishReason = p.Delta.StopReason
				outputTokens = p.Usage.OutputTokens
			}
		case "error":
			var p struct {
				Error struct {
					Type    string `json:"type"`
					Message string `json:"message"`
				} `json:"error"`
			}
			if json.Unmarshal([]byte(value), &p) == nil {
				ae := svcerrors.New(svcerrors.AnthropicErrorClass(p.Error.Type), p.Error.Type, p.Error.Message)
				if sendErr := send(streamErrEvent(key, ae)); sendErr != nil {
					return sendErr
				}
				return errStreamTerminated
			}
		}
		return nil
	})

	if err == errStreamTerminated {
		return nil
	}
	if err != nil {
		return err
	}

	return send(finalChatEvent(key, &contract.ChatResponse{
		Content:      contentBuf.String(),
		Model:        model,
		FinishReason: finishReason,
		Usage:        &contract.Usage{InputTokens: inputTokens, OutputTokens: outputTokens},
	}))
}

func anthropicHeaders(apiKey string) map[string]string {
	return map[string]string{
		"x-api-key":         apiKey,
		"anthropic-version": "2023-06-01",
		"content-type":      "application/json",
	}
}

func anthropicBody(modelID string, chat *contract.ChatRequest, stream bool) ([]byte, error) {
	if modelID == "" {
		return nil, fmt.Errorf("model_id is required")
	}

	type message struct {
		Role    string `json:"role"`
		Content string `json:"content"`
	}
	msgs := make([]message, 0, len(chat.Messages))
	for _, m := range chat.Messages {
		msgs = append(msgs, message{Role: m.Role, Content: m.Content})
	}

	body := map[string]any{
		"model":    modelID,
		"messages": msgs,
		"stream":   stream,
	}
	if chat.MaxTokens != nil {
		body["max_tokens"] = *chat.MaxTokens
	} else {
		body["max_tokens"] = 4096 // Anthropic requires max_tokens
	}
	if chat.SystemPrompt != "" {
		body["system"] = chat.SystemPrompt
	}
	if chat.Temperature != nil {
		body["temperature"] = *chat.Temperature
	}

	return json.Marshal(body)
}

func anthropicErrFromBody(statusCode int, data []byte) *contract.AnvilError {
	var errBody struct {
		Error struct {
			Type    string `json:"type"`
			Message string `json:"message"`
		} `json:"error"`
	}
	_ = json.Unmarshal(data, &errBody)
	class := svcerrors.AnthropicErrorClass(errBody.Error.Type)
	if errBody.Error.Type == "" {
		class = svcerrors.FromHTTPStatus(statusCode)
	}
	msg := errBody.Error.Message
	if msg == "" {
		msg = string(data)
	}
	return svcerrors.New(class, errBody.Error.Type, msg)
}
