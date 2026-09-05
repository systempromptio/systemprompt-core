//! Outbound adapter targeting the Anthropic Messages API.
//!
//! [`AnthropicOutbound`] builds a Messages request from the canonical model,
//! sends it upstream, and returns either a buffered `CanonicalResponse` or a
//! stream of canonical events translated from the Anthropic SSE format.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use systemprompt_models::wire::anthropic;

use super::{OutboundAdapter, OutboundCtx, OutboundOutcome, PreparedBody};

mod request;
mod response;
pub(in crate::services::gateway) mod streaming;
mod terminal;

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
    fn build_body(&self, ctx: &OutboundCtx<'_>) -> Result<PreparedBody> {
        if let Some(raw) = ctx.raw_body
            && let Some(bytes) = request::normalize_raw_body(raw, ctx)
        {
            return Ok(PreparedBody {
                bytes,
                raw_lane: true,
            });
        }
        let body = request::build_request_body(ctx.request, ctx.upstream_model, ctx.model_limits);
        Ok(PreparedBody {
            bytes: bytes::Bytes::from(
                serde_json::to_vec(&body).map_err(|e| anyhow!("render Anthropic request: {e}"))?,
            ),
            raw_lane: false,
        })
    }

    async fn send(&self, ctx: OutboundCtx<'_>, body: &PreparedBody) -> Result<OutboundOutcome> {
        let passthrough = body.raw_lane;
        let url = format!("{}/messages", ctx.endpoint.trim_end_matches('/'));

        let mut req = super::http_client().post(&url).body(body.bytes.clone());
        for (name, value) in request_headers(&ctx) {
            req = req.header(name, value);
        }
        let upstream_response = super::send_checked(ctx.route.provider.as_str(), req).await?;

        let content_type = upstream_response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(ToOwned::to_owned);

        if ctx.request.stream {
            let stream = upstream_response.bytes_stream();
            if passthrough {
                return Ok(OutboundOutcome::RawStreaming {
                    content_type,
                    stream: terminal::correct_stream(streaming::raw_sse_stream(stream)),
                });
            }
            return Ok(OutboundOutcome::Streaming(
                streaming::sse_to_canonical_events(stream),
            ));
        }

        let bytes = upstream_response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read Anthropic response: {e}"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("Anthropic response not valid JSON: {e}"))?;
        let canonical = Box::new(response::parse_response(&value, ctx.request.model.as_str()));
        if passthrough {
            return Ok(OutboundOutcome::RawBuffered {
                body: terminal::correct_buffered(bytes),
                content_type,
                canonical,
            });
        }
        Ok(OutboundOutcome::Buffered(canonical))
    }
}

// Why: `anthropic-version` and `anthropic-beta` must reach the provider
// unchanged, so the hardcoded version is a fallback, never an override.
fn request_headers(ctx: &OutboundCtx<'_>) -> Vec<(String, String)> {
    let mut headers = vec![
        ("x-api-key".to_owned(), ctx.api_key.to_owned()),
        ("content-type".to_owned(), "application/json".to_owned()),
    ];
    let client_sent_version = ctx
        .forward_headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("anthropic-version"));
    if !client_sent_version {
        headers.push((
            "anthropic-version".to_owned(),
            anthropic::ANTHROPIC_VERSION.to_owned(),
        ));
    }
    headers.extend(ctx.forward_headers.iter().cloned());
    headers.extend(
        ctx.route
            .extra_headers
            .iter()
            .map(|(name, value)| (name.clone(), value.clone())),
    );
    headers
}
