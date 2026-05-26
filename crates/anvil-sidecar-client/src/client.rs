#![allow(clippy::missing_errors_doc)]

use crate::proto;
use crate::proto::sidecar_client::SidecarClient;

/// Protocol versions this client supports, in preference order (most preferred first).
/// A change to this list is a breaking protocol contract change.
pub(crate) const SUPPORTED_VERSIONS: &[&str] = &["v1"];

/// Connection lifecycle state, replacing the earlier `bool handshaked`.
///
/// `ProtocolReady` means protocol negotiation succeeded but the sidecar's config epoch
/// differs from the Vault's — `reload_config()` must succeed before `invoke()` is allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionState {
    Disconnected,
    ProtocolReady,
    Ready,
}

#[derive(Debug)]
pub enum ClientError {
    /// `handshake()` must be called before any other RPC on this connection.
    HandshakeRequired,
    /// Handshake succeeded but config epochs differ; call `reload_config()` before `invoke()`.
    ConfigEpochMismatch,
    /// The sidecar negotiated a version not in `SUPPORTED_VERSIONS`.
    ProtocolMismatch(String),
    Transport(tonic::Status),
    /// The sidecar returned an `AnvilError` in a unary response envelope.
    Anvil(proto::AnvilError),
    /// The stream terminated with a stream-level `Error` event.
    Stream(Option<proto::AnvilError>),
    /// An idempotency key in the response does not match the one sent.
    ResponseMismatch {
        sent: String,
        received: String,
    },
    /// The sidecar emitted an event after `FinalResult`, violating the stream state machine.
    StreamStateMachineViolation,
    /// The stream closed without emitting a `FinalResult` or `Error` — adapter bug.
    NoFinalResult,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::HandshakeRequired => {
                write!(f, "handshake() must be called before any other RPC")
            }
            ClientError::ConfigEpochMismatch => {
                write!(
                    f,
                    "config-epoch mismatch; call reload_config() before invoke()"
                )
            }
            ClientError::ProtocolMismatch(v) => {
                write!(f, "sidecar negotiated unsupported version: {v:?}")
            }
            ClientError::Transport(s) => write!(f, "transport error: {s}"),
            ClientError::Anvil(e) => write!(f, "anvil error ({:?}): {}", e.class, e.message),
            ClientError::Stream(e) => write!(f, "stream error: {e:?}"),
            ClientError::ResponseMismatch { sent, received } => {
                write!(
                    f,
                    "idempotency key mismatch: sent {sent:?}, received {received:?}"
                )
            }
            ClientError::StreamStateMachineViolation => {
                write!(
                    f,
                    "sidecar emitted an event after FinalResult — adapter bug"
                )
            }
            ClientError::NoFinalResult => {
                write!(f, "stream closed without a FinalResult or Error event")
            }
        }
    }
}

impl std::error::Error for ClientError {}

impl From<tonic::Status> for ClientError {
    fn from(s: tonic::Status) -> Self {
        ClientError::Transport(s)
    }
}

/// Contract-enforcing wrapper around the raw gRPC `SidecarClient`.
///
/// Enforces:
/// - Handshake-first: RPCs other than `handshake()` are gated on connection state.
/// - Protocol version validation: `handshake()` rejects unsupported negotiated versions.
/// - Config-epoch state: `invoke()` and `invoke_streaming()` are blocked until epochs match;
///   call `reload_config()` to advance to `Ready` after a mismatch.
/// - Idempotency keys: `invoke()` and `invoke_streaming()` generate `UUIDv7` keys and
///   validate their echo in every response envelope.
/// - Unary error envelope: `InvokeResponse.result = Error` is surfaced as `ClientError::Anvil`.
///
/// P3c: add retry/backoff for `Transport` errors (exponential + jitter, configurable max).
pub struct AnvilSidecarClient {
    inner: SidecarClient<tonic::transport::Channel>,
    state: ConnectionState,
}

impl AnvilSidecarClient {
    pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
    where
        D: TryInto<tonic::transport::Endpoint>,
        D::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let inner = SidecarClient::connect(dst).await?;
        Ok(Self {
            inner,
            state: ConnectionState::Disconnected,
        })
    }

    /// Returns `true` if `handshake()` succeeded and config epochs match.
    pub fn is_ready(&self) -> bool {
        self.state == ConnectionState::Ready
    }

    /// Returns `true` if `handshake()` succeeded but a config reload is required.
    pub fn needs_config_reload(&self) -> bool {
        self.state == ConnectionState::ProtocolReady
    }

    /// Performs the Handshake RPC.
    ///
    /// Sets `core_protocol_version = "v1"` and `supported_versions = ["v1"]` automatically.
    /// The caller supplies only `vault_config_epoch` (SHA-256 of the current provider config).
    ///
    /// On success the connection advances to `Ready` (epochs match) or `ProtocolReady` (mismatch;
    /// call `reload_config()` before `invoke()`). On failure the state remains `Disconnected`.
    pub async fn handshake(
        &mut self,
        vault_config_epoch: String,
    ) -> Result<proto::HandshakeResponse, ClientError> {
        let supported: Vec<String> = SUPPORTED_VERSIONS.iter().map(|s| (*s).to_owned()).collect();
        let req = proto::HandshakeRequest {
            core_protocol_version: "v1".into(),
            supported_versions: supported.clone(),
            vault_config_epoch: vault_config_epoch.clone(),
        };
        let resp = self.inner.handshake(req).await?.into_inner();

        // Reject any negotiated version we do not support.
        if !supported.contains(&resp.negotiated_version) {
            self.state = ConnectionState::Disconnected;
            return Err(ClientError::ProtocolMismatch(
                resp.negotiated_version.clone(),
            ));
        }

        self.state = if resp.sidecar_config_epoch == vault_config_epoch {
            ConnectionState::Ready
        } else {
            // Epoch mismatch: protocol succeeded but invoke() is blocked until reload.
            ConnectionState::ProtocolReady
        };

        Ok(resp)
    }

    pub async fn invoke(
        &mut self,
        mut request: proto::InvokeRequest,
    ) -> Result<proto::InvokeResponse, ClientError> {
        match self.state {
            ConnectionState::Disconnected => return Err(ClientError::HandshakeRequired),
            ConnectionState::ProtocolReady => return Err(ClientError::ConfigEpochMismatch),
            ConnectionState::Ready => {}
        }
        let key = new_idempotency_key();
        request.idempotency_key = key.clone();
        let resp = self.inner.invoke(request).await?.into_inner();

        // Validate idempotency key echo (non-empty response keys must match).
        if !resp.idempotency_key.is_empty() && resp.idempotency_key != key {
            return Err(ClientError::ResponseMismatch {
                sent: key,
                received: resp.idempotency_key,
            });
        }

        // Promote AnvilError payload to typed client error.
        if let Some(proto::invoke_response::Result::Error(e)) = resp.result {
            return Err(ClientError::Anvil(e));
        }

        Ok(resp)
    }

    pub async fn invoke_streaming(
        &mut self,
        mut request: proto::InvokeRequest,
    ) -> Result<InvokeStream, ClientError> {
        match self.state {
            ConnectionState::Disconnected => return Err(ClientError::HandshakeRequired),
            ConnectionState::ProtocolReady => return Err(ClientError::ConfigEpochMismatch),
            ConnectionState::Ready => {}
        }
        let key = new_idempotency_key();
        request.idempotency_key = key.clone();
        let stream = self.inner.invoke_streaming(request).await?.into_inner();
        Ok(InvokeStream {
            inner: stream,
            idempotency_key: key,
        })
    }

    pub async fn cancel(
        &mut self,
        request: proto::CancelRequest,
    ) -> Result<proto::CancelResponse, ClientError> {
        if self.state == ConnectionState::Disconnected {
            return Err(ClientError::HandshakeRequired);
        }
        Ok(self.inner.cancel(request).await?.into_inner())
    }

    pub async fn health(&mut self) -> Result<proto::HealthResponse, ClientError> {
        if self.state == ConnectionState::Disconnected {
            return Err(ClientError::HandshakeRequired);
        }
        Ok(self
            .inner
            .health(proto::HealthRequest {})
            .await?
            .into_inner())
    }

    /// Probe liveness without requiring a prior handshake.
    ///
    /// The Go sidecar exempts `Health` from the Handshake-first requirement.
    /// This method bypasses the client-side state check so that callers can
    /// verify the sidecar is alive before establishing a session (e.g. in
    /// `anvil setup` Step 5 connectivity tests and `anvil sidecar status`).
    pub async fn probe_health(&mut self) -> Result<proto::HealthResponse, ClientError> {
        Ok(self
            .inner
            .health(proto::HealthRequest {})
            .await?
            .into_inner())
    }

    /// Atomically swaps provider config on the sidecar.
    ///
    /// On success (`resp.success == true`) advances the connection state to `Ready`,
    /// allowing `invoke()`. On failure the state is unchanged.
    pub async fn reload_config(
        &mut self,
        request: proto::ReloadConfigRequest,
    ) -> Result<proto::ReloadConfigResponse, ClientError> {
        // ProtocolReady is explicitly allowed: this is the recovery path after epoch mismatch.
        if self.state == ConnectionState::Disconnected {
            return Err(ClientError::HandshakeRequired);
        }
        let resp = self.inner.reload_config(request).await?.into_inner();
        if resp.success {
            self.state = ConnectionState::Ready;
        }
        Ok(resp)
    }
}

fn new_idempotency_key() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Drains a server-streaming `InvokeStreaming` response to its terminal event.
///
/// `Token` and `Heartbeat` events are silently discarded — callers cannot accumulate
/// partial output, enforcing the NO-COMMIT-ON-PARTIAL-OUTPUT invariant at the type level.
/// Only `FinalResult` is returned, verified to be followed by clean stream closure.
///
/// Use `idempotency_key()` to retrieve the correlation key for a concurrent `cancel()` call.
///
/// P3c: add an `events()` or `raw_stream()` API for token-observable streaming for live display.
pub struct InvokeStream {
    inner: tonic::codec::Streaming<proto::InvokeStreamEvent>,
    idempotency_key: String,
}

impl InvokeStream {
    /// Returns the `UUIDv7` idempotency key generated for this stream.
    /// Pass this to `AnvilSidecarClient::cancel()` to cancel the in-flight call.
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    /// Drains the stream to its terminal event and verifies clean closure.
    ///
    /// Returns `Ok(FinalResult)` when a `FinalResult` event arrives and is followed by
    /// stream closure (no further events).
    /// Returns `Err(ClientError::Stream(...))` on a stream-level `Error` event.
    /// Returns `Err(ClientError::StreamStateMachineViolation)` if any event follows `FinalResult`.
    /// Returns `Err(ClientError::ResponseMismatch{...})` if an event's idempotency key diverges.
    /// Returns `Err(ClientError::NoFinalResult)` if the stream closes without a terminal event.
    pub async fn collect(mut self) -> Result<proto::FinalResult, ClientError> {
        loop {
            match self.inner.message().await? {
                None => return Err(ClientError::NoFinalResult),
                Some(event) => {
                    // Validate idempotency key echo on every event (skip if not set).
                    if !event.idempotency_key.is_empty()
                        && event.idempotency_key != self.idempotency_key
                    {
                        return Err(ClientError::ResponseMismatch {
                            sent: self.idempotency_key,
                            received: event.idempotency_key,
                        });
                    }
                    match event.event {
                        Some(proto::invoke_stream_event::Event::FinalResult(r)) => {
                            // Verify the stream state machine: nothing may follow FinalResult.
                            return match self.inner.message().await? {
                                None => Ok(r),
                                Some(_) => Err(ClientError::StreamStateMachineViolation),
                            };
                        }
                        Some(proto::invoke_stream_event::Event::Error(e)) => {
                            return Err(ClientError::Stream(e.error));
                        }
                        Some(
                            proto::invoke_stream_event::Event::Token(_)
                            | proto::invoke_stream_event::Event::Heartbeat(_),
                        ) => {}
                        None => return Err(ClientError::NoFinalResult),
                    }
                }
            }
        }
    }
}
