//! Gateway dispatch entry point: route resolution, policy and quota checks,
//! upstream send, and response finalization.
#![expect(
    clippy::clone_on_ref_ptr,
    reason = "Arc::clone usage is intentional and ergonomic in this gateway dispatch path"
)]

mod finalize;

use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::body::Body;
use axum::response::Response;
use bytes::Bytes;
use systemprompt_database::DbPool;
use systemprompt_models::profile::{GatewayConfig, ProviderRegistry};

use self::finalize::{FinalizeCtx, attach_request_id, finalize, run_request_safety_scan};
use super::audit::{GatewayAudit, GatewayRequestContext};
use super::policy::PolicyResolver;
use super::protocol::canonical::CanonicalRequest;
use super::protocol::inbound::InboundAdapter;
use super::protocol::outbound::OutboundCtx;
use super::quota;
use super::registry::GatewayUpstreamRegistry;

pub const REQUEST_ID_HEADER: &str = "x-systemprompt-request-id";

#[derive(Debug, Clone, Copy)]
pub struct GatewayService;

#[derive(Debug)]
pub struct DispatchInputs {
    pub request: CanonicalRequest,
    pub raw_body: Bytes,
    pub ctx: GatewayRequestContext,
    pub inbound: Arc<dyn InboundAdapter>,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct PolicyDenied(pub String);

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct QuotaExceeded {
    pub message: String,
    pub retry_after_seconds: i32,
}

impl GatewayService {
    pub async fn dispatch(
        config: &GatewayConfig,
        registry: &ProviderRegistry,
        db: &DbPool,
        inputs: DispatchInputs,
    ) -> Result<Response<Body>> {
        let DispatchInputs {
            request,
            raw_body,
            ctx,
            inbound,
        } = inputs;
        if ctx.session_id.is_none() {
            return Err(anyhow!(
                "gateway dispatch missing conversation binding (session_id)"
            ));
        }

        let ai_request_id = ctx.ai_request_id.clone();

        if !config.is_model_exposed(registry, &request.model) {
            tracing::warn!(
                ai_request_id = %ai_request_id,
                model = %request.model,
                "Gateway denied: model not exposed by gateway policy or registry"
            );
            return Err(PolicyDenied(format!(
                "model '{}' is not permitted by gateway policy",
                request.model
            ))
            .into());
        }

        let route = config
            .resolve_route(registry, &request.model)
            .ok_or_else(|| anyhow!("No gateway route matches model '{}'", request.model))?;

        let provider = route.resolve(registry).ok_or_else(|| {
            anyhow!(
                "Gateway route '{}' provider '{}' is not declared in profile.providers",
                route.id.as_str(),
                route.provider.as_str()
            )
        })?;

        let secrets = systemprompt_config::SecretsBootstrap::get()
            .map_err(|e| anyhow!("Secrets not available: {e}"))?;

        let upstream_api_key = secrets
            .get(provider.api_key_secret.as_str())
            .ok_or_else(|| {
                anyhow!(
                    "Gateway API key secret '{}' not configured",
                    provider.api_key_secret.as_str()
                )
            })?;

        let upstream = GatewayUpstreamRegistry::global()
            .get(provider.protocol.as_tag())
            .ok_or_else(|| {
                anyhow!(
                    "Gateway has no outbound adapter for wire protocol '{}'",
                    provider.protocol.as_tag()
                )
            })?;

        let is_streaming = request.stream;

        tracing::info!(
            ai_request_id = %ai_request_id,
            user_id = %ctx.user_id,
            model = %request.model,
            provider = %route.provider,
            upstream = %provider.endpoint,
            wire_protocol = %ctx.wire_protocol,
            streaming = is_streaming,
            "Gateway request dispatched"
        );

        let resolver = PolicyResolver::new(db)?;
        let policy = resolver.resolve().await;

        let audit = Arc::new(
            GatewayAudit::new(db, ctx.clone()).map_err(|e| anyhow!("audit init failed: {e}"))?,
        );

        if let Err(e) = audit.open(&request, &raw_body).await {
            tracing::error!(error = %e, "audit open failed — proceeding without audit row");
        }

        if let Some(decision) =
            quota::precheck_and_reserve(db, &ctx.user_id, &policy.quota_windows).await?
        {
            if !decision.allow {
                let msg = format!(
                    "quota exceeded for window {}s (used {}/{:?})",
                    decision.window_seconds, decision.state.requests, decision.limit_requests
                );
                if let Err(e) = audit.fail(&msg).await {
                    tracing::warn!(error = %e, "quota audit fail failed");
                }
                return Err(QuotaExceeded {
                    message: msg,
                    retry_after_seconds: decision.window_seconds,
                }
                .into());
            }
        }

        run_request_safety_scan(db, &ai_request_id, &request).await;

        let upstream_model = route.effective_upstream_model(&request.model).to_owned();
        let outbound_ctx = OutboundCtx {
            route: route.as_ref(),
            endpoint: &provider.endpoint,
            api_key: upstream_api_key,
            request: &request,
            upstream_model: &upstream_model,
        };

        let outcome = match upstream.send(outbound_ctx).await {
            Ok(o) => o,
            Err(e) => {
                if let Err(audit_err) = audit.fail(&e.to_string()).await {
                    tracing::warn!(error = %audit_err, "upstream audit fail failed");
                }
                return Err(e);
            },
        };

        let response = finalize(
            outcome,
            FinalizeCtx {
                audit: Arc::clone(&audit),
                db: db.clone(),
                ai_request_id: ai_request_id.clone(),
                policy,
                inbound,
                request_model: request.model.clone(),
            },
        )
        .await;
        Ok(attach_request_id(response, &ai_request_id))
    }
}
