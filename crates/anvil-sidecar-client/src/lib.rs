#[allow(clippy::all, clippy::pedantic)]
pub mod proto {
    tonic::include_proto!("anvil.v1");
}

#[cfg(test)]
mod tests {
    use super::proto;

    // hinge_test: pins=anvil.v1, intended=proto-package-version, phase=P3a
    #[test]
    fn test_proto_package_version() {
        // Reads the canonical proto source — stronger than a hand-written constant.
        // Breaks if the proto package declaration changes, even if bootstrap files are stale.
        let proto_src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/anvil/v1/sidecar.proto"
        ))
        .expect("failed to read proto/anvil/v1/sidecar.proto");
        assert!(
            proto_src.contains("package anvil.v1;"),
            "proto/anvil/v1/sidecar.proto does not declare 'package anvil.v1;'"
        );
    }

    // hinge_test: pins=ERROR_CLASS_UNSPECIFIED, intended=error-class-string-names, phase=P3a
    #[test]
    fn test_error_class_unspecified_name() {
        assert_eq!(
            proto::ErrorClass::Unspecified.as_str_name(),
            "ERROR_CLASS_UNSPECIFIED"
        );
    }

    // hinge_test: pins=6+discriminants, intended=error-class-count, phase=P3a
    #[test]
    fn test_error_class_count() {
        // Pins discriminant values — any change is a breaking wire-format change.
        assert_eq!(proto::ErrorClass::Unspecified as i32, 0);
        assert_eq!(proto::ErrorClass::Transport as i32, 1);
        assert_eq!(proto::ErrorClass::ProviderRefusal as i32, 2);
        assert_eq!(proto::ErrorClass::SchemaViolation as i32, 3);
        assert_eq!(proto::ErrorClass::AdapterBug as i32, 4);
        assert_eq!(proto::ErrorClass::Timeout as i32, 5);
        assert_eq!(proto::ErrorClass::Cancelled as i32, 6);
        let non_unspecified = [
            proto::ErrorClass::Transport,
            proto::ErrorClass::ProviderRefusal,
            proto::ErrorClass::SchemaViolation,
            proto::ErrorClass::AdapterBug,
            proto::ErrorClass::Timeout,
            proto::ErrorClass::Cancelled,
        ];
        assert_eq!(non_unspecified.len(), 6);
    }

    // hinge_test: pins=core_protocol_version+supported_versions, intended=handshake-required-fields, phase=P3a
    #[test]
    fn test_handshake_required_fields() {
        // Structural smoke test: pins field names and presence at compile time.
        let req = proto::HandshakeRequest {
            core_protocol_version: "v1".into(),
            supported_versions: vec!["v1".into()],
            vault_config_epoch: String::new(),
        };
        assert_eq!(req.core_protocol_version, "v1");
        assert!(!req.supported_versions.is_empty());
    }

    // hinge_test: pins=invoke_request::Payload::Chat+chat_request_shape, intended=invoke-chat-oneof-shape, phase=P3a
    #[test]
    fn test_invoke_request_chat_payload() {
        let req = proto::InvokeRequest {
            idempotency_key: "00000000-0000-7000-8000-000000000001".into(),
            model_id: "claude-opus-4-7".into(),
            provider_connection_id: "anthropic-prod".into(),
            credentials: Some(proto::Credentials {
                credential: Some(proto::credentials::Credential::ApiKey("sk-test".into())),
            }),
            timeout: Some(proto::Timeout { millis: 30_000 }),
            payload: Some(proto::invoke_request::Payload::Chat(proto::ChatRequest {
                system_prompt: "You are a helpful assistant.".into(),
                messages: vec![proto::Message {
                    role: "user".into(),
                    content: "Hello".into(),
                }],
                max_tokens: Some(1024),
                temperature: None,
            })),
        };
        assert_eq!(req.model_id, "claude-opus-4-7");
        assert!(matches!(
            req.payload,
            Some(proto::invoke_request::Payload::Chat(_))
        ));
        if let Some(proto::invoke_request::Payload::Chat(ref chat)) = req.payload {
            assert_eq!(chat.messages.len(), 1);
            assert_eq!(chat.messages[0].role, "user");
        }
        assert_eq!(req.timeout.as_ref().unwrap().millis, 30_000);
    }
}
