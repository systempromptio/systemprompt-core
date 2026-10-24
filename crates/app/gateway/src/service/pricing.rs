//! Pricing selection and dispatch tracing for upstream gateway requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::resolve::ResolvedUpstream;
use super::{CanonicalRequest, GatewayRequestContext};
use systemprompt_manifest::services::ModelPricing;

pub(super) const fn dispatch_pricing(upstream: &ResolvedUpstream<'_>) -> ModelPricing {
    crate::pricing::resolve_hop(&upstream.hop)
}

pub(super) fn trace_dispatch(
    ctx: &GatewayRequestContext,
    request: &CanonicalRequest,
    upstream: &ResolvedUpstream<'_>,
) {
    tracing::info!(
        ai_request_id = %ctx.ai_request_id,
        user_id = %ctx.user_id,
        model = %request.model,
        provider = %upstream.hop.provider,
        endpoint = %upstream.hop.endpoint_ref(),
        credential = %upstream.hop.credential.id,
        upstream = %upstream.hop.url,
        wire_protocol = ctx.origin.wire.as_str(),
        client_kind = ctx.origin.client.as_str(),
        streaming = request.stream,
        "Gateway request dispatched"
    );
}
