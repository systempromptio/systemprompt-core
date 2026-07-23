//! Gateway dispatch entry point: route resolution, policy and quota checks,
//! upstream send, and response finalization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
#![expect(
    clippy::clone_on_ref_ptr,
    reason = "Arc::clone usage is intentional and ergonomic in this gateway dispatch path"
)]

mod finalize;
mod resolve;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::finalize::{apply_system_prompt_override, attach_request_id};
}

use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::body::Body;
use axum::response::Response;
use bytes::Bytes;
use systemprompt_ai::SafetyConfig;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, UserId};
use systemprompt_models::profile::{GatewayConfig, ProviderRegistry};

use self::finalize::{
    FinalizeCtx, apply_system_prompt_override, attach_request_id, finalize, run_request_safety_scan,
};
use self::resolve::{ResolvedUpstream, resolve_upstream};
use super::audit::{GatewayAudit, GatewayRequestContext};
use super::policy::{PolicyResolver, QuotaWindow};
use super::protocol::canonical::CanonicalRequest;
use super::protocol::inbound::InboundAdapter;
use super::protocol::outbound::{OutboundCtx, OutboundOutcome};
use super::quota;

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
pub enum DispatchError {
    #[error(transparent)]
    PreAudit(anyhow::Error),
    #[error(transparent)]
    Recorded(anyhow::Error),
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

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct SafetyBlocked {
    pub category: String,
    pub message: String,
}

impl GatewayService {
    pub async fn dispatch(
        config: &GatewayConfig,
        registry: &ProviderRegistry,
        db: &DbPool,
        inputs: DispatchInputs,
    ) -> Result<Response<Body>, DispatchError> {
        let DispatchInputs {
            mut request,
            raw_body,
            ctx,
            inbound,
        } = inputs;
        if ctx.session_id.is_none() {
            return Err(DispatchError::PreAudit(anyhow!(
                "gateway dispatch missing conversation binding (session_id)"
            )));
        }

        let ai_request_id = ctx.ai_request_id.clone();
        let upstream = resolve_upstream(config, registry, &request, &ai_request_id).await?;

        tracing::info!(
            ai_request_id = %ai_request_id,
            user_id = %ctx.user_id,
            model = %request.model,
            provider = %upstream.route.provider,
            upstream = %upstream.provider.endpoint,
            wire_protocol = %ctx.wire_protocol,
            streaming = request.stream,
            "Gateway request dispatched"
        );

        let resolver = PolicyResolver::new(db).map_err(DispatchError::PreAudit)?;
        let policy = resolver.resolve().await;

        let audit = Arc::new(
            GatewayAudit::new(db, ctx.clone())
                .map_err(|e| DispatchError::PreAudit(anyhow!("audit init failed: {e}")))?,
        );

        if let Err(e) = audit.open(&request, &raw_body).await {
            tracing::error!(error = %e, "audit open failed — proceeding without audit row");
        }

        if let Some(descriptor) = upstream.route_match_descriptor.as_deref() {
            audit.set_route_match(descriptor).await;
        }

        enforce_quota(db, &ctx.user_id, &policy.quota_windows, &audit).await?;
        enforce_request_guards(db, &ctx.user_id, &audit).await?;
        enforce_request_safety(db, &ai_request_id, &request, &policy.safety, &audit).await?;

        let outcome = send_to_upstream(config, &upstream, &mut request, &audit).await?;

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

async fn send_to_upstream(
    config: &GatewayConfig,
    upstream: &ResolvedUpstream<'_>,
    request: &mut CanonicalRequest,
    audit: &GatewayAudit,
) -> Result<OutboundOutcome, DispatchError> {
    let upstream_model = upstream
        .route
        .effective_upstream_model(&request.model)
        .to_owned();
    if let Some(descriptor) =
        apply_system_prompt_override(config, &upstream.provider.name, &upstream_model, request)
            .await
    {
        audit.set_system_prompt_override(&descriptor).await;
    }
    let model_limits = upstream
        .provider
        .find_model(&upstream_model)
        .map(|m| m.limits);
    let outbound_ctx = OutboundCtx {
        route: upstream.route.as_ref(),
        endpoint: &upstream.provider.endpoint,
        api_key: upstream.api_key,
        request,
        upstream_model: &upstream_model,
        model_limits,
    };

    match upstream.adapter.send(outbound_ctx).await {
        Ok(o) => Ok(o),
        Err(e) => {
            audit_upstream_failure(audit, upstream.provider.name.as_str(), &request.model, &e)
                .await;
            Err(DispatchError::Recorded(e))
        },
    }
}

async fn enforce_quota(
    db: &DbPool,
    user_id: &UserId,
    quota_windows: &[QuotaWindow],
    audit: &GatewayAudit,
) -> Result<(), DispatchError> {
    let reservation = quota::precheck_and_reserve(db, user_id, quota_windows)
        .await
        .map_err(DispatchError::Recorded)?;
    let Some(decision) = reservation else {
        return Ok(());
    };
    if decision.allow {
        return Ok(());
    }
    let msg = format!(
        "quota exceeded for window {}s (used {}/{:?})",
        decision.window_seconds, decision.state.requests, decision.limit_requests
    );
    if let Err(e) = audit.fail(&msg).await {
        tracing::warn!(error = %e, "quota audit fail failed");
    }
    Err(DispatchError::Recorded(
        QuotaExceeded {
            message: msg,
            retry_after_seconds: decision.window_seconds,
        }
        .into(),
    ))
}

async fn enforce_request_guards(
    db: &DbPool,
    user_id: &UserId,
    audit: &GatewayAudit,
) -> Result<(), DispatchError> {
    let Some(pool) = db.pool() else {
        return Ok(());
    };
    let Err(deny) = systemprompt_extension::run_gateway_guards(&pool, user_id.as_str()).await
    else {
        return Ok(());
    };
    tracing::warn!(
        user_id = %user_id,
        reason = %deny.message,
        "Gateway request denied by request guard"
    );
    if let Err(e) = audit.fail(&deny.message).await {
        tracing::warn!(error = %e, "request-guard audit fail failed");
    }
    Err(DispatchError::Recorded(
        QuotaExceeded {
            message: deny.message,
            retry_after_seconds: deny.retry_after_seconds,
        }
        .into(),
    ))
}

async fn enforce_request_safety(
    db: &DbPool,
    ai_request_id: &AiRequestId,
    request: &CanonicalRequest,
    safety: &SafetyConfig,
    audit: &GatewayAudit,
) -> Result<(), DispatchError> {
    let findings = run_request_safety_scan(db, ai_request_id, request, safety).await;
    let Some(finding) = findings
        .iter()
        .find(|f| safety.block_categories.contains(&f.category))
    else {
        return Ok(());
    };
    let msg = format!(
        "request blocked by safety policy: category '{}'",
        finding.category
    );
    tracing::warn!(
        ai_request_id = %ai_request_id,
        category = %finding.category,
        scanner = %finding.scanner,
        "Gateway blocked request by safety policy"
    );
    if let Err(e) = audit.fail(&msg).await {
        tracing::warn!(error = %e, "safety-block audit fail failed");
    }
    Err(DispatchError::Recorded(
        SafetyBlocked {
            category: finding.category.clone(),
            message: msg,
        }
        .into(),
    ))
}

async fn audit_upstream_failure(
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
