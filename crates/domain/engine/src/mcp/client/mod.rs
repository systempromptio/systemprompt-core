//! The session with one server: request correlation, timeouts, cancellation,
//! server-initiated messages and the MCP methods this client uses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod methods;
mod options;
mod session;

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{broadcast, mpsc};
use url::Url;

pub use options::{
    DEFAULT_REQUEST_TIMEOUT, LATEST_PROTOCOL_VERSION, RequestOptions, SUPPORTED_PROTOCOL_VERSIONS,
    ServerNotification,
};
use session::Session;

use crate::error::McpError;
use crate::headers::HeaderCell;
use crate::jsonrpc;
use crate::transport::{
    InboundSink, LegacySse, LocalCommand, StdioTransport, StreamTransport, StreamableHttp,
    Transport, spawn_line_reader,
};
use crate::types::{ClientInfo, ServerInfo};

/// A connection to one MCP server.
///
/// Cheap to clone; every clone shares the session, which closes when the last
/// clone is dropped or [`McpClient::shutdown`] is called. Creating one needs a
/// running tokio runtime. Over a byte stream, call [`McpClient::initialize`]
/// after [`McpClient::from_stream`]. Shutting down deletes a streamable HTTP
/// session, or closes a local server's stdin and kills it if it has not exited
/// within a short grace period; requests still waiting fail with
/// [`McpError::Closed`].
#[derive(Clone)]
pub struct McpClient {
    inner: Arc<Session>,
}

impl fmt::Debug for McpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpClient")
            .field(
                "server",
                &self.inner.server.get().map(|info| info.name.as_str()),
            )
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl McpClient {
    pub fn from_stream<R, W>(reader: R, writer: W) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (sink, inbound) = InboundSink::new();
        spawn_line_reader(reader, sink.clone());
        let transport = StreamTransport::new(writer, sink.clone());
        Self::start(Box::new(transport), &sink, inbound)
    }

    pub(crate) fn streamable_http(http: reqwest::Client, url: Url, headers: HeaderCell) -> Self {
        let (sink, inbound) = InboundSink::new();
        let transport = StreamableHttp::new(http, url, headers, sink.clone());
        Self::start(Box::new(transport), &sink, inbound)
    }

    pub(crate) async fn legacy_sse(
        http: reqwest::Client,
        url: Url,
        headers: HeaderCell,
    ) -> Result<Self, McpError> {
        let (sink, inbound) = InboundSink::new();
        let transport = LegacySse::connect(http, url, headers, sink.clone()).await?;
        Ok(Self::start(Box::new(transport), &sink, inbound))
    }

    pub(crate) fn stdio(server: &str, local: LocalCommand<'_>) -> Result<Self, McpError> {
        let (sink, inbound) = InboundSink::new();
        let transport = StdioTransport::spawn(server, local, sink.clone())?;
        Ok(Self::start(Box::new(transport), &sink, inbound))
    }

    // JSON: the inbound channel carries decoded JSON-RPC messages for the session.
    fn start(
        transport: Box<dyn Transport>,
        sink: &InboundSink,
        inbound: mpsc::UnboundedReceiver<Value>,
    ) -> Self {
        Self {
            inner: Session::start(transport, sink, inbound),
        }
    }

    pub async fn initialize(
        &self,
        client: &ClientInfo,
        timeout: Option<Duration>,
    ) -> Result<ServerInfo, McpError> {
        let params = json!({
            "protocolVersion": LATEST_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": client,
        });
        let options = RequestOptions {
            timeout,
            cancel: None,
        };
        let result = self.request("initialize", Some(params), options).await?;
        let info = ServerInfo::from_initialize(&result)?;
        self.inner
            .transport
            .set_protocol_version(&info.protocol_version);
        if self.inner.server.set(info.clone()).is_err() {
            tracing::debug!(server = %info.name, "server already initialized; keeping the first answer");
        }
        self.notify("notifications/initialized", None).await?;
        tracing::debug!(server = %info.name, version = %info.protocol_version, "server initialized");
        Ok(info)
    }

    pub fn server_info(&self) -> Option<&ServerInfo> {
        self.inner.server.get()
    }

    pub fn notifications(&self) -> broadcast::Receiver<ServerNotification> {
        self.inner.notifications.subscribe()
    }

    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    pub async fn closed(&self) {
        self.inner.closed.cancelled().await;
    }

    // JSON: notification params are the called method's own JSON-RPC payload.
    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), McpError> {
        if self.is_closed() {
            return Err(McpError::Closed);
        }
        self.inner
            .transport
            .send(jsonrpc::notification(method, params))
            .await
    }

    // JSON: request params and result are the called method's own JSON-RPC payload.
    pub async fn request(
        &self,
        method: &str,
        params: Option<Value>,
        options: RequestOptions,
    ) -> Result<Value, McpError> {
        self.inner.request(method, params, options).await
    }

    pub async fn shutdown(&self) {
        self.inner.transport.close().await;
        self.inner.closed.cancel();
        self.inner.fail_pending();
        self.inner.drain_tasks().await;
    }
}
