//! Observe-while-forwarding tap that captures an external MCP tool-call result.
//!
//! For an SSE response the tap forwards each chunk untouched while scanning the
//! stream for the JSON-RPC frame matching the request id; for a single JSON
//! response it buffers, parses, and forwards. Either way it finalizes the
//! [`McpAudit`] exactly once, on stream EOF or drop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use bytes::Bytes;
use futures_util::{Stream, TryStreamExt};
use serde_json::Value;

use super::super::backend::{ResponseHandler, SSE_KEEPALIVE_INTERVAL, SseKeepaliveStream};
use super::McpAudit;
use super::jsonrpc::{ToolCallOutcome, extract_sse_data, parse_response_frame};

pub(crate) async fn record(
    response: reqwest::Response,
    audit: McpAudit,
) -> Result<Response<Body>, String> {
    let status = StatusCode::from_u16(response.status().as_u16())
        .map_err(|e| format!("Invalid status code: {e}"))?;
    let headers = response.headers().clone();
    let is_sse = ResponseHandler::is_event_stream(&headers);

    if is_sse {
        let accumulator = Arc::new(Mutex::new(SseAccumulator::new(audit.request_id().clone())));
        let stream = response.bytes_stream().map_err(io::Error::other);
        let tapped = McpAuditTapStream {
            inner: stream,
            accumulator,
            audit: Some(audit),
        };
        let body = Body::from_stream(SseKeepaliveStream::new(tapped, SSE_KEEPALIVE_INTERVAL));
        ResponseHandler::assemble(status, &headers, true, body)
    } else {
        let bytes = response.bytes().await.map_err(|e| e.to_string())?;
        let outcome = match std::str::from_utf8(&bytes) {
            Ok(text) => parse_response_frame(text, audit.request_id()),
            Err(e) => {
                tracing::warn!(error = %e, "external MCP response body was not valid UTF-8; not audited");
                None
            },
        };
        audit.finalize(outcome);
        ResponseHandler::assemble(status, &headers, false, Body::from(bytes))
    }
}

struct SseAccumulator {
    buf: Vec<u8>,
    request_id: Value,
    outcome: Option<ToolCallOutcome>,
}

impl SseAccumulator {
    const fn new(request_id: Value) -> Self {
        Self {
            buf: Vec::new(),
            request_id,
            outcome: None,
        }
    }

    fn push(&mut self, chunk: &[u8]) {
        if self.outcome.is_some() {
            return;
        }
        self.buf.extend_from_slice(chunk);
        while let Some(end) = find_frame_end(&self.buf) {
            let frame: Vec<u8> = self.buf.drain(..end).collect();
            self.consume(&frame);
            if self.outcome.is_some() {
                break;
            }
        }
    }

    fn consume(&mut self, frame: &[u8]) {
        let text = String::from_utf8_lossy(frame);
        if let Some(data) = extract_sse_data(&text) {
            self.outcome = parse_response_frame(&data, &self.request_id);
        }
    }
}

fn find_frame_end(buf: &[u8]) -> Option<usize> {
    buf.windows(2)
        .position(|w| w == b"\n\n")
        .map(|i| i + 2)
        .or_else(|| buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4))
}

struct McpAuditTapStream<S> {
    inner: S,
    accumulator: Arc<Mutex<SseAccumulator>>,
    audit: Option<McpAudit>,
}

impl<S> McpAuditTapStream<S> {
    fn finish(&mut self) {
        if let Some(audit) = self.audit.take() {
            let outcome = self
                .accumulator
                .lock()
                .ok()
                .and_then(|mut acc| acc.outcome.take());
            audit.finalize(outcome);
        }
    }
}

impl<S> Stream for McpAuditTapStream<S>
where
    S: Stream<Item = Result<Bytes, io::Error>> + Unpin,
{
    type Item = Result<Bytes, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                if let Ok(mut acc) = self.accumulator.lock() {
                    acc.push(&bytes);
                }
                Poll::Ready(Some(Ok(bytes)))
            },
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => {
                self.finish();
                Poll::Ready(None)
            },
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S> Drop for McpAuditTapStream<S> {
    fn drop(&mut self) {
        self.finish();
    }
}
