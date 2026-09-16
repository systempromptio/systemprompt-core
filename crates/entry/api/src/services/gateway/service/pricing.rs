//! Pricing selection and dispatch tracing for upstream gateway requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::resolve::ResolvedUpstream;
use super::{CanonicalRequest, DispatchError, GatewayRequestContext};
use crate::services::gateway::pricing as model_pricing;
use systemprompt_models::services::{GatewayConfig, ModelPricing, ProviderRegistry};

pub(super) fn dispatch_pricing(
    config: &GatewayConfig,
    registry: &ProviderRegistry,
    request: &CanonicalRequest,
    upstream: &ResolvedUpstream<'_>,
) -> Result<ModelPricing, DispatchError> {
    model_pricing::resolve(
        upstream.route.provider.as_str(),
        &[request.model.as_str()],
        Some(config),
        registry,
    )
    .map_err(|error| DispatchError::PreAudit(error.into()))
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
