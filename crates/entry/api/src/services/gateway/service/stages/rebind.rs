//! The failover half of a scanned dispatch: an upstream attempt that leaves
//! the audit row open for a second try, and re-binding the governed request
//! to a different provider's wire.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::services::providers::WireProtocol;

use super::outbound::{CtxParts, outbound_ctx, send_attempt};
use super::{ScannedDispatch, automatic_prompt_caching};
use crate::services::gateway::audit::GatewayAudit;
use crate::services::gateway::image_fetch::{ImageFetchPolicy, inline_url_images};
use crate::services::gateway::protocol::outbound::OutboundOutcome;
use crate::services::gateway::service::resolve::ResolvedUpstream;

impl ScannedDispatch {
    pub(in crate::services::gateway::service) async fn send_attempt(
        &self,
        upstream: &ResolvedUpstream<'_>,
        forward_headers: &[(String, String)],
        audit: &GatewayAudit,
    ) -> anyhow::Result<OutboundOutcome> {
        let prepared = &self.0;
        let ctx = outbound_ctx(
            upstream,
            &prepared.request,
            CtxParts {
                upstream_model: &prepared.upstream_model,
                model_limits: prepared.model_limits,
                automatic_prompt_caching: false,
                forward_headers,
                raw_body: None,
            },
        );
        send_attempt(upstream, ctx, &prepared.body, audit).await
    }

    // Why: the governed, scanned canonical request is the one thing every wire
    // can be built from, so a failover to a different provider rebuilds the
    // body from it (never the raw passthrough lane, which is bound to the wire
    // the client spoke) without re-running governance or the scanners — the
    // request content they judged is unchanged.
    pub(in crate::services::gateway::service) async fn rebind(
        &mut self,
        upstream: &ResolvedUpstream<'_>,
        audit: &GatewayAudit,
    ) -> anyhow::Result<()> {
        let prepared = &mut self.0;
        let requested = prepared.request.model.as_str().to_owned();
        let upstream_model = upstream
            .provider
            .upstream_model_for(upstream.route.upstream_model.as_deref(), &requested)
            .to_owned();
        let model_limits = upstream
            .provider
            .find_served_model(&requested)
            .map(|m| m.limits);
        if upstream.provider.wire == WireProtocol::Gemini {
            inline_url_images(&mut prepared.request, &ImageFetchPolicy::default()).await?;
        }
        let ctx = outbound_ctx(
            upstream,
            &prepared.request,
            CtxParts {
                upstream_model: &upstream_model,
                model_limits,
                automatic_prompt_caching: automatic_prompt_caching(
                    prepared.automatic_prompt_caching_enabled,
                    upstream,
                    &upstream_model,
                ),
                forward_headers: &[],
                raw_body: None,
            },
        );
        let body = upstream.adapter.build_body(&ctx)?;
        audit.set_prepared_body_digest(&body.bytes).await;
        prepared.upstream_model = upstream_model;
        prepared.model_limits = model_limits;
        prepared.body = body;
        Ok(())
    }
}
