//! Outbound adapter targeting the `OpenAI` Responses API.
//!
//! [`OpenAiResponsesOutbound`] orchestrates transport — auth headers, HTTP
//! status handling, stream-vs-buffered dispatch — and delegates every wire
//! concern (request build, response parse, SSE-to-event mapping) to the shared
//! [`systemprompt_models::wire::openai_responses`] codec.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use systemprompt_models::wire::openai_responses as codec;

use super::{
    OutboundAdapter, OutboundCtx, OutboundOutcome, UpstreamError, extract_upstream_message,
};

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use systemprompt_models::wire::openai_responses::{
        build_request_body, parse_response_object, sse_to_canonical_events,
    };
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenAiResponsesOutbound;

#[async_trait]
impl OutboundAdapter for OpenAiResponsesOutbound {
    fn provider_tag(&self) -> &'static str {
        "openai-responses"
    }

    async fn send(&self, ctx: OutboundCtx<'_>) -> Result<OutboundOutcome> {
        let body = codec::build_request_body(
            ctx.request,
            ctx.upstream_model,
            ctx.model_limits.map(|l| l.max_output_tokens),
        );
        let url = format!("{}/responses", ctx.endpoint.trim_end_matches('/'));

        let client = reqwest::Client::new();
        let mut req = client
            .post(&url)
            .header("authorization", format!("Bearer {}", ctx.api_key))
            .header("content-type", "application/json")
            .json(&body);
        for (name, value) in &ctx.route.extra_headers {
            req = req.header(name.as_str(), value.as_str());
        }

        let upstream_response = req.send().await.map_err(|e| {
            anyhow::Error::new(UpstreamError::Transport {
                provider: self.provider_tag(),
                source: e,
            })
        })?;
        let status = upstream_response.status();
        if !status.is_success() {
            let err = upstream_response
                .text()
                .await
                .unwrap_or_else(|e| format!("<failed to read upstream body: {e}>"));
            return Err(anyhow::Error::new(UpstreamError::Status {
                provider: self.provider_tag(),
                status: status.as_u16(),
                message: extract_upstream_message(&err),
            }));
        }

        if ctx.request.stream {
            let stream = upstream_response.bytes_stream();
            let event_stream = codec::sse_to_canonical_events(stream, ctx.request.model.clone());
            return Ok(OutboundOutcome::Streaming(event_stream));
        }

        let bytes = upstream_response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read Responses body: {e}"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("Responses body not valid JSON: {e}"))?;
        let canon = codec::parse_response_object(&value, &ctx.request.model);
        Ok(OutboundOutcome::Buffered(Box::new(canon)))
    }
}
