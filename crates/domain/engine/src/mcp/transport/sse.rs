//! Server-sent events: the incremental parser shared by both HTTP transports,
//! and the legacy transport in which the client holds one event stream open and
//! posts its messages to the endpoint that stream announces.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::pin::Pin;

use bytes::Bytes;
use futures_util::stream::{Stream, StreamExt};
use reqwest::StatusCode;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderValue};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use url::Url;

use super::{InboundSink, Transport};
use crate::error::McpError;
use crate::headers::{self, HeaderCell};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SseEvent {
    pub(crate) event: Option<String>,
    pub(crate) data: String,
}

#[derive(Debug, Default)]
pub(crate) struct SseParser {
    buffer: Vec<u8>,
    event: Option<String>,
    data: Option<String>,
    started: bool,
}

impl SseParser {
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        let mut start = 0;
        let mut index = 0;
        while index < self.buffer.len() {
            let byte = self.buffer[index];
            if byte != b'\n' && byte != b'\r' {
                index += 1;
                continue;
            }
            // Why: a CR that ends the chunk may be the first half of a CRLF split across
            // reads.
            if byte == b'\r' && index + 1 == self.buffer.len() {
                break;
            }
            let width = if byte == b'\r' && self.buffer[index + 1] == b'\n' {
                2
            } else {
                1
            };
            let line = String::from_utf8_lossy(&self.buffer[start..index]).into_owned();
            self.line(&line, &mut events);
            index += width;
            start = index;
        }
        self.buffer.drain(..start);
        events
    }

    fn line(&mut self, raw: &str, events: &mut Vec<SseEvent>) {
        let line = if self.started {
            raw
        } else {
            self.started = true;
            raw.strip_prefix('\u{feff}').unwrap_or(raw)
        };
        if line.is_empty() {
            match self.data.take() {
                Some(data) => events.push(SseEvent {
                    event: self.event.take(),
                    data,
                }),
                None => self.event = None,
            }
            return;
        }
        if line.starts_with(':') {
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line, ""),
        };
        match field {
            "event" => self.event = Some(value.to_owned()),
            "data" => match &mut self.data {
                Some(data) => {
                    data.push('\n');
                    data.push_str(value);
                },
                None => self.data = Some(value.to_owned()),
            },
            _ => {},
        }
    }
}

pub(crate) fn deliver(sink: &InboundSink, event: &SseEvent) {
    if event.event.as_deref().is_some_and(|name| name != "message") || event.data.trim().is_empty()
    {
        return;
    }
    match serde_json::from_str::<Value>(&event.data) {
        Ok(message) => sink.deliver(message),
        Err(error) => tracing::debug!(error = %error, "ignoring a non-JSON event from the server"),
    }
}

pub(crate) type ByteStream = Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>;

pub(crate) fn byte_stream(response: reqwest::Response) -> ByteStream {
    Box::pin(response.bytes_stream())
}

pub(crate) async fn pump(
    mut body: ByteStream,
    mut parser: SseParser,
    sink: &InboundSink,
    cancel: &CancellationToken,
) {
    loop {
        let chunk = tokio::select! {
            () = cancel.cancelled() => return,
            chunk = body.next() => chunk,
        };
        match chunk {
            Some(Ok(bytes)) => {
                for event in parser.push(&bytes) {
                    deliver(sink, &event);
                }
            },
            Some(Err(error)) => {
                tracing::debug!(error = %error.without_url(), "the server's event stream failed");
                return;
            },
            None => return,
        }
    }
}

pub(crate) fn is_event_stream(response: &reqwest::Response) -> bool {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("text/event-stream"))
}

pub(crate) struct LegacySse {
    http: reqwest::Client,
    endpoint: Url,
    headers: HeaderCell,
    cancel: CancellationToken,
    sink: InboundSink,
}

impl LegacySse {
    pub(crate) async fn connect(
        http: reqwest::Client,
        url: Url,
        headers: HeaderCell,
        sink: InboundSink,
    ) -> Result<Self, McpError> {
        let mut request_headers = headers::current(&headers).await?;
        request_headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        let response = http
            .get(url.clone())
            .headers(request_headers)
            .send()
            .await
            .map_err(McpError::http)?;
        let status = response.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err(McpError::Unauthorized);
        }
        if !status.is_success() {
            return Err(McpError::Status {
                status: status.as_u16(),
            });
        }
        let mut body = byte_stream(response);
        let mut parser = SseParser::default();
        let (announced, backlog) = announced_endpoint(&mut body, &mut parser).await?;
        let endpoint = url.join(announced.trim()).map_err(McpError::Url)?;
        if endpoint.origin() != url.origin() {
            return Err(McpError::Protocol(
                "the announced endpoint is on another origin",
            ));
        }
        let cancel = CancellationToken::new();
        let reader_sink = sink.clone();
        let reader_cancel = cancel.clone();
        sink.tasks().spawn("mcp-legacy-sse", async move {
            for event in &backlog {
                deliver(&reader_sink, event);
            }
            pump(body, parser, &reader_sink, &reader_cancel).await;
            reader_sink.close();
        });
        Ok(Self {
            http,
            endpoint,
            headers,
            cancel,
            sink,
        })
    }
}

async fn announced_endpoint(
    body: &mut ByteStream,
    parser: &mut SseParser,
) -> Result<(String, Vec<SseEvent>), McpError> {
    loop {
        let Some(chunk) = body.next().await else {
            return Err(McpError::Protocol(
                "event stream ended before announcing an endpoint",
            ));
        };
        let bytes = chunk.map_err(McpError::http)?;
        let mut found = None;
        let mut backlog = Vec::new();
        for event in parser.push(&bytes) {
            if found.is_none() && event.event.as_deref() == Some("endpoint") {
                found = Some(event.data);
            } else if found.is_some() {
                backlog.push(event);
            }
        }
        if let Some(found) = found {
            return Ok((found, backlog));
        }
    }
}

impl Drop for LegacySse {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[async_trait::async_trait]
impl Transport for LegacySse {
    // JSON: one complete JSON-RPC message, posted to the announced endpoint.
    async fn send(&self, message: Value) -> Result<(), McpError> {
        if self.cancel.is_cancelled() {
            return Err(McpError::Closed);
        }
        let body = serde_json::to_vec(&message).map_err(McpError::Json)?;
        let mut request_headers = headers::current(&self.headers).await?;
        request_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let response = self
            .http
            .post(self.endpoint.clone())
            .headers(request_headers)
            .body(body)
            .send()
            .await
            .map_err(McpError::http)?;
        let status = response.status();
        if status == StatusCode::UNAUTHORIZED {
            return Err(McpError::Unauthorized);
        }
        if !status.is_success() {
            return Err(McpError::Status {
                status: status.as_u16(),
            });
        }
        Ok(())
    }

    async fn close(&self) {
        self.cancel.cancel();
        self.sink.close();
    }
}
