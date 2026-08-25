//! Staged dispatch pipeline: `PreparedDispatch` → `GovernedDispatch` →
//! `ScannedDispatch` → upstream send.
//!
//! Each stage owns the request by value and is only constructible from the
//! previous one, so the ordering the gateway's audit trail depends on —
//! build the exact wire payload, then govern it, then scan it, then send it —
//! is enforced by the types rather than by call-site discipline. Governance
//! ahead of the scanner plane also keeps first-deny-wins across both: a
//! denied request produces exactly one audit row and one 403.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use systemprompt_ai::SafetyConfig;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, CallId, SessionId};
use systemprompt_models::profile::GatewayConfig;
use systemprompt_models::services::ai::ModelLimits;
use systemprompt_models::wire::inspect;
use systemprompt_security::authz::types::Decision;
use systemprompt_security::policy::{
    AgentScope, AuditOrigin, AuditTarget, ChainEntryResult, DecisionAudit, Evaluation,
    GovernanceEngine, GovernedInput, GovernedTarget, PolicyContext, PrincipalSnapshot,
    record_decision,
};

use super::super::audit::{GatewayAudit, GatewayRequestContext};
use super::super::protocol::canonical::CanonicalRequest;
use super::super::protocol::inbound::InboundAdapter;
use super::super::protocol::outbound::{OutboundCtx, OutboundOutcome, PreparedBody};
use super::finalize::{apply_system_prompt_override, run_request_safety_scan};
use super::resolve::ResolvedUpstream;
use super::{DispatchError, GovernanceDenied, SafetyBlocked, blocks_at_phase};

pub(super) struct UpstreamRelay<'a> {
    pub raw_body: &'a Bytes,
    pub inbound: &'a dyn InboundAdapter,
}

pub(super) struct PreparedDispatch {
    request: CanonicalRequest,
    upstream_model: String,
    model_limits: Option<ModelLimits>,
    body: PreparedBody,
}

pub(super) struct GovernedDispatch(PreparedDispatch);

pub(super) struct ScannedDispatch(PreparedDispatch);

impl PreparedDispatch {
    pub(super) async fn build(
        config: &GatewayConfig,
        upstream: &ResolvedUpstream<'_>,
        mut request: CanonicalRequest,
        audit: &GatewayAudit,
        relay: UpstreamRelay<'_>,
    ) -> Result<Self, DispatchError> {
        let upstream_model = upstream
            .route
            .effective_upstream_model(&request.model)
            .to_owned();
        let override_descriptor = apply_system_prompt_override(
            config,
            &upstream.provider.name,
            &upstream_model,
            &mut request,
        )
        .await;
        if let Some(descriptor) = &override_descriptor {
            audit.set_system_prompt_override(descriptor).await;
        }
        let model_limits = upstream
            .provider
            .find_model(&upstream_model)
            .map(|m| m.limits);
        let raw_body = match &override_descriptor {
            Some(_) => None,
            None => (relay.inbound.passthrough_wire() == Some(upstream.provider.wire))
                .then_some(relay.raw_body),
        };
        strip_caller_identity(&mut request);

        let ctx = outbound_ctx(
            upstream,
            &request,
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
        audit.set_prepared_body_digest(&body.bytes).await;

        let surface = inspect::string_leaves(&body.bytes, inspect::SurfaceBudget::default());
        if surface.truncated() {
            tracing::warn!(
                ai_request_id = %audit.ctx.ai_request_id,
                leaves = surface.len(),
                "Gateway inspection surface truncated — part of the forwarded body was not scanned"
            );
        }
        request.forwarded_surface = surface;

        Ok(Self {
            request,
            upstream_model,
            model_limits,
            body,
        })
    }
}

impl GovernedDispatch {
    pub(super) async fn enforce(
        prepared: PreparedDispatch,
        db: &DbPool,
        ctx: &GatewayRequestContext,
        audit: &GatewayAudit,
    ) -> Result<Self, DispatchError> {
        let PromptEvaluation {
            evaluation,
            call_id,
            session_id,
        } = evaluate_prompt(ctx, &prepared.request);

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
            return Ok(Self(prepared));
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
}

impl ScannedDispatch {
    pub(super) async fn enforce(
        governed: GovernedDispatch,
        repos: &super::super::GatewayRepositories,
        ai_request_id: &AiRequestId,
        safety: &SafetyConfig,
        audit: &GatewayAudit,
    ) -> Result<Self, DispatchError> {
        let GovernedDispatch(prepared) = governed;
        let findings = run_request_safety_scan(
            &repos.safety_findings,
            ai_request_id,
            &prepared.request,
            safety,
        )
        .await;
        let Some(finding) = findings.iter().find(|f| {
            safety.block_categories.contains(&f.category)
                && blocks_at_phase(f.phase, safety.history)
        }) else {
            return Ok(Self(prepared));
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

    pub(super) fn request_model(&self) -> &str {
        &self.0.request.model
    }

    pub(super) async fn send(
        &self,
        upstream: &ResolvedUpstream<'_>,
        forward_headers: &[(String, String)],
        audit: &GatewayAudit,
    ) -> Result<OutboundOutcome, DispatchError> {
        let prepared = &self.0;
        let ctx = outbound_ctx(
            upstream,
            &prepared.request,
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
                audit_upstream_failure(
                    audit,
                    upstream.provider.name.as_str(),
                    &prepared.request.model,
                    &e,
                )
                .await;
                Err(DispatchError::Recorded(e))
            },
        }
    }
}

#[derive(Clone, Copy)]
struct CtxParts<'a> {
    upstream_model: &'a str,
    model_limits: Option<ModelLimits>,
    forward_headers: &'a [(String, String)],
    raw_body: Option<&'a Bytes>,
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

// Why: `metadata.user_id` is an end-user identifier meant for the provider the
// caller chose, so it must not reach a different wire's upstream. Stripped
// unconditionally on the canonical form because an adapter may decline the raw
// lane and fall back to the canonical build; the passthrough lane applies the
// same rule to the raw body in `normalize_raw_body`.
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
            agent_scope: ctx.access_scope,
        },
        target: AuditTarget {
            tool_name: GovernedTarget::Prompt.as_str().to_owned(),
            plugin_id: None,
        },
        chain: evaluation.chain,
        approver: None,
        act_chain: Vec::new(),
        context_id: Some(ctx.context_id.as_str().to_owned()),
        trace_id: ctx.trace_id.as_ref().map(|t| t.as_str().to_owned()),
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
    // Why: `flatten_text` includes the forwarded surface `PreparedDispatch`
    // attached, so the chain scans exactly the bytes that will go on the wire
    // — operator `extra_patterns` included, which the hardcoded safety scanner
    // cannot do.
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
        access_scope: ctx.access_scope,
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
