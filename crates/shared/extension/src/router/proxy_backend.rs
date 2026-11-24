//! The MCP backends the API's proxy engine forwards to.
//!
//! The proxy engine under `/api/v1/mcp/{server}/…` is kernel machinery: it
//! authenticates the caller, resolves the managed service row, signs the hop
//! token, streams the request and the response, and keeps the SSE heartbeat.
//! What is MCP-specific about a backend it reads through
//! [`ProxyBackendResolver`], which the `mcp-http` member provides: the
//! port this instance spawns a managed server on, the binding of a managed
//! MCP session to its server and principal, and, for an external server,
//! an [`ExternalUpstream`] (the provider endpoint and per-user headers, the
//! external session binding, the fixed tool arguments and the per-call
//! audit). The engine governs each external tool call through the
//! [`ProxiedCallGovernor`] it hands the upstream, so the decision stays the
//! kernel's. With no resolver provided the MCP proxy is not mounted (404);
//! there is no unaudited pass-through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Bytes;
use axum::http::{HeaderMap, HeaderValue};
use futures::stream::BoxStream;
use systemprompt_identifiers::{ServiceName, SessionId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::McpServerConfig;
use systemprompt_traits::BoxedSource;

/// A proxied response body, chunk by chunk.
pub type ProxyBodyStream = BoxStream<'static, Result<Bytes, std::io::Error>>;

/// Why an MCP backend refused or failed a proxied request.
#[derive(Debug, thiserror::Error)]
pub enum ProxyBackendError {
    #[error("the backend requires the caller to authenticate")]
    AuthenticationRequired,
    #[error("the caller has not connected an account for this server")]
    ProviderNotConnected,
    #[error("the backend is unavailable: {status}")]
    Unavailable { status: String },
    #[error("the external server could not be resolved")]
    ResolveFailed(#[source] BoxedSource),
    #[error("the backend refused the request")]
    Forbidden,
    #[error("the MCP registry could not be read")]
    RegistryUnavailable(#[source] BoxedSource),
    #[error("the MCP session binding could not be read")]
    SessionLookupFailed(#[source] BoxedSource),
}

/// The MCP backends behind the proxy engine. Injected as
/// `dyn ProxyBackendResolver`, hence `#[async_trait]`.
#[async_trait]
pub trait ProxyBackendResolver: Send + Sync + std::fmt::Debug {
    fn external_server(
        &self,
        service: &ServiceName,
    ) -> Result<Option<McpServerConfig>, ProxyBackendError>;

    fn spawn_port(&self, service: &ServiceName) -> Option<u16>;

    async fn session_bound(
        &self,
        service: &ServiceName,
        session: &SessionId,
        principal: Option<&UserId>,
    ) -> Result<bool, ProxyBackendError>;

    async fn bind_session(
        &self,
        service: &ServiceName,
        session: &SessionId,
        principal: Option<&UserId>,
    ) -> Result<(), ProxyBackendError>;

    async fn open_external(
        &self,
        server: &McpServerConfig,
        caller: &RequestContext,
    ) -> Result<Box<dyn ExternalUpstream>, ProxyBackendError>;
}

pub type DynProxyBackendResolver = Arc<dyn ProxyBackendResolver>;

/// One caller's view of an external MCP server for one request.
#[async_trait]
pub trait ExternalUpstream: Send + Sync {
    fn url(&self) -> &str;

    fn headers(&self) -> &HeaderMap;

    async fn accepts(&self, session: &HeaderValue) -> Result<bool, ProxyBackendError>;

    async fn remember(&self, response_headers: &HeaderMap) -> Result<(), ProxyBackendError>;

    async fn forget(&self, request_headers: &HeaderMap) -> Result<(), ProxyBackendError>;

    async fn prepare(
        &self,
        body: &mut Vec<u8>,
        governor: &dyn ProxiedCallGovernor,
    ) -> Result<Option<Box<dyn CallAudit>>, ProxyBackendError>;
}

/// A JSON-RPC `tools/call` an external server is asked to run, as the
/// governor judges it.
#[derive(Debug)]
pub enum GovernedToolCall<'a> {
    Valid {
        tool: &'a str,
        arguments: serde_json::Value,
    },
    InvalidName {
        raw_name: Option<&'a str>,
    },
}

/// The kernel's policy decision on one proxied external tool call. Injected
/// as `dyn ProxiedCallGovernor`, hence `#[async_trait]`.
#[async_trait]
pub trait ProxiedCallGovernor: Send + Sync {
    async fn enforce(
        &self,
        caller: &RequestContext,
        service: &ServiceName,
        call: GovernedToolCall<'_>,
    ) -> Result<(), ProxyBackendError>;
}

/// The audit of one proxied tool call: it observes the response body as it
/// is forwarded and records the execution once the body ends or is dropped.
pub trait CallAudit: Send {
    fn tap(self: Box<Self>, body: ProxyBodyStream, event_stream: bool) -> ProxyBodyStream;
}
