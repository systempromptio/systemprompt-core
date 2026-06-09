//! Outbound adapter targeting the Google Gemini generativeLanguage API.
//!
//! [`GeminiOutbound`] renders the canonical model to a Gemini `generateContent`
//! request via [`systemprompt_models::wire::gemini`], sends it upstream, and
//! returns either a buffered [`CanonicalResponse`] or a stream of canonical
//! events translated from the Gemini `?alt=sse` byte stream. Auth rides the
//! `x-goog-api-key` header.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use systemprompt_models::wire::gemini;

use super::super::canonical_response::CanonicalResponse;
use super::{
    OutboundAdapter, OutboundCtx, OutboundOutcome, UpstreamError, extract_upstream_message,
};

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use systemprompt_models::wire::gemini::{
        build_request_body, parse_response, sse_to_canonical_events,
    };
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GeminiOutbound;

#[async_trait]
impl OutboundAdapter for GeminiOutbound {
    async fn send(&self, ctx: OutboundCtx<'_>) -> Result<OutboundOutcome> {
        let body = gemini::build_request_body(
            ctx.request,
            ctx.model_limits.and_then(|l| l.max_thinking_budget),
            ctx.model_limits.map(|l| l.max_output_tokens),
        );
        let path = gemini::upstream_path(ctx.upstream_model, ctx.request.stream);
        let url = format!("{}{path}", ctx.endpoint.trim_end_matches('/'));

        let client = reqwest::Client::new();
        let mut req = client
            .post(&url)
            .header(gemini::API_KEY_HEADER, ctx.api_key)
            .header("content-type", "application/json")
            .json(&body);
        for (name, value) in &ctx.route.extra_headers {
            req = req.header(name.as_str(), value.as_str());
        }

        let upstream_response = req.send().await.map_err(|e| {
            anyhow::Error::new(UpstreamError::Transport {
                provider: ctx.route.provider.as_str().to_owned(),
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
                provider: ctx.route.provider.as_str().to_owned(),
                status: status.as_u16(),
                message: extract_upstream_message(&err),
            }));
        }

        if ctx.request.stream {
            let stream = upstream_response.bytes_stream();
            let event_stream = gemini::sse_to_canonical_events(stream, ctx.request.model.clone());
            return Ok(OutboundOutcome::Streaming(event_stream));
        }

        let bytes = upstream_response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read Gemini response: {e}"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("Gemini response not valid JSON: {e}"))?;
        let canon: CanonicalResponse = gemini::parse_response(&value, ctx.request.model.as_str());
        Ok(OutboundOutcome::Buffered(Box::new(canon)))
    }
}
