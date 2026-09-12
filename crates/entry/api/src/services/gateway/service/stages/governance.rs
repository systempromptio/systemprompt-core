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
    Evaluation, GovernanceEngine, GovernanceEngineError, GovernedInput, GovernedTarget,
    PolicyContext, PrincipalSnapshot, record_decision,
};

pub(in crate::services::gateway::service) const QUOTA_POLICY_LABEL: &str = "quota";

use super::recovery::{PromptRecovery, govern_prompt};
use crate::services::gateway::audit::GatewayRequestContext;
use crate::services::gateway::protocol::canonical::CanonicalRequest;
use crate::services::gateway::protocol::outbound::PreparedBody;

pub(super) async fn record_governance_decision(
    db: &DbPool,
    ctx: &GatewayRequestContext,
    evaluation: Evaluation,
    call_id: CallId,
    session_id: SessionId,
) -> anyhow::Result<()> {
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
    let pool = db.write_pool_arc()?;
    record_decision(&pool, &decision_audit).await?;
    Ok(())
}

pub(in crate::services::gateway::service) async fn record_quota_warning(
    db: &DbPool,
    ctx: &GatewayRequestContext,
    message: &str,
) -> anyhow::Result<()> {
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
    record_governance_decision(db, ctx, evaluation, call_id, session_id).await
}

pub(super) struct PromptEvaluation {
    pub(super) evaluation: Evaluation,
    pub(super) call_id: CallId,
    pub(super) session_id: SessionId,
    pub(super) recovery_count: usize,
    pub(super) recovery_locations: Vec<String>,
}

pub(super) fn evaluate_prompt(
    ctx: &GatewayRequestContext,
    request: &mut CanonicalRequest,
    body: &mut PreparedBody,
) -> Result<PromptEvaluation, GovernanceEngineError> {
    let session_id = ctx.session_id.clone().unwrap_or_else(SessionId::system);
    let call_id = CallId::new(ctx.ai_request_id.as_str());
    let input = GovernedInput::prompt_parts([]);
    let policy_ctx = PolicyContext {
        target: GovernedTarget::Prompt,
        agent_scope: AgentScope::User {
            user_id: ctx.user_id.clone(),
        },
        access_scope: ctx.access_scope,
        session_id: &session_id,
        user_id: &ctx.user_id,
        input: &input,
        call_id: &call_id,
    };
    let PromptRecovery {
        evaluation,
        recovery_count,
        recovery_locations,
    } = govern_prompt(GovernanceEngine::global()?, &policy_ctx, request, body);

    Ok(PromptEvaluation {
        evaluation,
        call_id,
        session_id,
        recovery_count,
        recovery_locations,
    })
}
