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

// GoogleAIAdapter implements ProviderAdapter for the Google AI Studio Gemini API.
type GoogleAIAdapter struct{}

func (a GoogleAIAdapter) Invoke(ctx context.Context, conn *config.ProviderConnection, req *contract.InvokeRequest) (*contract.InvokeResponse, error) {
	key := req.IdempotencyKey

	chat := req.GetChat()
	if chat == nil {
		return &contract.InvokeResponse{
			IdempotencyKey: key,
			Result:         &contract.InvokeResponse_Error{Error: schemaErr("chat payload required for Google AI Studio")},
		}, nil
	}

	ak, err := apiKey(req.Credentials)
	if err != nil {
		return &contract.InvokeResponse{
			IdempotencyKey: key,
			Result:         &contract.InvokeResponse_Error{Error: schemaErr(err.Error())},
		}, nil
	}

	body, err := googleBody(chat)
	if err != nil {
		return &contract.InvokeResponse{
			IdempotencyKey: key,
			Result:         &contract.InvokeResponse_Error{Error: schemaErr(err.Error())},
		}, nil
	}

	if req.ModelId == "" {
		return &contract.InvokeResponse{
			IdempotencyKey: key,
			Result:         &contract.InvokeResponse_Error{Error: schemaErr("model_id is required")},
		}, nil
	}

	endpoint := resolveEndpoint(conn, "https://generativelanguage.googleapis.com")
	url := fmt.Sprintf("%s/v1beta/models/%s:generateContent", endpoint, req.ModelId)

	resp, err := doHTTP(ctx, http.MethodPost, url, googleHeaders(ak), body)
	if err != nil {
		return nil, fmt.Errorf("google HTTP: %w", err)
	}
	defer resp.Body.Close()

	data, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("google read body: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return &contract.InvokeResponse{
			IdempotencyKey: key,
			Result:         &contract.InvokeResponse_Error{Error: googleErrFromBody(resp.StatusCode, data)},
		}, nil
	}

	text, finishReason, usage, err := parseGoogleResponse(data)
	if err != nil {
		return nil, fmt.Errorf("google parse response: %w", err)
	}

	return &contract.InvokeResponse{
		IdempotencyKey: key,
		Result: &contract.InvokeResponse_Chat{Chat: &contract.ChatResponse{
			Content:      text,
			Model:        req.ModelId,
			FinishReason: finishReason,
			Usage:        usage,
		}},
	}, nil
}

func (a GoogleAIAdapter) InvokeStreaming(ctx context.Context, conn *config.ProviderConnection, req *contract.InvokeRequest, send EventSender) error {
	key := req.IdempotencyKey

	chat := req.GetChat()
	if chat == nil {
		return send(streamErrEvent(key, schemaErr("chat payload required for Google AI Studio")))
	}

	ak, err := apiKey(req.Credentials)
	if err != nil {
		return send(streamErrEvent(key, schemaErr(err.Error())))
	}

	if req.ModelId == "" {
		return send(streamErrEvent(key, schemaErr("model_id is required")))
	}

	body, err := googleBody(chat)
	if err != nil {
		return send(streamErrEvent(key, schemaErr(err.Error())))
	}

	endpoint := resolveEndpoint(conn, "https://generativelanguage.googleapis.com")
	// P4a: ?alt=sse requests SSE format. If Google API returns non-SSE (API change),
	// parseSSE produces no events and sends empty FinalResult. Add format detection if needed.
	url := fmt.Sprintf("%s/v1beta/models/%s:streamGenerateContent?alt=sse", endpoint, req.ModelId)

	resp, err := doHTTP(ctx, http.MethodPost, url, googleHeaders(ak), body)
	if err != nil {
		return fmt.Errorf("google HTTP: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		data, _ := io.ReadAll(resp.Body)
		return send(streamErrEvent(key, googleErrFromBody(resp.StatusCode, data)))
	}

	var (
		contentBuf   bytes.Buffer
		finishReason string
		inputTokens  int32
		outputTokens int32
	)

	err = parseSSE(bufio.NewReader(resp.Body), func(field, value string) error {
		if field == "event" {
			return nil
		}
		var chunk googleResponse
		if err := json.Unmarshal([]byte(value), &chunk); err != nil {
			return nil // skip malformed chunks
		}
		if chunk.Error != nil {
			ae := svcerrors.New(
				svcerrors.GoogleErrorClass(chunk.Error.Code),
				chunk.Error.Status,
				chunk.Error.Message,
			)
			if sendErr := send(streamErrEvent(key, ae)); sendErr != nil {
				return sendErr
			}
			return errStreamTerminated
		}
		if chunk.UsageMetadata != nil {
			inputTokens = chunk.UsageMetadata.PromptTokenCount
			outputTokens = chunk.UsageMetadata.CandidatesTokenCount
		}
		for _, c := range chunk.Candidates {
			if c.FinishReason != "" {
				finishReason = c.FinishReason
			}
			for _, part := range c.Content.Parts {
				if part.Text != "" {
					contentBuf.WriteString(part.Text)
					if err := send(tokenEvent(key, part.Text)); err != nil {
						return err
					}
				}
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
		Model:        req.ModelId,
		FinishReason: finishReason,
		Usage:        &contract.Usage{InputTokens: inputTokens, OutputTokens: outputTokens},
	}))
}

func googleHeaders(ak string) map[string]string {
	return map[string]string{
		"content-type":   "application/json",
		"x-goog-api-key": ak,
	}
}

func googleBody(chat *contract.ChatRequest) ([]byte, error) {
	type part struct {
		Text string `json:"text"`
	}
	type content struct {
		Role  string `json:"role"`
		Parts []part `json:"parts"`
	}

	var contents []content
	for _, m := range chat.Messages {
		role := m.Role
		if role == "assistant" {
			role = "model"
		}
		contents = append(contents, content{Role: role, Parts: []part{{Text: m.Content}}})
	}

	body := map[string]any{"contents": contents}

	genConfig := map[string]any{}
	if chat.MaxTokens != nil {
		genConfig["maxOutputTokens"] = *chat.MaxTokens
	}
	if chat.Temperature != nil {
		genConfig["temperature"] = *chat.Temperature
	}
	if len(genConfig) > 0 {
		body["generationConfig"] = genConfig
	}

	if chat.SystemPrompt != "" {
		body["systemInstruction"] = map[string]any{
			"parts": []part{{Text: chat.SystemPrompt}},
		}
	}

	return json.Marshal(body)
}

type googleResponse struct {
	Candidates []struct {
		Content struct {
			Parts []struct {
				Text string `json:"text"`
			} `json:"parts"`
			Role string `json:"role"`
		} `json:"content"`
		FinishReason string `json:"finishReason"`
	} `json:"candidates"`
	UsageMetadata *struct {
		PromptTokenCount     int32 `json:"promptTokenCount"`
		CandidatesTokenCount int32 `json:"candidatesTokenCount"`
	} `json:"usageMetadata"`
	Error *struct {
		Code    int    `json:"code"`
		Message string `json:"message"`
		Status  string `json:"status"`
	} `json:"error"`
}

func parseGoogleResponse(data []byte) (text, finishReason string, usage *contract.Usage, err error) {
	var r googleResponse
	if err = json.Unmarshal(data, &r); err != nil {
		return "", "", nil, fmt.Errorf("unmarshal: %w", err)
	}
	if r.Error != nil {
		return "", "", nil, fmt.Errorf("google error %d: %s", r.Error.Code, r.Error.Message)
	}
	var sb bytes.Buffer
	for _, c := range r.Candidates {
		if c.FinishReason != "" {
			finishReason = c.FinishReason
		}
		for _, p := range c.Content.Parts {
			sb.WriteString(p.Text)
		}
	}
	usage = &contract.Usage{}
	if r.UsageMetadata != nil {
		usage.InputTokens = r.UsageMetadata.PromptTokenCount
		usage.OutputTokens = r.UsageMetadata.CandidatesTokenCount
	}
	return sb.String(), finishReason, usage, nil
}

func googleErrFromBody(statusCode int, data []byte) *contract.AnvilError {
	var r googleResponse
	_ = json.Unmarshal(data, &r)
	if r.Error != nil {
		return svcerrors.New(svcerrors.GoogleErrorClass(r.Error.Code), r.Error.Status, r.Error.Message)
	}
	return svcerrors.UnexpectedStatus(statusCode, string(data))
}
