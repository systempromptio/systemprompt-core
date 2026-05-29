//! Outbound adapter targeting the OpenAI Chat Completions API.
//!
//! [`OpenAiChatOutbound`] builds a Chat Completions request from the canonical
//! model, sends it upstream, and returns either a buffered response or a stream
//! of canonical events translated from the Chat Completions SSE format. Also
//! serves OpenAI-compatible providers exposing the same surface.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;

use super::{OutboundAdapter, OutboundCtx, OutboundOutcome};

mod request;
mod response;
mod streaming;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::request::build_request_body;
    pub use super::response::parse_response;
    pub use super::streaming::sse_to_canonical_events;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenAiChatOutbound;

#[async_trait]
impl OutboundAdapter for OpenAiChatOutbound {
    fn provider_tag(&self) -> &'static str {
        "openai"
    }

    async fn send(&self, ctx: OutboundCtx<'_>) -> Result<OutboundOutcome> {
        let body = request::build_request_body(ctx.request, ctx.upstream_model);
        let url = format!("{}/chat/completions", ctx.endpoint.trim_end_matches('/'));

        let client = reqwest::Client::new();
        let mut req = client
            .post(&url)
            .header("authorization", format!("Bearer {}", ctx.api_key))
            .header("content-type", "application/json")
            .json(&body);
        for (name, value) in &ctx.route.extra_headers {
            req = req.header(name.as_str(), value.as_str());
        }

        let upstream_response = req
            .send()
            .await
            .map_err(|e| anyhow!("Upstream OpenAI-compatible request failed: {e}"))?;

        let status = upstream_response.status();
        if !status.is_success() {
            let err = upstream_response
                .text()
                .await
                .unwrap_or_else(|e| format!("<failed to read upstream body: {e}>"));
            return Err(anyhow!("Upstream error {status}: {err}"));
        }

        if ctx.request.stream {
            let stream = upstream_response.bytes_stream();
            let event_stream =
                streaming::sse_to_canonical_events(stream, ctx.request.model.clone());
            return Ok(OutboundOutcome::Streaming(event_stream));
        }

        let bytes = upstream_response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read OpenAI response: {e}"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("OpenAI response not valid JSON: {e}"))?;
        let canon = response::parse_response(&value, &ctx.request.model);
        Ok(OutboundOutcome::Buffered(canon))
    }
}
