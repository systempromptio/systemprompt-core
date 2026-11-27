//! Streamable HTTP: every message is a POST; a request's answer comes back as
//! JSON or as an event stream, and an optional GET stream carries what the
//! server sends on its own, such as tool list changes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use reqwest::StatusCode;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use url::Url;

use super::sse::{self, SseParser};
use super::{InboundSink, Transport};
use crate::mcp::error::McpError;
use crate::mcp::headers::{self, HeaderCell};
use crate::mcp::jsonrpc;

const SESSION_HEADER: &str = "mcp-session-id";
const PROTOCOL_HEADER: &str = "mcp-protocol-version";
const POST_ACCEPT: &str = "application/json, text/event-stream";
const DELETE_GRACE: Duration = Duration::from_secs(2);
const LISTEN_RETRY_MIN: Duration = Duration::from_secs(1);
const LISTEN_RETRY_MAX: Duration = Duration::from_secs(30);

pub(crate) struct StreamableHttp {
    shared: Arc<Shared>,
}

struct Shared {
    http: reqwest::Client,
    url: Url,
    headers: HeaderCell,
    session: Mutex<Option<String>>,
    protocol: Mutex<Option<String>>,
    listening: AtomicBool,
    cancel: CancellationToken,
    sink: InboundSink,
}

impl StreamableHttp {
    pub(crate) fn new(
        http: reqwest::Client,
        url: Url,
        headers: HeaderCell,
        sink: InboundSink,
    ) -> Self {
        Self {
            shared: Arc::new(Shared {
                http,
                url,
                headers,
                session: Mutex::new(None),
                protocol: Mutex::new(None),
                listening: AtomicBool::new(false),
                cancel: CancellationToken::new(),
                sink,
            }),
        }
    }
}

impl Drop for StreamableHttp {
    fn drop(&mut self) {
        self.shared.cancel.cancel();
    }
}

impl Shared {
    async fn request_headers(&self) -> Result<HeaderMap, McpError> {
        let mut map = headers::current(&self.headers).await?;
        let session = self.session.lock().clone();
        if let Some(session) = session
            && let Ok(value) = HeaderValue::from_str(&session)
        {
            map.insert(HeaderName::from_static(SESSION_HEADER), value);
        }
        let protocol = self.protocol.lock().clone();
        if let Some(protocol) = protocol
            && let Ok(value) = HeaderValue::from_str(&protocol)
        {
            map.insert(HeaderName::from_static(PROTOCOL_HEADER), value);
        }
        Ok(map)
    }

    // JSON: one complete JSON-RPC message, posted as the request body.
    async fn post(self: &Arc<Self>, message: Value) -> Result<(), McpError> {
        if self.cancel.is_cancelled() {
            return Err(McpError::Closed);
        }
        let expects_reply = jsonrpc::is_request(&message);
        let opens_listener = jsonrpc::method(&message) == Some("notifications/initialized");
        let body = serde_json::to_vec(&message).map_err(McpError::Json)?;
        let mut request_headers = self.request_headers().await?;
        request_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        request_headers.insert(ACCEPT, HeaderValue::from_static(POST_ACCEPT));
        let had_session = self.session.lock().is_some();
        let post = self
            .http
            .post(self.url.clone())
            .headers(request_headers)
            .body(body)
            .send();
        let sent = tokio::select! {
            () = self.cancel.cancelled() => return Err(McpError::Closed),
            sent = post => sent,
        };
        let response = sent.map_err(McpError::http)?;
        self.accept(&response, had_session)?;
        if opens_listener {
            self.listen();
        }
        if response.status() == StatusCode::ACCEPTED || !expects_reply {
            return Ok(());
        }
        self.read_reply(response).await
    }

    fn accept(&self, response: &reqwest::Response, had_session: bool) -> Result<(), McpError> {
        let session = response
            .headers()
            .get(SESSION_HEADER)
            .and_then(|value| value.to_str().ok());
        if let Some(session) = session {
            *self.session.lock() = Some(session.to_owned());
        }
        let status = response.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err(McpError::Unauthorized);
        }
        if status == StatusCode::NOT_FOUND && had_session {
            tracing::debug!("the server no longer knows this session");
            self.sink.close();
            return Err(McpError::SessionExpired);
        }
        if !status.is_success() {
            tracing::debug!(status = status.as_u16(), "the server refused a message");
            return Err(McpError::Status {
                status: status.as_u16(),
            });
        }
        Ok(())
    }

    async fn read_reply(self: &Arc<Self>, response: reqwest::Response) -> Result<(), McpError> {
        if sse::is_event_stream(&response) {
            let shared = Arc::clone(self);
            self.sink
                .tasks()
                .spawn("mcp-http-reply-stream", async move {
                    let body = sse::byte_stream(response);
                    sse::pump(body, SseParser::default(), &shared.sink, &shared.cancel).await;
                });
            return Ok(());
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if content_type.starts_with("application/json") {
            let bytes = response.bytes().await.map_err(McpError::http)?;
            let reply: Value = serde_json::from_slice(&bytes).map_err(McpError::Json)?;
            self.sink.deliver(reply);
            return Ok(());
        }
        Err(McpError::ContentType { content_type })
    }

    fn listen(self: &Arc<Self>) {
        if self.listening.swap(true, Ordering::SeqCst) {
            return;
        }
        let shared = Arc::clone(self);
        self.sink
            .tasks()
            .spawn("mcp-http-listen", async move { shared.listen_loop().await });
    }

    async fn listen_loop(self: Arc<Self>) {
        let mut retry = LISTEN_RETRY_MIN;
        loop {
            let mut request_headers = match self.request_headers().await {
                Ok(map) => map,
                Err(error) => {
                    tracing::debug!(error = %error, "not opening the notification stream");
                    return;
                },
            };
            request_headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
            let sent = tokio::select! {
                () = self.cancel.cancelled() => return,
                sent = self.http.get(self.url.clone()).headers(request_headers).send() => sent,
            };
            match sent {
                Ok(response)
                    if response.status().is_success() && sse::is_event_stream(&response) =>
                {
                    let opened = Instant::now();
                    sse::pump(
                        sse::byte_stream(response),
                        SseParser::default(),
                        &self.sink,
                        &self.cancel,
                    )
                    .await;
                    if opened.elapsed() >= LISTEN_RETRY_MAX {
                        retry = LISTEN_RETRY_MIN;
                    }
                },
                Ok(response) => {
                    tracing::debug!(
                        status = response.status().as_u16(),
                        "the server offers no notification stream"
                    );
                    return;
                },
                Err(error) => {
                    tracing::debug!(error = %error.without_url(), "the notification stream failed to open");
                },
            }
            tokio::select! {
                () = self.cancel.cancelled() => return,
                () = tokio::time::sleep(retry) => {}
            }
            retry = (retry * 2).min(LISTEN_RETRY_MAX);
        }
    }

    async fn end_session(&self) {
        let session = self.session.lock().clone();
        if session.is_none() || self.cancel.is_cancelled() {
            return;
        }
        let Ok(request_headers) = self.request_headers().await else {
            return;
        };
        let delete = self
            .http
            .delete(self.url.clone())
            .headers(request_headers)
            .send();
        match tokio::time::timeout(DELETE_GRACE, delete).await {
            Ok(Ok(response)) => tracing::debug!(
                status = response.status().as_u16(),
                "ended the server session"
            ),
            Ok(Err(error)) => {
                tracing::debug!(error = %error.without_url(), "ending the server session failed");
            },
            Err(_) => tracing::debug!("ending the server session timed out"),
        }
    }
}

#[async_trait::async_trait]
impl Transport for StreamableHttp {
    // JSON: one complete JSON-RPC message, posted as the request body.
    async fn send(&self, message: Value) -> Result<(), McpError> {
        self.shared.post(message).await
    }

    fn set_protocol_version(&self, version: &str) {
        *self.shared.protocol.lock() = Some(version.to_owned());
    }

    async fn close(&self) {
        self.shared.end_session().await;
        self.shared.cancel.cancel();
        self.shared.sink.close();
    }
}
