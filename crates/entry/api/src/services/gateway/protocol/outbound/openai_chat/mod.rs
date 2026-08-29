//! Outbound adapter targeting the `OpenAI` Chat Completions API.
//!
//! [`OpenAiChatOutbound`] orchestrates transport — auth headers, HTTP status
//! handling, stream-vs-buffered dispatch — and delegates every wire concern
//! (request build, response parse, SSE-to-event mapping) to the shared
//! [`systemprompt_models::wire::openai_chat`] codec. Also serves
//! OpenAI-compatible providers exposing the same surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use systemprompt_models::wire::openai_chat as codec;

use super::{OutboundAdapter, OutboundCtx, OutboundOutcome, PreparedBody};

mod raw;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::raw::normalize_raw_body;
    pub use systemprompt_models::wire::openai_chat::{
        build_request_body, parse_response, sse_to_canonical_events,
    };
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenAiChatOutbound;

#[async_trait]
impl OutboundAdapter for OpenAiChatOutbound {
    fn build_body(&self, ctx: &OutboundCtx<'_>) -> Result<PreparedBody> {
        if let Some(raw) = ctx.raw_body
            && let Some(bytes) = raw::normalize_raw_body(raw, ctx)
        {
            return Ok(PreparedBody {
                bytes,
                raw_lane: true,
            });
        }
        Ok(PreparedBody {
            bytes: bytes::Bytes::from(
                serde_json::to_vec(&codec::build_request_body(
                    ctx.request,
                    ctx.upstream_model,
                    ctx.model_limits,
                ))
                .map_err(|e| anyhow!("render request body: {e}"))?,
            ),
            raw_lane: false,
        })
    }

    async fn send(&self, ctx: OutboundCtx<'_>, body: &PreparedBody) -> Result<OutboundOutcome> {
        let url = format!("{}/chat/completions", ctx.endpoint.trim_end_matches('/'));

        let mut req = super::http_client()
            .post(&url)
            .header("authorization", format!("Bearer {}", ctx.api_key))
            .header("content-type", "application/json")
            .body(body.bytes.clone());
        for (name, value) in &ctx.route.extra_headers {
            req = req.header(name.as_str(), value.as_str());
        }
        let upstream_response = super::send_checked(ctx.route.provider.as_str(), req).await?;

        if ctx.request.stream {
            let stream = upstream_response.bytes_stream();
            let event_stream = codec::sse_to_canonical_events(stream, ctx.request.model.clone());
            return Ok(OutboundOutcome::Streaming(event_stream));
        }

        let bytes = upstream_response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read OpenAI response: {e}"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("OpenAI response not valid JSON: {e}"))?;
        let canon = codec::parse_response(&value, &ctx.request.model);
        Ok(OutboundOutcome::Buffered(Box::new(canon)))
    }
}
