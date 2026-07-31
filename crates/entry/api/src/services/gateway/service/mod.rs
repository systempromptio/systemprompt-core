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

pub(super) use self::finalize::run_response_safety_scan;

#[cfg(feature = "test-api")]
pub mod test_api {
    pub use super::blocks_at_phase;
    pub use super::finalize::{apply_system_prompt_override, attach_request_id, dedupe_findings};
}

use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::body::Body;
use axum::response::Response;
use bytes::Bytes;
use systemprompt_ai::{PHASE_REQUEST, PHASE_REQUEST_HISTORY, SafetyConfig, SafetyHistoryMode};
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
use super::protocol::outbound::{OutboundCtx, OutboundOutcome, PreparedBody};
use super::quota;
use systemprompt_identifiers::{CallId, SessionId};
use systemprompt_models::services::ai::ModelLimits;
use systemprompt_models::wire::inspect;
use systemprompt_security::authz::types::Decision;
use systemprompt_security::policy::types::AccessScope;
use systemprompt_security::policy::{
    AgentScope, AuditOrigin, AuditTarget, ChainEntryResult, DecisionAudit, Evaluation,
    GovernanceEngine, GovernedInput, GovernedTarget, PolicyContext, PrincipalSnapshot,
    record_decision,
};

pub const REQUEST_ID_HEADER: &str = "x-systemprompt-request-id";

#[derive(Debug, Clone, Copy)]
pub struct GatewayService;

#[derive(Debug)]
pub struct DispatchInputs {
    pub request: CanonicalRequest,
    pub raw_body: Bytes,
    pub ctx: GatewayRequestContext,
    pub inbound: Arc<dyn InboundAdapter>,
    /// Caller headers cleared for verbatim relay to the upstream.
    pub forward_headers: Vec<(String, String)>,
    /// Caller headers that identify the client, user, or session. Recorded on
    /// the audit row and never sent upstream.
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
        inputs: DispatchInputs,
    ) -> Result<Response<Body>, DispatchError> {
        let DispatchInputs {
            mut request,
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

        let resolver = PolicyResolver::new(db).map_err(DispatchError::PreAudit)?;
        let policy = resolver.resolve().await;

        let audit = open_audit(db, &ctx, &request, &raw_body, &identity_headers).await?;

        if let Some(descriptor) = upstream.route_match_descriptor.as_deref() {
            audit.set_route_match(descriptor).await;
        }

        enforce_quota(db, &ctx.user_id, &policy.quota_windows, &audit).await?;
        enforce_request_guards(db, &ctx.user_id, &upstream, &request, &audit).await?;

        // Why: the payload is built before the scan so governance inspects the
        // exact bytes that will go on the wire. Scanning the canonical form and
        // sending something derived from it separately is how the two drift.
        let prepared = prepare_payload(
            config,
            &upstream,
            &mut request,
            &audit,
            UpstreamRelay {
                raw_body: &raw_body,
                inbound: inbound.as_ref(),
            },
        )
        .await?;
        audit.set_prepared_body_digest(&prepared.body.bytes).await;
        attach_forwarded_surface(&mut request, &prepared, &ai_request_id);

        // Why: governance runs before the scanner plane so first-deny-wins holds
        // across both — a denied request never reaches the scanners, and so
        // produces exactly one audit row and one 403.
        enforce_governance(db, &ctx, &request, &audit).await?;
        enforce_request_safety(db, &ai_request_id, &request, &policy.safety, &audit).await?;

        let outcome =
            send_to_upstream(&upstream, &request, &prepared, &forward_headers, &audit).await?;

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

struct UpstreamRelay<'a> {
    raw_body: &'a Bytes,
    inbound: &'a dyn InboundAdapter,
}

async fn open_audit(
    db: &DbPool,
    ctx: &GatewayRequestContext,
    request: &CanonicalRequest,
    raw_body: &Bytes,
    identity_headers: &[(String, String)],
) -> Result<Arc<GatewayAudit>, DispatchError> {
    let audit = Arc::new(
        GatewayAudit::new(db, ctx.clone())
            .map_err(|e| DispatchError::PreAudit(anyhow!("audit init failed: {e}")))?,
    );
    if let Err(e) = audit.open(request, raw_body).await {
        tracing::error!(error = %e, "audit open failed — proceeding without audit row");
    }
    // Why: these headers identify the client, user, and any spawning agent.
    // They are recorded here, against the audit row's request id, and then
    // dropped before the upstream send so a third-party provider never receives
    // them. Emitted on the trace rather than as an `ai_requests` column because
    // the attribution they add is per-agent, not per-request.
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

#[derive(Clone, Copy)]
struct CtxParts<'a> {
    upstream_model: &'a str,
    model_limits: Option<ModelLimits>,
    forward_headers: &'a [(String, String)],
    raw_body: Option<&'a Bytes>,
}

struct Prepared {
    upstream_model: String,
    model_limits: Option<ModelLimits>,
    body: PreparedBody,
}

async fn prepare_payload(
    config: &GatewayConfig,
    upstream: &ResolvedUpstream<'_>,
    request: &mut CanonicalRequest,
    audit: &GatewayAudit,
    relay: UpstreamRelay<'_>,
) -> Result<Prepared, DispatchError> {
    let upstream_model = upstream
        .route
        .effective_upstream_model(&request.model)
        .to_owned();
    let override_descriptor =
        apply_system_prompt_override(config, &upstream.provider.name, &upstream_model, request)
            .await;
    if let Some(descriptor) = &override_descriptor {
        audit.set_system_prompt_override(descriptor).await;
    }
    let model_limits = upstream
        .provider
        .find_model(&upstream_model)
        .map(|m| m.limits);
    // Why: an applied override rewrote the canonical request, so the caller's
    // original bytes no longer describe what the gateway decided to send.
    let raw_body = (override_descriptor.is_none()
        && relay.inbound.passthrough_wire() == Some(upstream.provider.wire))
    .then_some(relay.raw_body);
    // Why: unconditional, because an adapter may decline the raw lane and fall
    // back to the canonical build. Stripping only when the lane was rejected up
    // front would leave that fallback forwarding the identity.
    strip_caller_identity(request);

    let ctx = outbound_ctx(
        upstream,
        request,
        CtxParts {
            upstream_model: &upstream_model,
            model_limits,
            forward_headers: &[],
            raw_body,
        },
    );
    let body = upstream
        .adapter
        .build_body(&ctx)
        .map_err(DispatchError::Recorded)?;
    Ok(Prepared {
        upstream_model,
        model_limits,
        body,
    })
}

fn attach_forwarded_surface(
    request: &mut CanonicalRequest,
    prepared: &Prepared,
    ai_request_id: &AiRequestId,
) {
    let surface = inspect::string_leaves(&prepared.body.bytes, inspect::SurfaceBudget::default());
    if surface.truncated() {
        tracing::warn!(
            ai_request_id = %ai_request_id,
            leaves = surface.len(),
            "Gateway inspection surface truncated — part of the forwarded body was not scanned"
        );
    }
    request.forwarded_surface = surface;
}

fn outbound_ctx<'a>(
    upstream: &'a ResolvedUpstream<'a>,
    request: &'a CanonicalRequest,
    parts: CtxParts<'a>,
) -> OutboundCtx<'a> {
    OutboundCtx {
        route: upstream.route.as_ref(),
        endpoint: &upstream.provider.endpoint,
        api_key: upstream.api_key,
        request,
        upstream_model: parts.upstream_model,
        model_limits: parts.model_limits,
        forward_headers: parts.forward_headers,
        raw_body: parts.raw_body,
    }
}

async fn send_to_upstream(
    upstream: &ResolvedUpstream<'_>,
    request: &CanonicalRequest,
    prepared: &Prepared,
    forward_headers: &[(String, String)],
    audit: &GatewayAudit,
) -> Result<OutboundOutcome, DispatchError> {
    let ctx = outbound_ctx(
        upstream,
        request,
        CtxParts {
            upstream_model: &prepared.upstream_model,
            model_limits: prepared.model_limits,
            forward_headers,
            raw_body: None,
        },
    );
    match upstream.adapter.send(ctx, &prepared.body).await {
        Ok(o) => Ok(o),
        Err(e) => {
            audit_upstream_failure(audit, upstream.provider.name.as_str(), &request.model, &e)
                .await;
            Err(DispatchError::Recorded(e))
        },
    }
}

// Why: `metadata.user_id` is an end-user identifier meant for the provider the
// caller chose, so it must not reach a different wire's upstream. The
// passthrough lane applies the same rule to the raw body in
// `normalize_raw_body`.
fn strip_caller_identity(request: &mut CanonicalRequest) {
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

// Why: written on allow as well as on deny — the chain trace is the product,
// not just the refusals.
async fn record_governance_decision(
    db: &DbPool,
    ctx: &GatewayRequestContext,
    evaluation: Evaluation,
    call_id: CallId,
    session_id: SessionId,
) {
    let decision_audit = DecisionAudit {
        id: uuid::Uuid::new_v4().to_string(),
        call_id: call_id.as_str().to_owned(),
        origin: AuditOrigin::Governed,
        decision: evaluation.decision,
        principal: PrincipalSnapshot {
            user_id: ctx.user_id.clone(),
            session_id,
            agent_session: None,
            agent_id: None,
            agent_scope: AccessScope::Unknown,
        },
        target: AuditTarget {
            tool_name: GovernedTarget::Prompt.as_str().to_owned(),
            plugin_id: None,
        },
        chain: evaluation.chain,
        approver: None,
        act_chain: Vec::new(),
        context_id: Some(ctx.context_id.as_str().to_owned()),
    };
    match db.write_pool_arc() {
        Ok(pool) => {
            if let Err(e) = record_decision(&pool, &decision_audit).await {
                tracing::error!(
                    target: "governance.audit.write_failed",
                    error = %e,
                    ai_request_id = %ctx.ai_request_id,
                    "gateway governance audit write failed; row dropped"
                );
            }
        },
        Err(e) => tracing::error!(
            target: "governance.audit.write_failed",
            error = %e,
            ai_request_id = %ctx.ai_request_id,
            "no write pool for the gateway governance decision; row dropped"
        ),
    }
}

struct PromptEvaluation {
    evaluation: Evaluation,
    call_id: CallId,
    session_id: SessionId,
}

fn evaluate_prompt(ctx: &GatewayRequestContext, request: &CanonicalRequest) -> PromptEvaluation {
    // Why: `flatten_text` includes the forwarded surface attached just above,
    // so the chain scans exactly the bytes that will go on the wire — operator
    // `extra_patterns` included, which the hardcoded safety scanner cannot do.
    let input = GovernedInput::prompt(request.flatten_text());
    // Why: the bucket key is `session_id:user_id`. A sessionless inference call
    // needs a *stable* placeholder — minting one per request would give every
    // call its own bucket and silently disable rate limiting.
    let session_id = ctx.session_id.clone().unwrap_or_else(SessionId::system);
    // Why: the engine's idempotency contract is per-call_id, and the ai-request
    // id is the one identifier stable across re-evaluations of this call. It is
    // what stops the rate limiter charging twice.
    let call_id = CallId::new(ctx.ai_request_id.as_str());

    // Why: the same engine instance the MCP governance webhook uses — the rate
    // limiter's buckets are instance-scoped, so a second engine would give
    // inference its own budget and silently double every operator limit.
    let evaluation = GovernanceEngine::global().evaluate(&PolicyContext {
        target: GovernedTarget::Prompt,
        agent_scope: AgentScope::User {
            user_id: ctx.user_id.clone(),
        },
        // Why: the gateway context carries no permission tier. Both
        // scope-shaped policies are inert on a Prompt target, so this is not a
        // live gap today — but it would become one if a future change governed
        // `tool_use` blocks inside a request body.
        access_scope: AccessScope::Unknown,
        session_id: &session_id,
        user_id: &ctx.user_id,
        input: &input,
        call_id: &call_id,
    });

    PromptEvaluation {
        evaluation,
        call_id,
        session_id,
    }
}

async fn enforce_governance(
    db: &DbPool,
    ctx: &GatewayRequestContext,
    request: &CanonicalRequest,
    audit: &GatewayAudit,
) -> Result<(), DispatchError> {
    let PromptEvaluation {
        evaluation,
        call_id,
        session_id,
    } = evaluate_prompt(ctx, request);

    let denied = match &evaluation.decision {
        Decision::Allow { .. } => None,
        Decision::Deny { reason } => Some(reason.to_string()),
    };
    let policy = evaluation
        .chain
        .iter()
        .find(|e| e.result == ChainEntryResult::Fail)
        .map_or_else(
            || "default_allow".to_owned(),
            |e| e.policy_id.as_str().to_owned(),
        );

    record_governance_decision(db, ctx, evaluation, call_id, session_id).await;

    let Some(reason) = denied else {
        return Ok(());
    };
    tracing::warn!(
        ai_request_id = %ctx.ai_request_id,
        user_id = %ctx.user_id,
        policy = %policy,
        reason = %reason,
        "Gateway request denied by governance policy"
    );
    if let Err(e) = audit.fail(&reason).await {
        tracing::warn!(error = %e, "governance-deny audit fail failed");
    }
    Err(DispatchError::Recorded(
        GovernanceDenied {
            policy,
            message: reason,
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
    let Some(finding) = findings.iter().find(|f| {
        safety.block_categories.contains(&f.category) && blocks_at_phase(f.phase, safety.history)
    }) else {
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

/// Whether a finding raised at `phase` may deny the request.
///
/// A blocked category found in an earlier turn would otherwise deny every
/// remaining turn of the conversation, including the turns that carry nothing
/// objectionable — and a tool call the policy layer already refused is replayed
/// into the scan surface for the rest of the session.
pub fn blocks_at_phase(phase: &str, history: SafetyHistoryMode) -> bool {
    match phase {
        PHASE_REQUEST => true,
        PHASE_REQUEST_HISTORY => history == SafetyHistoryMode::Block,
        _ => false,
    }
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
