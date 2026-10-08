//! Pricing selection and dispatch tracing for upstream gateway requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::resolve::ResolvedUpstream;
use super::{CanonicalRequest, DispatchError, GatewayRequestContext};
use crate::pricing as model_pricing;
use systemprompt_manifest::services::ModelPricing;

pub(super) fn dispatch_pricing(
    request: &CanonicalRequest,
    upstream: &ResolvedUpstream<'_>,
) -> Result<ModelPricing, DispatchError> {
    model_pricing::resolve_upstream(&upstream.route, upstream.provider, request.model.as_str())
        .map_err(DispatchError::pre_audit)
}

pub(super) fn failover_pricing(
    upstream: &ResolvedUpstream<'_>,
    requested_model: &str,
) -> Result<ModelPricing, model_pricing::MissingPricing> {
    model_pricing::resolve_upstream(&upstream.route, upstream.provider, requested_model)
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
        provider = %upstream.route.provider,
        upstream = %upstream.provider.endpoint,
        wire_protocol = ctx.origin.wire.as_str(),
        client_kind = ctx.origin.client.as_str(),
        streaming = request.stream,
        "Gateway request dispatched"
    );
}
