//! Governance evaluation for gateway prompts and the decision-audit write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use systemprompt_database::DbPool;
use systemprompt_identifiers::{CallId, PolicyId, SessionId};
use systemprompt_security::authz::types::{Decision, DenyReason};
use systemprompt_security::policy::{
    AgentScope, AuditOrigin, AuditTarget, ChainEntryOutcome, ChainEntryResult, DecisionAudit,
    Evaluation, GovernanceEngine, GovernedInput, GovernedTarget, PolicyContext, PrincipalSnapshot,
    record_decision,
};

// Why: the quota windows are not a chain policy, so nothing in the chain would
// name them; the label is fixed here so the warn report can group on it.
pub(in crate::services::gateway::service) const QUOTA_POLICY_LABEL: &str = "quota";

use super::super::super::audit::GatewayRequestContext;
use super::super::super::protocol::canonical::CanonicalRequest;

// Why: written on allow as well as on deny — the chain trace is the product,
// not just the refusals.
pub(super) async fn record_governance_decision(
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
            client_id: ctx.client_id.clone(),
            claimed: None,
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

// Why: a quota breach under `quota_mode: warn` is a governance warning like
// any other and belongs in the same table the report reads, with the same
// shape a chain warning has — one `Warn` chain entry naming the policy — so a
// quota row and a secret_scan row are directly comparable.
pub(in crate::services::gateway::service) async fn record_quota_warning(
    db: &DbPool,
    ctx: &GatewayRequestContext,
    message: &str,
) {
    let session_id = ctx.session_id.clone().unwrap_or_else(SessionId::system);
    let call_id = CallId::new(ctx.ai_request_id.as_str());
    let reason = DenyReason::PolicyViolation {
        policy: QUOTA_POLICY_LABEL.to_owned(),
        detail: Cow::Owned(message.to_owned()),
    };
    let evaluation = Evaluation {
        decision: Decision::Warn { reason },
        chain: vec![ChainEntryOutcome {
            policy_id: PolicyId::new(QUOTA_POLICY_LABEL),
            result: ChainEntryResult::Warn,
            detail: message.to_owned(),
            duration_ms: 0.0,
        }],
    };
    record_governance_decision(db, ctx, evaluation, call_id, session_id).await;
}

pub(super) struct PromptEvaluation {
    pub(super) evaluation: Evaluation,
    pub(super) call_id: CallId,
    pub(super) session_id: SessionId,
}

pub(super) fn evaluate_prompt(
    ctx: &GatewayRequestContext,
    request: &CanonicalRequest,
) -> PromptEvaluation {
    // Why: `flatten_parts` includes the forwarded surface `PreparedDispatch`
    // attached, so the chain scans exactly the bytes that will go on the wire
    // — operator `extra_patterns` included, which the hardcoded safety scanner
    // cannot do — and each part keeps its source path, so a denial names the
    // leaf that actually matched instead of blaming `prompt.text`.
    let input = GovernedInput::prompt_parts(request.flatten_parts());
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
