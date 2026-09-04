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
mod stages;

pub(super) use self::finalize::run_response_safety_scan;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::blocks_at_phase;
    pub use super::finalize::{apply_system_prompt_override, attach_request_id, dedupe_findings};
    pub use super::resolve::{describe_route_match, enforce_route_requirements};
}

use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::body::Body;
use axum::response::Response;
use bytes::Bytes;
use systemprompt_ai::{PHASE_REQUEST, PHASE_REQUEST_HISTORY, SafetyHistoryMode};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::services::{GatewayConfig, ProviderRegistry};

use self::finalize::{FinalizeCtx, attach_request_id, finalize};
use self::resolve::{ResolvedUpstream, resolve_upstream};
use self::stages::{
    GovernedDispatch, PreparedDispatch, ScannedDispatch, UpstreamRelay, record_quota_warning,
};
use super::audit::{GatewayAudit, GatewayRequestContext};
use super::policy::{GatewayPolicySpec, PolicyResolver};
use super::protocol::canonical::CanonicalRequest;
use super::protocol::inbound::InboundAdapter;
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
    pub forward_headers: Vec<(String, String)>,
    pub identity_headers: Vec<(String, String)>,
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
pub struct GuardForbidden {
    pub message: String,
}

/// A denial from the typed four-stage governance chain — the same engine and
/// the same operator-configured policies that govern MCP tool calls.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct GovernanceDenied {
    pub policy: String,
    pub message: String,
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
        repos: &super::GatewayRepositories,
        inputs: DispatchInputs,
    ) -> Result<Response<Body>, DispatchError> {
        let DispatchInputs {
            request,
            raw_body,
            ctx,
            inbound,
            forward_headers,
            identity_headers,
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

        let resolver = PolicyResolver::from_repository(repos.gateway_policies.clone());
        let policy = resolver.resolve().await;

        let audit = open_audit(repos, &ctx, &request, &raw_body, &identity_headers).await?;

        if let Some(descriptor) = upstream.route_match_descriptor.as_deref() {
            audit.set_route_match(descriptor).await;
        }

        enforce_quota(db, repos, &ctx, &policy, &audit).await?;
        enforce_request_guards(db, &ctx.user_id, &upstream, &request, &audit).await?;

        let prepared = PreparedDispatch::build(
            config,
            &upstream,
            request,
            &audit,
            UpstreamRelay {
                raw_body: &raw_body,
                inbound: inbound.as_ref(),
            },
        )
        .await?;
        let governed = GovernedDispatch::enforce(prepared, db, &ctx, &audit).await?;
        let scanned =
            ScannedDispatch::enforce(governed, repos, &ai_request_id, &policy.safety, &audit)
                .await?;

        let outcome = scanned.send(&upstream, &forward_headers, &audit).await?;

        let response = finalize(
            outcome,
            FinalizeCtx {
                audit: Arc::clone(&audit),
                db: db.clone(),
                repos: repos.clone(),
                ai_request_id: ai_request_id.clone(),
                policy,
                inbound,
                request_model: scanned.request_model().to_owned(),
            },
        )
        .await;
        Ok(attach_request_id(response, &ai_request_id))
    }
}

async fn open_audit(
    repos: &super::GatewayRepositories,
    ctx: &GatewayRequestContext,
    request: &CanonicalRequest,
    raw_body: &Bytes,
    identity_headers: &[(String, String)],
) -> Result<Arc<GatewayAudit>, DispatchError> {
    let audit = Arc::new(GatewayAudit::new(repos, ctx.clone()));
    if let Err(e) = audit.open(request, raw_body).await {
        tracing::error!(error = %e, "audit open failed — proceeding without audit row");
    }
    // Why: identity headers are recorded against the audit row, then dropped
    // before the upstream send so a third-party provider never receives them.
    if !identity_headers.is_empty() {
        tracing::info!(
            ai_request_id = %ctx.ai_request_id,
            user_id = %ctx.user_id,
            headers = ?identity_headers,
            "Gateway consumed client identity headers"
        );
    }
    Ok(audit)
}

async fn enforce_quota(
    db: &DbPool,
    repos: &super::GatewayRepositories,
    ctx: &GatewayRequestContext,
    policy: &GatewayPolicySpec,
    audit: &GatewayAudit,
) -> Result<(), DispatchError> {
    let reservation = quota::precheck_and_reserve(
        db,
        &repos.quota_buckets,
        &ctx.user_id,
        &policy.quota_windows,
    )
    .await
    .map_err(DispatchError::Recorded)?;
    let Some(decision) = reservation else {
        return Ok(());
    };
    if decision.allow {
        return Ok(());
    }
    // Why: warn mode on the quota plane. The window was reserved against and
    // the ceiling was breached exactly as under enforce; only the refusal is
    // dropped, and the breach lands in `governance_decisions` under policy
    // `quota` so the report can price what enforcement would have cost.
    if policy.quota_mode.is_warn() {
        tracing::warn!(
            ai_request_id = %ctx.ai_request_id,
            user_id = %ctx.user_id,
            window_seconds = decision.window_seconds,
            reason = %decision.message,
            "Gateway quota window exhausted in warn mode; allowing the request"
        );
        record_quota_warning(db, ctx, &decision.message).await;
        return Ok(());
    }
    let msg = decision.message;
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
    upstream: &ResolvedUpstream<'_>,
    request: &CanonicalRequest,
    audit: &GatewayAudit,
) -> Result<(), DispatchError> {
    let Some(pool) = db.pool() else {
        return Ok(());
    };
    let guard_request = systemprompt_extension::GatewayGuardRequest {
        user_id: user_id.as_str(),
        model: &request.model,
        route_id: Some(upstream.route.id.as_str()),
        provider: upstream.route.provider.as_str(),
        streaming: request.stream,
    };
    let Err(deny) = systemprompt_extension::run_gateway_guards(&pool, &guard_request).await else {
        return Ok(());
    };
    tracing::warn!(
        user_id = %user_id,
        model = %request.model,
        route_id = %upstream.route.id,
        kind = ?deny.kind,
        reason = %deny.message,
        "Gateway request denied by request guard"
    );
    if let Err(e) = audit.fail(&deny.message).await {
        tracing::warn!(error = %e, "request-guard audit fail failed");
    }
    let inner: anyhow::Error = match deny.kind {
        systemprompt_extension::GatewayDenyKind::Forbidden => GuardForbidden {
            message: deny.message,
        }
        .into(),
        systemprompt_extension::GatewayDenyKind::Quota => QuotaExceeded {
            message: deny.message,
            retry_after_seconds: deny.retry_after_seconds,
        }
        .into(),
    };
    Err(DispatchError::Recorded(inner))
}

pub fn blocks_at_phase(phase: &str, history: SafetyHistoryMode) -> bool {
    match phase {
        PHASE_REQUEST => true,
        PHASE_REQUEST_HISTORY => history == SafetyHistoryMode::Block,
        _ => false,
    }
}
