//! Outbound-context assembly, caller-identity stripping, URL-image resolution,
//! and upstream-failure auditing for the dispatch stages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use systemprompt_models::services::ai::ModelLimits;
use systemprompt_models::services::providers::WireProtocol;

use super::super::super::audit::GatewayAudit;
use super::super::super::image_fetch::{ImageFetchPolicy, inline_url_images};
use super::super::super::protocol::canonical::CanonicalRequest;
use super::super::super::protocol::outbound::OutboundCtx;
use super::super::DispatchError;
use super::super::resolve::ResolvedUpstream;

#[derive(Clone, Copy)]
pub(super) struct CtxParts<'a> {
    pub(super) upstream_model: &'a str,
    pub(super) model_limits: Option<ModelLimits>,
    pub(super) forward_headers: &'a [(String, String)],
    pub(super) raw_body: Option<&'a Bytes>,
}

pub(super) fn outbound_ctx<'a>(
    upstream: &'a ResolvedUpstream<'a>,
    request: &'a CanonicalRequest,
    parts: CtxParts<'a>,
) -> OutboundCtx<'a> {
    OutboundCtx {
        route: upstream.route.as_ref(),
        endpoint: &upstream.provider.endpoint,
        api_key: &upstream.api_key,
        request,
        upstream_model: parts.upstream_model,
        model_limits: parts.model_limits,
        forward_headers: parts.forward_headers,
        raw_body: parts.raw_body,
    }
}

// Why: `metadata.user_id` is an end-user identifier meant for the provider the
// caller chose, so it must not reach a different wire's upstream. Stripped
// unconditionally on the canonical form because an adapter may decline the raw
// lane and fall back to the canonical build; the passthrough lane applies the
// same rule to the raw body in `normalize_raw_body`.
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

pub(super) async fn audit_upstream_failure(
    audit: &GatewayAudit,
    provider: &str,
    model: &str,
    error: &anyhow::Error,
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

// Why: Gemini's generateContent has no URL image part, so a URL that reaches
// the codec is downgraded to text and the model never sees the picture.
// Anthropic and OpenAI both accept a URL natively, so nothing is fetched for
// them and no other wire pays the latency.
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
            Err(DispatchError::Recorded(failure.into()))
        },
    }
}
