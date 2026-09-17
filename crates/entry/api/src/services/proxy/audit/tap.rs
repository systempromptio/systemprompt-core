//! Observe-while-forwarding tap that captures an external MCP tool-call result
//! and stamps the execution id into it.
//!
//! For an SSE response the tap forwards the stream frame by frame, scanning
//! for the JSON-RPC frame matching the request id and rewriting that one
//! frame's `_meta` before it goes out; for a single JSON response it buffers,
//! parses, stamps, and forwards. Either way it finalizes the [`McpAudit`]
//! exactly once, on stream EOF or drop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use bytes::Bytes;
use futures_util::{Stream, TryStreamExt};
use serde_json::Value;

use super::super::backend::{ResponseHandler, SSE_KEEPALIVE_INTERVAL, SseKeepaliveStream};
use super::McpAudit;
use super::jsonrpc::{
    ToolCallOutcome, extract_sse_data, frame_matches, parse_response_frame, replace_sse_data,
    stamp_execution,
};

pub async fn record(
    response: reqwest::Response,
    audit: McpAudit,
) -> Result<Response<Body>, String> {
    let status = StatusCode::from_u16(response.status().as_u16())
        .map_err(|e| format!("Invalid status code: {e}"))?;
    let headers = response.headers().clone();
    let is_sse = ResponseHandler::is_event_stream(&headers);

    if is_sse {
        let accumulator = SseAccumulator::new(
            audit.request_id().clone(),
            audit.mcp_execution_id().to_string(),
        );
        let stream = response.bytes_stream().map_err(io::Error::other);
        let tapped = McpAuditTapStream {
            inner: stream,
            accumulator,
            ready: VecDeque::new(),
            audit: Some(audit),
        };
        let body = Body::from_stream(SseKeepaliveStream::new(tapped, SSE_KEEPALIVE_INTERVAL));
        ResponseHandler::assemble(status, &headers, true, body)
    } else {
        let bytes = response.bytes().await.map_err(|e| e.to_string())?;
        let (outcome, body) = match std::str::from_utf8(&bytes) {
            Ok(text) => {
                let outcome = parse_response_frame(text, audit.request_id());
                let body = if outcome.is_some() {
                    stamp_execution(text, audit.mcp_execution_id().as_str())
                        .map_or_else(|| Body::from(bytes.clone()), Body::from)
                } else {
                    Body::from(bytes.clone())
                };
                (outcome, body)
            },
            Err(e) => {
                tracing::warn!(error = %e, "external MCP response body was not valid UTF-8; not audited");
                (None, Body::from(bytes.clone()))
            },
        };
        audit.finalize(outcome);
        // Why: headers are reassembled by the handler, so a rewritten body
        // never ships with the upstream content-length.
        ResponseHandler::assemble(status, &headers, false, body)
    }
}

struct SseAccumulator {
    buf: Vec<u8>,
    request_id: Value,
    mcp_execution_id: String,
    outcome: Option<ToolCallOutcome>,
}

impl SseAccumulator {
    const fn new(request_id: Value, mcp_execution_id: String) -> Self {
        Self {
            buf: Vec::new(),
            request_id,
            mcp_execution_id,
            outcome: None,
        }
    }

    fn push(&mut self, chunk: &[u8]) -> Vec<Bytes> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        while let Some(end) = find_frame_end(&self.buf) {
            let frame: Vec<u8> = self.buf.drain(..end).collect();
            out.push(self.consume(frame));
        }
        out
    }

    fn flush(&mut self) -> Option<Bytes> {
        if self.buf.is_empty() {
            return None;
        }
        let frame: Vec<u8> = self.buf.drain(..).collect();
        Some(self.consume(frame))
    }

    fn consume(&mut self, frame: Vec<u8>) -> Bytes {
        if self.outcome.is_some() {
            return Bytes::from(frame);
        }
        let text = String::from_utf8_lossy(&frame).into_owned();
        let Some(data) = extract_sse_data(&text) else {
            return Bytes::from(frame);
        };
        if !frame_matches(&data, &self.request_id) {
            return Bytes::from(frame);
        }
        self.outcome = parse_response_frame(&data, &self.request_id);
        stamp_execution(&data, &self.mcp_execution_id).map_or_else(
            || Bytes::from(frame),
            |stamped| Bytes::from(replace_sse_data(&text, &stamped)),
        )
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
    accumulator: SseAccumulator,
    ready: VecDeque<Bytes>,
    audit: Option<McpAudit>,
}

impl<S> McpAuditTapStream<S> {
    fn finish(&mut self) {
        if let Some(audit) = self.audit.take() {
            audit.finalize(self.accumulator.outcome.take());
        }
    }
}

impl<S> Stream for McpAuditTapStream<S>
where
    S: Stream<Item = Result<Bytes, io::Error>> + Unpin,
{
    type Item = Result<Bytes, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(frame) = self.ready.pop_front() {
                return Poll::Ready(Some(Ok(frame)));
            }
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    let frames = self.accumulator.push(&bytes);
                    self.ready.extend(frames);
                },
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
                Poll::Ready(None) => {
                    if let Some(rest) = self.accumulator.flush() {
                        self.ready.push_back(rest);
                        continue;
                    }
                    self.finish();
                    return Poll::Ready(None);
                },
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl<S> Drop for McpAuditTapStream<S> {
    fn drop(&mut self) {
        self.finish();
    }
}
