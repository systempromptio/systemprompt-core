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

use async_trait::async_trait;
use serde_json::Value;
use systemprompt_wire::{WireProtocol, anthropic};

use super::{OutboundAdapter, OutboundCtx, OutboundError, OutboundOutcome, PreparedBody};

mod learned;
pub mod refused_fields;
pub mod rejected_betas;
pub mod request;
pub mod response;
pub mod streaming;
mod terminal;

#[derive(Debug, Clone, Copy, Default)]
pub struct AnthropicOutbound;

#[async_trait]
impl OutboundAdapter for AnthropicOutbound {
    fn build_body(&self, ctx: &OutboundCtx<'_>) -> Result<PreparedBody, OutboundError> {
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
            bytes: bytes::Bytes::from(serde_json::to_vec(&body).map_err(|source| {
                OutboundError::RenderBody {
                    wire: "anthropic",
                    source,
                }
            })?),
            raw_lane: false,
        })
    }

    async fn send(
        &self,
        ctx: OutboundCtx<'_>,
        body: &PreparedBody,
    ) -> Result<OutboundOutcome, OutboundError> {
        let passthrough = body.raw_lane;
        let url = ctx.upstream.url(
            WireProtocol::Anthropic,
            ctx.upstream_model,
            ctx.request.stream,
        );

        let upstream_response =
            send_learning_refusals(ctx.route.provider.as_str(), &url, &ctx, body).await?;

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
            .map_err(|source| OutboundError::ReadBody {
                wire: "anthropic",
                source,
            })?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|source| OutboundError::DecodeBody {
                wire: "anthropic",
                source,
            })?;
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

async fn send_learning_refusals(
    provider: &str,
    url: &str,
    ctx: &OutboundCtx<'_>,
    body: &PreparedBody,
) -> Result<reqwest::Response, super::UpstreamError> {
    let headers = rejected_betas::without(request_headers(ctx), &rejected_betas::learned(provider));
    let error = match send_once(provider, url, &headers, &body.bytes).await {
        Ok(response) => return Ok(response),
        Err(e) => e,
    };
    let refused = refused_betas(&error);
    let refused_fields = if body.raw_lane {
        refused_fields(&error)
    } else {
        std::collections::BTreeSet::new()
    };
    let header_carries = rejected_betas::carries_any(&headers, &refused);
    let body_carries = request::carries_any_field(&body.bytes, &refused_fields);
    if !header_carries && !body_carries {
        return Err(error);
    }
    if refused
        .iter()
        .any(|beta| beta.starts_with("tool-search-") || beta.starts_with("advanced-tool-use-"))
        || refused_fields
            .iter()
            .any(|field| matches!(field.as_str(), "defer_loading" | "tool_reference"))
    {
        tracing::warn!(
            provider,
            "tool search is not supported by this upstream; preserving the original error"
        );
        return Err(error);
    }
    rejected_betas::learn(provider, &refused);
    refused_fields::learn(provider, &refused_fields);
    tracing::warn!(
        provider,
        refused_betas = ?refused,
        refused_fields = ?refused_fields,
        "upstream refused anthropic-beta values or body fields; dropped for this provider and re-sent"
    );
    let dropped =
        anthropic::BetaHeader::parse(&refused.iter().cloned().collect::<Vec<_>>().join(","));
    let bytes = request::without_refused(&body.bytes, provider, &dropped, &refused_fields);
    send_once(
        provider,
        url,
        &rejected_betas::without(headers, &refused),
        &bytes,
    )
    .await
}

async fn send_once(
    provider: &str,
    url: &str,
    headers: &[(String, String)],
    body: &bytes::Bytes,
) -> Result<reqwest::Response, super::UpstreamError> {
    let mut req = super::http_client().post(url).body(body.clone());
    for (name, value) in headers {
        req = req.header(name, value);
    }
    super::send_checked(provider, req).await
}

fn bad_request_message(error: &super::UpstreamError) -> Option<&str> {
    match error {
        super::UpstreamError::Status {
            status: 400,
            message,
            ..
        } => Some(message),
        super::UpstreamError::Status { .. } | super::UpstreamError::Transport { .. } => None,
    }
}

fn refused_betas(error: &super::UpstreamError) -> std::collections::BTreeSet<String> {
    bad_request_message(error).map_or_else(Default::default, rejected_betas::refused_in)
}

fn refused_fields(error: &super::UpstreamError) -> std::collections::BTreeSet<String> {
    bad_request_message(error).map_or_else(Default::default, refused_fields::refused_in)
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
