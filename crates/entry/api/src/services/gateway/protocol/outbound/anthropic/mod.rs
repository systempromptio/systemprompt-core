use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;

use super::super::canonical_response::CanonicalResponse;
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
pub struct AnthropicOutbound;

#[async_trait]
impl OutboundAdapter for AnthropicOutbound {
    fn provider_tag(&self) -> &'static str {
        "anthropic"
    }

    async fn send(&self, ctx: OutboundCtx<'_>) -> Result<OutboundOutcome> {
        let body = request::build_request_body(ctx.request, ctx.upstream_model);
        let url = format!("{}/messages", ctx.endpoint.trim_end_matches('/'));

        let client = reqwest::Client::new();
        let mut req = client
            .post(&url)
            .header("x-api-key", ctx.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body);
        for (name, value) in &ctx.route.extra_headers {
            req = req.header(name.as_str(), value.as_str());
        }
        let upstream_response = req
            .send()
            .await
            .map_err(|e| anyhow!("Upstream Anthropic request failed: {e}"))?;

        let status = upstream_response.status();

        if ctx.request.stream {
            if !status.is_success() {
                let err = upstream_response
                    .text()
                    .await
                    .unwrap_or_else(|e| format!("<failed to read upstream body: {e}>"));
                return Err(anyhow!("Upstream error {status}: {err}"));
            }
            let stream = upstream_response.bytes_stream();
            let event_stream = streaming::sse_to_canonical_events(stream);
            return Ok(OutboundOutcome::Streaming(event_stream));
        }

        if !status.is_success() {
            let err = upstream_response
                .text()
                .await
                .unwrap_or_else(|e| format!("<failed to read upstream body: {e}>"));
            return Err(anyhow!("Upstream error {status}: {err}"));
        }

        let bytes = upstream_response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read Anthropic response: {e}"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("Anthropic response not valid JSON: {e}"))?;
        let canon: CanonicalResponse = response::parse_response(&value, ctx.request.model.as_str());
        Ok(OutboundOutcome::Buffered(canon))
    }
}
