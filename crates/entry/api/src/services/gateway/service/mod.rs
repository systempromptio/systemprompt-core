//! Gateway dispatch entry point: route resolution, policy and quota checks,
//! upstream send, and response finalization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
#![expect(
    clippy::clone_on_ref_ptr,
    reason = "Arc::clone usage is intentional and ergonomic in this gateway dispatch path"
)]

pub mod abandon;
pub mod credentials;
mod error;
mod guards;
pub mod finalize;
mod pricing;
pub mod resolve;
pub mod stages;

pub use self::error::{
    DispatchError, GovernanceDenied, GuardForbidden, GuardUnavailable, PolicyDenied,
    PromptRepairRequired, QuotaExceeded, SafetyBlocked,
};
pub(super) use self::finalize::run_response_safety_scan;

use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::body::Body;
use axum::response::Response;
use bytes::Bytes;
use systemprompt_database::DbPool;
use systemprompt_models::services::{GatewayConfig, ProviderRegistry, QuotaFaultMode};

use self::abandon::AbandonGuard;
use self::finalize::{FinalizeCtx, attach_request_id, finalize};
use self::guards::{enforce_quota, enforce_request_guards};
use self::pricing::{dispatch_pricing, trace_dispatch};
use self::resolve::{ResolvedUpstream, resolve_upstream};
use self::stages::{
    GovernedDispatch, PreparedDispatch, ScannedDispatch, UpstreamRelay,
};
use super::audit::{GatewayAudit, GatewayRequestContext};
use super::policy::{GatewayPolicySpec, PolicyResolver};
use super::protocol::canonical::CanonicalRequest;
use super::protocol::inbound::InboundAdapter;

pub const REQUEST_ID_HEADER: &str = "x-systemprompt-request-id";
pub const RECOVERY_COUNT_HEADER: &str = "x-systemprompt-recovery-count";

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
        let (policy, evaluation_session) =
            dispatch_policy(repos, &ctx, config.quota_fault_mode).await?;
        let stream_usage = inbound.wants_stream_usage(&raw_body);
        let ai_request_id = ctx.ai_request_id.clone();
        let upstream = resolve_upstream(config, registry, &request, &ai_request_id).await?;
        let pricing = dispatch_pricing(config, registry, &request, &upstream, evaluation_session)?;

        trace_dispatch(&ctx, &request, &upstream);
        let audit = open_audit(repos, &ctx, &request, &raw_body, &identity_headers).await?;
        let mut guard = AbandonGuard::arm(Arc::clone(&audit));
        let result = dispatch_opened(OpenedDispatch {
            config,
            db,
            repos,
            audit,
            policy,
            stream_usage,
            ai_request_id,
            upstream,
            pricing,
            request,
            raw_body,
            ctx,
            inbound,
            forward_headers,
        })
        .await;
        // Why: every `Err` from the opened dispatch has already recorded itself
        // on the audit row, and an `Ok` has handed the row to its completion
        // task or stream tap. The guard is for the third outcome — the future
        // being dropped before it returns either.
        guard.disarm();
        result
    }
}

struct OpenedDispatch<'a> {
    config: &'a GatewayConfig,
    db: &'a DbPool,
    repos: &'a super::GatewayRepositories,
    audit: Arc<GatewayAudit>,
    policy: GatewayPolicySpec,
    stream_usage: bool,
    ai_request_id: systemprompt_identifiers::AiRequestId,
    upstream: ResolvedUpstream<'a>,
    pricing: systemprompt_models::services::ModelPricing,
    request: CanonicalRequest,
    raw_body: Bytes,
    ctx: GatewayRequestContext,
    inbound: Arc<dyn InboundAdapter>,
    forward_headers: Vec<(String, String)>,
}

async fn dispatch_opened(opened: OpenedDispatch<'_>) -> Result<Response<Body>, DispatchError> {
    let OpenedDispatch {
        config,
        db,
        repos,
        audit,
        policy,
        stream_usage,
        ai_request_id,
        upstream,
        pricing,
        request,
        raw_body,
        ctx,
        inbound,
        forward_headers,
    } = opened;
    audit
        .pin_pricing(pricing)
        .map_err(DispatchError::PreAudit)?;

    if let Some(descriptor) = upstream.route_match_descriptor.as_deref() {
        audit.set_route_match(descriptor).await;
    }

    enforce_quota(db, repos, &policy, &audit, config.quota_fault_mode).await?;
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
        ScannedDispatch::enforce(governed, repos, &ai_request_id, &policy.safety, &audit).await?;

    let evaluation = scanned.admit_evaluation(repos, &ctx, &pricing).await?;
    let retry_policy = if evaluation {
        super::protocol::outbound::retry::RetryPolicy::none()
    } else {
        super::protocol::outbound::retry::current_policy()
    };
    let outcome = super::protocol::outbound::retry::with_policy(
        retry_policy,
        scanned.send(&upstream, &forward_headers, &audit),
    )
    .await?;

    let mut response = finalize(
        outcome,
        FinalizeCtx {
            audit: Arc::clone(&audit),
            db: db.clone(),
            repos: repos.clone(),
            ai_request_id: ai_request_id.clone(),
            policy,
            quota_fault_mode: config.quota_fault_mode,
            inbound,
            request_model: scanned.request_model().to_owned(),
            stream_usage,
        },
    )
    .await;
    stages::recovery::attach_recovery_count(&mut response, scanned.recovery_count());
    Ok(attach_request_id(response, &ai_request_id))
}

async fn dispatch_policy(
    repos: &super::GatewayRepositories,
    ctx: &GatewayRequestContext,
    fault_mode: QuotaFaultMode,
) -> Result<(GatewayPolicySpec, bool), DispatchError> {
    if ctx.session_id.is_none() {
        return Err(DispatchError::PreAudit(anyhow!(
            "gateway dispatch missing authenticated session (session_id)"
        )));
    }

    let resolver = PolicyResolver::from_repository(repos.gateway_policies.clone());
    let policy = resolver
        .resolve(fault_mode)
        .await
        .map_err(|e| DispatchError::PreAudit(anyhow!(PolicyDenied(e.to_string()))))?;
    let evaluation_session = super::evaluation::preflight(repos, ctx, &policy)
        .await
        .map_err(DispatchError::PreAudit)?;
    Ok((policy, evaluation_session))
}

async fn open_audit(
    repos: &super::GatewayRepositories,
    ctx: &GatewayRequestContext,
    request: &CanonicalRequest,
    raw_body: &Bytes,
    identity_headers: &[(String, String)],
) -> Result<Arc<GatewayAudit>, DispatchError> {
    let audit = Arc::new(GatewayAudit::new(repos, ctx.clone()));
    if let Err(error) = audit.open(request, raw_body).await {
        if let Err(settlement_error) = audit
            .fail("Gateway admission failed before provider dispatch")
            .await
        {
            tracing::error!(%settlement_error, "Could not record failed gateway admission");
        }
        return Err(DispatchError::PreAudit(error));
    }
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
