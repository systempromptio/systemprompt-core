//! Outbound adapter targeting the Google Gemini generativeLanguage API.
//!
//! [`GeminiOutbound`] renders the canonical model to a Gemini `generateContent`
//! request via [`systemprompt_models::wire::gemini`], sends it upstream, and
//! returns either a buffered [`CanonicalResponse`] or a stream of canonical
//! events translated from the Gemini `?alt=sse` byte stream. Auth is the
//! upstream call's: `x-goog-api-key` for an API key, a bearer for a Vertex
//! service account.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use systemprompt_models::services::WireProtocol;
use systemprompt_models::wire::gemini;

use super::super::canonical_response::CanonicalResponse;
use super::{OutboundAdapter, OutboundCtx, OutboundOutcome, PreparedBody};

#[derive(Debug, Clone, Copy, Default)]
pub struct GeminiOutbound;

#[async_trait]
impl OutboundAdapter for GeminiOutbound {
    fn build_body(&self, ctx: &OutboundCtx<'_>) -> Result<PreparedBody> {
        Ok(PreparedBody {
            bytes: bytes::Bytes::from(
                serde_json::to_vec(&gemini::build_request_body(ctx.request, ctx.model_limits))
                    .map_err(|e| anyhow!("render request body: {e}"))?,
            ),
            raw_lane: false,
        })
    }

    async fn send(&self, ctx: OutboundCtx<'_>, body: &PreparedBody) -> Result<OutboundOutcome> {
        let url = ctx
            .upstream
            .url(WireProtocol::Gemini, ctx.upstream_model, ctx.request.stream);
        let mut req = super::http_client().post(&url).body(body.bytes.clone());
        for (name, value) in ctx.upstream.headers(WireProtocol::Gemini) {
            req = req.header(name, value);
        }
        for (name, value) in &ctx.route.extra_headers {
            req = req.header(name.as_str(), value.as_str());
        }
        let upstream_response = super::send_checked(ctx.route.provider.as_str(), req).await?;

        if ctx.request.stream {
            let stream = upstream_response.bytes_stream();
            let event_stream =
                gemini::sse_to_canonical_events(stream, ctx.request.model.to_string());
            return Ok(OutboundOutcome::Streaming(event_stream));
        }

        let bytes = upstream_response
            .bytes()
            .await
            .map_err(|e| anyhow!("Failed to read Gemini response: {e}"))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("Gemini response not valid JSON: {e}"))?;
        if let Some(defect) = gemini::buffered_defect(&value) {
            return Err(super::reject_defective_body(
                ctx.route.provider.as_str(),
                "gemini",
                &defect,
                &bytes,
            ));
        }
        let canon: CanonicalResponse = gemini::parse_response(&value, ctx.request.model.as_str())
            .map_err(|e| {
            super::reject_unparsable_body(ctx.route.provider.as_str(), "gemini", &e, &bytes)
        })?;
        Ok(OutboundOutcome::Buffered(Box::new(canon)))
    }
}
