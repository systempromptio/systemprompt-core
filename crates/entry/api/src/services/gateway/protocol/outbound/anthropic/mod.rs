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

use super::{OutboundAdapter, OutboundCtx, OutboundOutcome, UpstreamError};

mod request;
mod response;
pub(in crate::services::gateway) mod streaming;

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
    async fn send(&self, ctx: OutboundCtx<'_>) -> Result<OutboundOutcome> {
        let passthrough = ctx
            .raw_body
            .map(|raw| retarget_model(raw, ctx.upstream_model));
        let url = format!("{}/messages", ctx.endpoint.trim_end_matches('/'));

        let client = reqwest::Client::new();
        let mut req = passthrough.as_ref().map_or_else(
            || {
                client.post(&url).json(&request::build_request_body(
                    ctx.request,
                    ctx.upstream_model,
                    ctx.model_limits,
                ))
            },
            |body| client.post(&url).body(body.clone()),
        );
        for (name, value) in request_headers(&ctx) {
            req = req.header(name, value);
        }
        let upstream_response = req.send().await.map_err(|e| {
            anyhow::Error::new(UpstreamError::Transport {
                provider: ctx.route.provider.as_str().to_owned(),
                source: e,
            })
        })?;

        let status = upstream_response.status();
        if !status.is_success() {
            return Err(anyhow::Error::new(
                UpstreamError::from_response(ctx.route.provider.as_str(), upstream_response).await,
            ));
        }

        let content_type = upstream_response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(ToOwned::to_owned);

        if ctx.request.stream {
            let stream = upstream_response.bytes_stream();
            if passthrough.is_some() {
                return Ok(OutboundOutcome::RawStreaming {
                    content_type,
                    stream: streaming::raw_sse_stream(stream),
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
        if passthrough.is_some() {
            return Ok(OutboundOutcome::RawBuffered {
                body: bytes,
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

fn retarget_model(raw: &bytes::Bytes, upstream_model: &str) -> bytes::Bytes {
    let Ok(Value::Object(mut obj)) = serde_json::from_slice::<Value>(raw) else {
        return raw.clone();
    };
    if obj.get("model").and_then(Value::as_str) == Some(upstream_model) {
        return raw.clone();
    }
    obj.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
    serde_json::to_vec(&Value::Object(obj)).map_or_else(|_| raw.clone(), bytes::Bytes::from)
}
