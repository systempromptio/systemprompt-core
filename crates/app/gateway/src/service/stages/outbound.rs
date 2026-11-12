//! Outbound-context assembly, caller-identity stripping, URL-image resolution,
//! and upstream-failure auditing for the dispatch stages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use systemprompt_wire::{ModelLimits, WireProtocol};

use super::super::super::audit::GatewayAudit;
use super::super::super::image_fetch::{ImageFetchPolicy, inline_url_images};
use super::super::super::protocol::canonical::CanonicalRequest;
use super::super::super::protocol::inbound::InboundAdapter;
use super::super::super::protocol::outbound::{OutboundCtx, OutboundOutcome, PreparedBody};
use super::super::resolve::ResolvedUpstream;
use super::super::{DispatchError, GatewayError};

#[derive(Clone, Copy)]
pub(super) struct CtxParts<'a> {
    pub(super) upstream_model: &'a str,
    pub(super) model_limits: Option<ModelLimits>,
    pub(super) automatic_prompt_caching: bool,
    pub(super) forward_headers: &'a [(String, String)],
    pub(super) raw_body: Option<&'a Bytes>,
}

pub(super) const fn outbound_ctx<'a>(
    upstream: &'a ResolvedUpstream<'a>,
    request: &'a CanonicalRequest,
    parts: CtxParts<'a>,
) -> OutboundCtx<'a> {
    OutboundCtx {
        target: &upstream.hop,
        upstream: &upstream.call,
        http: &upstream.http,
        request,
        upstream_model: parts.upstream_model,
        model_limits: parts.model_limits,
        automatic_prompt_caching: parts.automatic_prompt_caching,
        forward_headers: parts.forward_headers,
        raw_body: parts.raw_body,
    }
}

// JSON: provider request body — the client's own bytes, sent as they arrived
// unless the route serves the model under another upstream id.
pub fn as_sent_body(raw: &Bytes, upstream_model: &str) -> Option<Bytes> {
    let Ok(serde_json::Value::Object(mut obj)) = serde_json::from_slice(raw) else {
        return None;
    };
    if obj.get("model").and_then(serde_json::Value::as_str) == Some(upstream_model) {
        return Some(raw.clone());
    }
    obj.insert(
        "model".to_owned(),
        serde_json::Value::String(upstream_model.to_owned()),
    );
    serde_json::to_vec(&serde_json::Value::Object(obj))
        .ok()
        .map(Bytes::from)
}

pub(super) fn strip_caller_identity(request: &mut CanonicalRequest) {
    let Some(metadata) = request.metadata.as_mut() else {
        return;
    };
    let Some(obj) = metadata.as_object_mut() else {
        return;
    };
    obj.remove("user_id");
    if obj.is_empty() {
        request.metadata = None;
    }
}

pub(in crate::service) async fn audit_upstream_failure(
    audit: &GatewayAudit,
    provider: &str,
    model: &str,
    error: &GatewayError,
) {
    tracing::warn!(
        provider = %provider,
        model = %model,
        error = %error,
        "gateway upstream call failed"
    );
    if let Err(audit_err) = audit.fail(&error.to_string()).await {
        tracing::warn!(error = %audit_err, "upstream audit fail failed");
    }
}

pub(super) async fn resolve_url_images(
    wire: WireProtocol,
    request: &mut CanonicalRequest,
    audit: &GatewayAudit,
) -> Result<(), DispatchError> {
    if wire != WireProtocol::Gemini {
        return Ok(());
    }
    match inline_url_images(request, &ImageFetchPolicy::default()).await {
        Ok(0) => Ok(()),
        Ok(count) => {
            tracing::debug!(
                ai_request_id = %audit.ctx.ai_request_id,
                images = count,
                "inlined image URLs for a wire that cannot carry them"
            );
            Ok(())
        },
        Err(failure) => {
            if let Err(e) = audit.fail(&failure.to_string()).await {
                tracing::warn!(error = %e, "image-fetch audit fail failed");
            }
            Err(DispatchError::recorded(failure))
        },
    }
}

// Why: the upstream bracket closes here only for a buffered outcome. A streamed
// one is still running when the adapter returns; the tap closes it when the
// upstream event stream terminates. A failed attempt leaves the bracket open so
// a failover attempt restarts it and the row reports the attempt that served.
pub(super) async fn send_attempt(
    upstream: &ResolvedUpstream<'_>,
    ctx: OutboundCtx<'_>,
    body: &PreparedBody,
    audit: &GatewayAudit,
) -> Result<OutboundOutcome, GatewayError> {
    audit.mark_upstream_start();
    let outcome = upstream.adapter.send(ctx, body).await?;
    if matches!(
        outcome,
        OutboundOutcome::Buffered(_) | OutboundOutcome::RawBuffered { .. }
    ) {
        audit.mark_upstream_end();
    }
    Ok(outcome)
}

pub(super) async fn send_bracketed(
    upstream: &ResolvedUpstream<'_>,
    ctx: OutboundCtx<'_>,
    body: &PreparedBody,
    request_model: &str,
    audit: &GatewayAudit,
) -> Result<OutboundOutcome, DispatchError> {
    match send_attempt(upstream, ctx, body, audit).await {
        Ok(outcome) => Ok(outcome),
        Err(e) => {
            audit.mark_upstream_end();
            audit_upstream_failure(audit, upstream.hop.provider.as_str(), request_model, &e).await;
            Err(DispatchError::Recorded(e))
        },
    }
}

pub(super) fn passthrough_body<'a>(
    raw_body: &'a Bytes,
    inbound: &dyn InboundAdapter,
    upstream: &ResolvedUpstream<'_>,
    mutated: bool,
) -> Option<&'a Bytes> {
    // Why: the passthrough forwards the client's bytes untouched, so any
    // mutation of the canonical request must force re-encoding.
    if mutated {
        return None;
    }
    (inbound.passthrough_wire() == Some(upstream.hop.wire)).then_some(raw_body)
}

pub(super) fn automatic_prompt_caching(enabled: bool, upstream: &ResolvedUpstream<'_>) -> bool {
    enabled
        && upstream.hop.wire == WireProtocol::Anthropic
        && upstream
            .catalog
            .model(upstream.hop.model.as_str())
            .is_some_and(|model| model.capabilities.prompt_caching)
}

pub(super) fn prepared_body(
    upstream: &ResolvedUpstream<'_>,
    ctx: &OutboundCtx<'_>,
    as_sent: bool,
) -> Result<PreparedBody, DispatchError> {
    let sent_whole = ctx
        .raw_body
        .filter(|_| as_sent)
        .and_then(|raw| as_sent_body(raw, ctx.upstream_model));
    sent_whole.map_or_else(
        || {
            upstream
                .adapter
                .build_body(ctx)
                .map_err(DispatchError::recorded)
        },
        |bytes| {
            Ok(PreparedBody {
                bytes,
                raw_lane: true,
            })
        },
    )
}
