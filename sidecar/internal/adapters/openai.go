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

// OpenAIAdapter implements ProviderAdapter for the OpenAI Chat Completions API.
type OpenAIAdapter struct{}

func (a OpenAIAdapter) Invoke(ctx context.Context, conn *config.ProviderConnection, req *contract.InvokeRequest) (*contract.InvokeResponse, error) {
	key := req.IdempotencyKey

	chat := req.GetChat()
	if chat == nil {
		return &contract.InvokeResponse{
			IdempotencyKey: key,
			Result:         &contract.InvokeResponse_Error{Error: schemaErr("chat payload required for OpenAI")},
		}, nil
	}

	ak, err := apiKey(req.Credentials)
	if err != nil {
		return &contract.InvokeResponse{
			IdempotencyKey: key,
			Result:         &contract.InvokeResponse_Error{Error: schemaErr(err.Error())},
		}, nil
	}

	body, err := openaiBody(req.ModelId, chat, false)
	if err != nil {
		return &contract.InvokeResponse{
			IdempotencyKey: key,
			Result:         &contract.InvokeResponse_Error{Error: schemaErr(err.Error())},
		}, nil
	}

	endpoint := resolveEndpoint(conn, "https://api.openai.com")
	resp, err := doHTTP(ctx, http.MethodPost, endpoint+"/v1/chat/completions", openaiHeaders(ak), body)
	if err != nil {
		return nil, fmt.Errorf("openai HTTP: %w", err)
	}
	defer resp.Body.Close()

	data, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("openai read body: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return &contract.InvokeResponse{
			IdempotencyKey: key,
			Result:         &contract.InvokeResponse_Error{Error: openaiErrFromBody(resp.StatusCode, data)},
		}, nil
	}

	var msg struct {
		Model   string `json:"model"`
		Choices []struct {
			Message struct {
				Content string `json:"content"`
			} `json:"message"`
			FinishReason string `json:"finish_reason"`
		} `json:"choices"`
		Usage struct {
			PromptTokens     int32 `json:"prompt_tokens"`
			CompletionTokens int32 `json:"completion_tokens"`
		} `json:"usage"`
	}
	if err := json.Unmarshal(data, &msg); err != nil {
		return nil, fmt.Errorf("openai parse response: %w", err)
	}
	if len(msg.Choices) == 0 {
		return nil, fmt.Errorf("openai: empty choices in response")
	}

	return &contract.InvokeResponse{
		IdempotencyKey: key,
		Result: &contract.InvokeResponse_Chat{Chat: &contract.ChatResponse{
			Content:      msg.Choices[0].Message.Content,
			Model:        msg.Model,
			FinishReason: msg.Choices[0].FinishReason,
			Usage:        &contract.Usage{InputTokens: msg.Usage.PromptTokens, OutputTokens: msg.Usage.CompletionTokens},
		}},
	}, nil
}

func (a OpenAIAdapter) InvokeStreaming(ctx context.Context, conn *config.ProviderConnection, req *contract.InvokeRequest, send EventSender) error {
	key := req.IdempotencyKey

	chat := req.GetChat()
	if chat == nil {
		return send(streamErrEvent(key, schemaErr("chat payload required for OpenAI")))
	}

	ak, err := apiKey(req.Credentials)
	if err != nil {
		return send(streamErrEvent(key, schemaErr(err.Error())))
	}

	body, err := openaiBody(req.ModelId, chat, true)
	if err != nil {
		return send(streamErrEvent(key, schemaErr(err.Error())))
	}

	endpoint := resolveEndpoint(conn, "https://api.openai.com")
	resp, err := doHTTP(ctx, http.MethodPost, endpoint+"/v1/chat/completions", openaiHeaders(ak), body)
	if err != nil {
		return fmt.Errorf("openai HTTP: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		data, _ := io.ReadAll(resp.Body)
		return send(streamErrEvent(key, openaiErrFromBody(resp.StatusCode, data)))
	}

	var (
		contentBuf   bytes.Buffer
		model        string
		finishReason string
		inputTokens  int32
		outputTokens int32
	)

	err = parseSSE(bufio.NewReader(resp.Body), func(field, value string) error {
		if field == "event" {
			return nil // OpenAI SSE uses only data: lines
		}
		if value == "[DONE]" {
			return errStreamTerminated
		}
		var chunk struct {
			Model   string `json:"model"`
			Choices []struct {
				Delta struct {
					Content string `json:"content"`
				} `json:"delta"`
				FinishReason *string `json:"finish_reason"`
			} `json:"choices"`
			Usage *struct {
				PromptTokens     int32 `json:"prompt_tokens"`
				CompletionTokens int32 `json:"completion_tokens"`
			} `json:"usage"`
		}
		if err := json.Unmarshal([]byte(value), &chunk); err != nil {
			return nil // skip malformed chunks
		}
		if model == "" && chunk.Model != "" {
			model = chunk.Model
		}
		if chunk.Usage != nil {
			inputTokens = chunk.Usage.PromptTokens
			outputTokens = chunk.Usage.CompletionTokens
		}
		if len(chunk.Choices) > 0 {
			if chunk.Choices[0].FinishReason != nil && *chunk.Choices[0].FinishReason != "" {
				finishReason = *chunk.Choices[0].FinishReason
			}
			text := chunk.Choices[0].Delta.Content
			if text != "" {
				contentBuf.WriteString(text)
				if err := send(tokenEvent(key, text)); err != nil {
					return err
				}
			}
		}
		return nil
	})

	if err == errStreamTerminated {
		return send(finalChatEvent(key, &contract.ChatResponse{
			Content:      contentBuf.String(),
			Model:        model,
			FinishReason: finishReason,
			Usage:        &contract.Usage{InputTokens: inputTokens, OutputTokens: outputTokens},
		}))
	}
	if err != nil {
		return err
	}

	// P4a: stream ended without [DONE] — response may be truncated. Send best-effort FinalResult.
	return send(finalChatEvent(key, &contract.ChatResponse{
		Content:      contentBuf.String(),
		Model:        model,
		FinishReason: finishReason,
		Usage:        &contract.Usage{InputTokens: inputTokens, OutputTokens: outputTokens},
	}))
}

func openaiHeaders(apiKey string) map[string]string {
	return map[string]string{
		"authorization": "Bearer " + apiKey,
		"content-type":  "application/json",
	}
}

func openaiBody(modelID string, chat *contract.ChatRequest, stream bool) ([]byte, error) {
	if modelID == "" {
		return nil, fmt.Errorf("model_id is required")
	}

	type message struct {
		Role    string `json:"role"`
		Content string `json:"content"`
	}
	var msgs []message
	if chat.SystemPrompt != "" {
		msgs = append(msgs, message{Role: "system", Content: chat.SystemPrompt})
	}
	for _, m := range chat.Messages {
		msgs = append(msgs, message{Role: m.Role, Content: m.Content})
	}

	body := map[string]any{
		"model":    modelID,
		"messages": msgs,
		"stream":   stream,
	}
	if stream {
		body["stream_options"] = map[string]any{"include_usage": true}
	}
	if chat.MaxTokens != nil {
		body["max_tokens"] = *chat.MaxTokens
	}
	if chat.Temperature != nil {
		body["temperature"] = *chat.Temperature
	}

	return json.Marshal(body)
}

func openaiErrFromBody(statusCode int, data []byte) *contract.AnvilError {
	var errBody struct {
		Error struct {
			Type    string `json:"type"`
			Message string `json:"message"`
			Code    string `json:"code"`
		} `json:"error"`
	}
	_ = json.Unmarshal(data, &errBody)
	class := svcerrors.OpenAIErrorClass(errBody.Error.Type)
	if errBody.Error.Type == "" {
		class = svcerrors.FromHTTPStatus(statusCode)
	}
	msg := errBody.Error.Message
	if msg == "" {
		msg = string(data)
	}
	vendorCode := errBody.Error.Code
	if vendorCode == "" {
		vendorCode = errBody.Error.Type
	}
	return svcerrors.New(class, vendorCode, msg)
}
