//! Outbound-context assembly, caller-identity stripping, and upstream-failure
//! auditing for the dispatch stages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use systemprompt_models::services::ai::ModelLimits;

use super::super::super::audit::GatewayAudit;
use super::super::super::protocol::canonical::CanonicalRequest;
use super::super::super::protocol::outbound::OutboundCtx;
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
