// Package errors provides typed error construction for sidecar responses.
package errors

import (
	"fmt"
	"net/http"

	contract "github.com/ai-nhancement/Anvil/sidecar/internal/contract"
)

// New builds a typed AnvilError.
func New(class contract.ErrorClass, vendorCode, msg string) *contract.AnvilError {
	return &contract.AnvilError{
		Class:      class,
		VendorCode: vendorCode,
		Message:    msg,
	}
}

// UnexpectedStatus builds an AnvilError for an unexpected HTTP status response.
func UnexpectedStatus(code int, body string) *contract.AnvilError {
	return New(FromHTTPStatus(code), fmt.Sprintf("HTTP %d", code), body)
}

// FromHTTPStatus maps an HTTP status code to the closest Anvil ErrorClass.
func FromHTTPStatus(code int) contract.ErrorClass {
	switch {
	case code == http.StatusUnauthorized || code == http.StatusForbidden:
		return contract.ErrorClass_ERROR_CLASS_TRANSPORT
	case code == http.StatusTooManyRequests:
		return contract.ErrorClass_ERROR_CLASS_TRANSPORT
	case code == http.StatusRequestTimeout || code == http.StatusGatewayTimeout:
		return contract.ErrorClass_ERROR_CLASS_TIMEOUT
	case code >= 400 && code < 500:
		return contract.ErrorClass_ERROR_CLASS_SCHEMA_VIOLATION
	default:
		return contract.ErrorClass_ERROR_CLASS_ADAPTER_BUG
	}
}

// AnthropicErrorClass maps an Anthropic API error type string to an ErrorClass.
func AnthropicErrorClass(errType string) contract.ErrorClass {
	switch errType {
	case "invalid_request_error", "not_found_error":
		return contract.ErrorClass_ERROR_CLASS_SCHEMA_VIOLATION
	case "authentication_error", "permission_error":
		return contract.ErrorClass_ERROR_CLASS_TRANSPORT
	case "rate_limit_error", "overloaded_error":
		return contract.ErrorClass_ERROR_CLASS_TRANSPORT
	case "timeout_error":
		return contract.ErrorClass_ERROR_CLASS_TIMEOUT
	default:
		return contract.ErrorClass_ERROR_CLASS_ADAPTER_BUG
	}
}

// OpenAIErrorClass maps an OpenAI API error type string to an ErrorClass.
func OpenAIErrorClass(errType string) contract.ErrorClass {
	switch errType {
	case "invalid_request_error":
		return contract.ErrorClass_ERROR_CLASS_SCHEMA_VIOLATION
	case "authentication_error":
		return contract.ErrorClass_ERROR_CLASS_TRANSPORT
	case "rate_limit_exceeded":
		return contract.ErrorClass_ERROR_CLASS_TRANSPORT
	case "timeout":
		return contract.ErrorClass_ERROR_CLASS_TIMEOUT
	case "content_policy_violation":
		return contract.ErrorClass_ERROR_CLASS_PROVIDER_REFUSAL
	default:
		return contract.ErrorClass_ERROR_CLASS_ADAPTER_BUG
	}
}

// GoogleErrorClass maps a Google API HTTP status code to an ErrorClass.
func GoogleErrorClass(httpStatus int) contract.ErrorClass {
	return FromHTTPStatus(httpStatus)
}
