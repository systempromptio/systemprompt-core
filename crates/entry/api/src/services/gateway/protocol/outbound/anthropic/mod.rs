//! Outbound adapter targeting the Anthropic Messages API.
//!
//! [`AnthropicOutbound`] builds a Messages request from the canonical model,
//! sends it upstream, and returns either a buffered `CanonicalResponse` or a
//! stream of canonical events translated from the Anthropic SSE format. The
//! same adapter serves `api.anthropic.com` and Claude on Vertex AI: the
//! upstream call's dialect decides the path, auth and body envelope.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use systemprompt_models::services::WireProtocol;
use systemprompt_models::wire::anthropic;

use super::{OutboundAdapter, OutboundCtx, OutboundOutcome, PreparedBody};

pub mod rejected_betas;
pub mod request;
pub mod response;
pub mod streaming;
mod terminal;

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
        let mut body =
            request::build_request_body(ctx.request, ctx.upstream_model, ctx.model_limits);
        request::enable_automatic_prompt_caching(&mut body, ctx);
        ctx.upstream
            .finish_value(WireProtocol::Anthropic, &mut body);
        Ok(PreparedBody {
            bytes: bytes::Bytes::from(
                serde_json::to_vec(&body).map_err(|e| anyhow!("render Anthropic request: {e}"))?,
            ),
            raw_lane: false,
        })
    }

    async fn send(&self, ctx: OutboundCtx<'_>, body: &PreparedBody) -> Result<OutboundOutcome> {
        let passthrough = body.raw_lane;
        let url = ctx.upstream.url(
            WireProtocol::Anthropic,
            ctx.upstream_model,
            ctx.request.stream,
        );

        let provider = ctx.route.provider.as_str();
        let headers =
            rejected_betas::without(request_headers(&ctx), &rejected_betas::learned(provider));
        let upstream_response = match send_once(provider, &url, &headers, &body.bytes).await {
            Err(e) => {
                let refused = refused_betas(&e);
                if !rejected_betas::carries_any(&headers, &refused) {
                    return Err(e);
                }
                rejected_betas::learn(provider, &refused);
                tracing::warn!(
                    provider,
                    refused = ?refused,
                    "upstream refused anthropic-beta values; dropped for this provider and re-sent"
                );
                send_once(
                    provider,
                    &url,
                    &rejected_betas::without(headers, &refused),
                    &body.bytes,
                )
                .await?
            },
            Ok(response) => response,
        };

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
        if let Some(defect) = anthropic::buffered_defect(&value) {
            return Err(super::reject_defective_body(
                ctx.route.provider.as_str(),
                "anthropic",
                &defect,
                &bytes,
            ));
        }
        let canonical = Box::new(
            response::parse_response(&value, ctx.request.model.as_str()).map_err(|e| {
                super::reject_unparsable_body(ctx.route.provider.as_str(), "anthropic", &e, &bytes)
            })?,
        );
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

async fn send_once(
    provider: &str,
    url: &str,
    headers: &[(String, String)],
    body: &bytes::Bytes,
) -> Result<reqwest::Response> {
    let mut req = super::http_client().post(url).body(body.clone());
    for (name, value) in headers {
        req = req.header(name, value);
    }
    super::send_checked(provider, req).await
}

fn refused_betas(error: &anyhow::Error) -> std::collections::BTreeSet<String> {
    match error.downcast_ref::<super::UpstreamError>() {
        Some(super::UpstreamError::Status {
            status: 400,
            message,
            ..
        }) => rejected_betas::refused_in(message),
        _ => std::collections::BTreeSet::new(),
    }
}

fn request_headers(ctx: &OutboundCtx<'_>) -> Vec<(String, String)> {
    let mut headers = ctx
        .upstream
        .headers_forwarding(WireProtocol::Anthropic, ctx.forward_headers);
    headers.extend(
        ctx.route
            .extra_headers
            .iter()
            .map(|(name, value)| (name.clone(), value.clone())),
    );
    headers
}
