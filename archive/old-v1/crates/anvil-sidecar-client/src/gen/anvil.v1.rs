// Generated from proto/anvil/v1/sidecar.proto (anvil.v1 package).
// Regenerate with `just gen-rust` when the proto changes.
// @generated

// ── Handshake ────────────────────────────────────────────────────────────────

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HandshakeRequest {
    #[prost(string, tag = "1")]
    pub core_protocol_version: ::prost::alloc::string::String,
    #[prost(string, repeated, tag = "2")]
    pub supported_versions: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag = "3")]
    pub vault_config_epoch: ::prost::alloc::string::String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HandshakeResponse {
    #[prost(string, tag = "1")]
    pub negotiated_version: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub sidecar_version: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub sidecar_build_info: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub sidecar_config_epoch: ::prost::alloc::string::String,
}

// ── Invoke ───────────────────────────────────────────────────────────────────

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InvokeRequest {
    #[prost(string, tag = "1")]
    pub idempotency_key: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub model_id: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub provider_connection_id: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "4")]
    pub credentials: ::core::option::Option<Credentials>,
    #[prost(message, optional, tag = "7")]
    pub timeout: ::core::option::Option<Timeout>,
    #[prost(oneof = "invoke_request::Payload", tags = "5, 6")]
    pub payload: ::core::option::Option<invoke_request::Payload>,
}
/// Nested message and enum types in `InvokeRequest`.
pub mod invoke_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Payload {
        #[prost(message, tag = "5")]
        Chat(super::ChatRequest),
        #[prost(message, tag = "6")]
        Embed(super::EmbedRequest),
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InvokeResponse {
    #[prost(string, tag = "1")]
    pub idempotency_key: ::prost::alloc::string::String,
    #[prost(oneof = "invoke_response::Result", tags = "2, 3, 4")]
    pub result: ::core::option::Option<invoke_response::Result>,
}
/// Nested message and enum types in `InvokeResponse`.
pub mod invoke_response {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "2")]
        Chat(super::ChatResponse),
        #[prost(message, tag = "3")]
        Embed(super::EmbedResponse),
        #[prost(message, tag = "4")]
        Error(super::AnvilError),
    }
}

// ── Streaming ────────────────────────────────────────────────────────────────

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InvokeStreamEvent {
    #[prost(string, tag = "1")]
    pub idempotency_key: ::prost::alloc::string::String,
    #[prost(oneof = "invoke_stream_event::Event", tags = "2, 3, 4, 5")]
    pub event: ::core::option::Option<invoke_stream_event::Event>,
}
/// Nested message and enum types in `InvokeStreamEvent`.
pub mod invoke_stream_event {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Event {
        #[prost(message, tag = "2")]
        Token(super::Token),
        #[prost(message, tag = "3")]
        FinalResult(super::FinalResult),
        #[prost(message, tag = "4")]
        Error(super::StreamError),
        #[prost(message, tag = "5")]
        Heartbeat(super::Heartbeat),
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Token {
    #[prost(string, tag = "1")]
    pub text: ::prost::alloc::string::String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FinalResult {
    #[prost(oneof = "final_result::Result", tags = "1, 2")]
    pub result: ::core::option::Option<final_result::Result>,
}
/// Nested message and enum types in `FinalResult`.
pub mod final_result {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Chat(super::ChatResponse),
        #[prost(message, tag = "2")]
        Embed(super::EmbedResponse),
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamError {
    #[prost(message, optional, tag = "1")]
    pub error: ::core::option::Option<AnvilError>,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Heartbeat {
    #[prost(int64, tag = "1")]
    pub timestamp_millis: i64,
}

// ── Cancel ───────────────────────────────────────────────────────────────────

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelRequest {
    #[prost(string, tag = "1")]
    pub idempotency_key: ::prost::alloc::string::String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelResponse {
    #[prost(bool, tag = "1")]
    pub cancelled: bool,
}

// ── Health ───────────────────────────────────────────────────────────────────

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HealthRequest {}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HealthResponse {
    #[prost(bool, tag = "1")]
    pub healthy: bool,
    #[prost(string, tag = "2")]
    pub version: ::prost::alloc::string::String,
}

// ── ReloadConfig ─────────────────────────────────────────────────────────────

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReloadConfigRequest {
    #[prost(string, tag = "1")]
    pub new_config_epoch: ::prost::alloc::string::String,
    #[prost(bytes = "vec", tag = "2")]
    pub new_provider_config: ::prost::alloc::vec::Vec<u8>,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReloadConfigResponse {
    #[prost(bool, tag = "1")]
    pub success: bool,
    #[prost(message, optional, tag = "2")]
    pub error: ::core::option::Option<AnvilError>,
    #[prost(string, tag = "3")]
    pub active_config_epoch: ::prost::alloc::string::String,
}

// ── Payloads ─────────────────────────────────────────────────────────────────

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChatRequest {
    #[prost(string, tag = "1")]
    pub system_prompt: ::prost::alloc::string::String,
    #[prost(message, repeated, tag = "2")]
    pub messages: ::prost::alloc::vec::Vec<Message>,
    #[prost(int32, optional, tag = "3")]
    pub max_tokens: ::core::option::Option<i32>,
    #[prost(float, optional, tag = "4")]
    pub temperature: ::core::option::Option<f32>,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Message {
    #[prost(string, tag = "1")]
    pub role: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub content: ::prost::alloc::string::String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChatResponse {
    #[prost(string, tag = "1")]
    pub content: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub model: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub usage: ::core::option::Option<Usage>,
    #[prost(string, tag = "4")]
    pub finish_reason: ::prost::alloc::string::String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EmbedRequest {
    #[prost(string, tag = "1")]
    pub text: ::prost::alloc::string::String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EmbedResponse {
    #[prost(float, repeated, tag = "1")]
    pub embedding: ::prost::alloc::vec::Vec<f32>,
    #[prost(string, tag = "2")]
    pub model: ::prost::alloc::string::String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Usage {
    #[prost(int32, tag = "1")]
    pub input_tokens: i32,
    #[prost(int32, tag = "2")]
    pub output_tokens: i32,
}

// ── Credentials ──────────────────────────────────────────────────────────────

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Credentials {
    #[prost(oneof = "credentials::Credential", tags = "1, 2")]
    pub credential: ::core::option::Option<credentials::Credential>,
}
/// Nested message and enum types in `Credentials`.
pub mod credentials {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Credential {
        #[prost(string, tag = "1")]
        ApiKey(::prost::alloc::string::String),
        #[prost(string, tag = "2")]
        BearerToken(::prost::alloc::string::String),
    }
}

// ── Timeout ──────────────────────────────────────────────────────────────────

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Timeout {
    #[prost(uint64, tag = "1")]
    pub millis: u64,
}

// ── Errors ───────────────────────────────────────────────────────────────────

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AnvilError {
    #[prost(enumeration = "ErrorClass", tag = "1")]
    pub class: i32,
    #[prost(string, tag = "2")]
    pub vendor_code: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub message: ::prost::alloc::string::String,
    #[prost(map = "string, string", tag = "4")]
    pub details: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
}

/// ErrorClass is the six-value error taxonomy. The zero value is a sentinel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ErrorClass {
    Unspecified = 0,
    Transport = 1,
    ProviderRefusal = 2,
    SchemaViolation = 3,
    AdapterBug = 4,
    Timeout = 5,
    Cancelled = 6,
}
impl ErrorClass {
    /// String value of the enum field names used in the ProtoBuf definition.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ErrorClass::Unspecified => "ERROR_CLASS_UNSPECIFIED",
            ErrorClass::Transport => "ERROR_CLASS_TRANSPORT",
            ErrorClass::ProviderRefusal => "ERROR_CLASS_PROVIDER_REFUSAL",
            ErrorClass::SchemaViolation => "ERROR_CLASS_SCHEMA_VIOLATION",
            ErrorClass::AdapterBug => "ERROR_CLASS_ADAPTER_BUG",
            ErrorClass::Timeout => "ERROR_CLASS_TIMEOUT",
            ErrorClass::Cancelled => "ERROR_CLASS_CANCELLED",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "ERROR_CLASS_UNSPECIFIED" => Some(Self::Unspecified),
            "ERROR_CLASS_TRANSPORT" => Some(Self::Transport),
            "ERROR_CLASS_PROVIDER_REFUSAL" => Some(Self::ProviderRefusal),
            "ERROR_CLASS_SCHEMA_VIOLATION" => Some(Self::SchemaViolation),
            "ERROR_CLASS_ADAPTER_BUG" => Some(Self::AdapterBug),
            "ERROR_CLASS_TIMEOUT" => Some(Self::Timeout),
            "ERROR_CLASS_CANCELLED" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

// ── gRPC client stub ─────────────────────────────────────────────────────────

pub mod sidecar_client {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value
    )]
    use tonic::codegen::*;

    #[derive(Debug, Clone)]
    pub struct SidecarClient<T> {
        inner: tonic::client::Grpc<T>,
    }

    impl SidecarClient<tonic::transport::Channel> {
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }

    impl<T> SidecarClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }

        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> SidecarClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            SidecarClient::new(InterceptedService::new(inner, interceptor))
        }

        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }

        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }

        pub async fn handshake(
            &mut self,
            request: impl tonic::IntoRequest<super::HandshakeRequest>,
        ) -> std::result::Result<tonic::Response<super::HandshakeResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/anvil.v1.Sidecar/Handshake");
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("anvil.v1.Sidecar", "Handshake"));
            self.inner.unary(req, path, codec).await
        }

        pub async fn invoke(
            &mut self,
            request: impl tonic::IntoRequest<super::InvokeRequest>,
        ) -> std::result::Result<tonic::Response<super::InvokeResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/anvil.v1.Sidecar/Invoke");
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("anvil.v1.Sidecar", "Invoke"));
            self.inner.unary(req, path, codec).await
        }

        pub async fn invoke_streaming(
            &mut self,
            request: impl tonic::IntoRequest<super::InvokeRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::InvokeStreamEvent>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/anvil.v1.Sidecar/InvokeStreaming");
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("anvil.v1.Sidecar", "InvokeStreaming"));
            self.inner.server_streaming(req, path, codec).await
        }

        pub async fn cancel(
            &mut self,
            request: impl tonic::IntoRequest<super::CancelRequest>,
        ) -> std::result::Result<tonic::Response<super::CancelResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/anvil.v1.Sidecar/Cancel");
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("anvil.v1.Sidecar", "Cancel"));
            self.inner.unary(req, path, codec).await
        }

        pub async fn health(
            &mut self,
            request: impl tonic::IntoRequest<super::HealthRequest>,
        ) -> std::result::Result<tonic::Response<super::HealthResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/anvil.v1.Sidecar/Health");
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("anvil.v1.Sidecar", "Health"));
            self.inner.unary(req, path, codec).await
        }

        pub async fn reload_config(
            &mut self,
            request: impl tonic::IntoRequest<super::ReloadConfigRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ReloadConfigResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/anvil.v1.Sidecar/ReloadConfig");
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("anvil.v1.Sidecar", "ReloadConfig"));
            self.inner.unary(req, path, codec).await
        }
    }
}
