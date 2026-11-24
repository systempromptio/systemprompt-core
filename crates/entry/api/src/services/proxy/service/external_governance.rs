//! Policy-chain enforcement before an external MCP tool call leaves the
//! gateway.
//!
//! [`KernelCallGovernor`] is the [`ProxiedCallGovernor`] the engine hands an
//! external upstream: the upstream classifies the JSON-RPC frame, the kernel
//! evaluates the named tool against the governance engine and records the
//! decision; a call naming no valid tool is recorded as a policy violation
//! and refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use std::borrow::Cow;
use systemprompt_extension::router::{GovernedToolCall, ProxiedCallGovernor, ProxyBackendError};
use systemprompt_governance::authz::{Decision, DenyReason};
use systemprompt_governance::policy::{
    AccessScope, AgentScope, AuditOrigin, AuditTarget, DecisionAudit, Evaluation, GovernedInput,
    GovernedTarget, McpToolInput, PolicyRequest, PrincipalSnapshot, record_decision,
};
use systemprompt_identifiers::{CallId, McpToolName, ServiceName};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

pub(super) struct KernelCallGovernor<'a> {
    ctx: &'a AppContext,
}

impl<'a> KernelCallGovernor<'a> {
    pub(super) const fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl ProxiedCallGovernor for KernelCallGovernor<'_> {
    async fn enforce(
        &self,
        caller: &RequestContext,
        service: &ServiceName,
        call: GovernedToolCall<'_>,
    ) -> Result<(), ProxyBackendError> {
        match call {
            GovernedToolCall::Valid { tool, arguments } => {
                enforce(self.ctx, caller, service, tool, arguments).await
            },
            GovernedToolCall::InvalidName { raw_name } => {
                record_invalid_name(self.ctx, caller, service, raw_name).await;
                Err(ProxyBackendError::Forbidden)
            },
        }
    }
}

async fn enforce(
    ctx: &AppContext,
    request: &RequestContext,
    service: &ServiceName,
    tool: &str,
    arguments: serde_json::Value,
) -> Result<(), ProxyBackendError> {
    let denied = || ProxyBackendError::Forbidden;
    let call_id = CallId::generate();
    let scope = access_scope(request);
    let target = McpToolName::try_new(format!("mcp__{service}__{tool}")).map_err(|error| {
        tracing::warn!(%error, %service, "External MCP tool name rejected");
        denied()
    })?;
    let input = GovernedInput::tool_arguments(McpToolInput::new(arguments));
    let evaluation = evaluate_tool(ctx, request, &target, &input, &call_id).await;
    let allowed = matches!(
        evaluation.decision,
        Decision::Allow { .. } | Decision::Warn { .. }
    );
    let record = DecisionAudit {
        id: call_id.to_string(),
        call_id: call_id.clone(),
        origin: AuditOrigin::Governed,
        decision: evaluation.decision,
        principal: principal(request, scope),
        target: AuditTarget {
            tool_name: target,
            plugin_id: None,
            tool_use_id: None,
        },
        chain: evaluation.chain,
        approver: None,
        act_chain: request.auth.act_chain.clone(),
        context_id: Some(request.context_id().clone()),
        trace_id: Some(request.trace_id().clone()),
    };
    let pool = ctx.db_pool().write_pool();
    record_decision(&pool, &record).await.map_err(|error| {
        tracing::warn!(%error, %service, "External MCP governance failed");
        denied()
    })?;
    if allowed { Ok(()) } else { Err(denied()) }
}

async fn evaluate_tool(
    ctx: &AppContext,
    request: &RequestContext,
    target: &McpToolName,
    input: &GovernedInput,
    call_id: &CallId,
) -> Evaluation {
    let roles = request
        .user
        .as_ref()
        .map(|user| user.roles.clone())
        .unwrap_or_default();
    ctx.governance()
        .charge_and_evaluate(PolicyRequest {
            target: GovernedTarget::Tool {
                tool: target.clone(),
            },
            agent_scope: AgentScope::User {
                user_id: request.user_id().clone(),
            },
            access_scope: access_scope(request),
            roles: &roles,
            session_id: request.session_id(),
            user_id: request.user_id(),
            input,
            call_id,
        })
        .await
}

fn access_scope(request: &RequestContext) -> AccessScope {
    request
        .user
        .as_ref()
        .map_or(AccessScope::User, |u| AccessScope::from_roles(&u.roles))
}

const RAW_NAME_AUDIT_LIMIT: usize = 128;

async fn record_invalid_name(
    ctx: &AppContext,
    request: &RequestContext,
    service: &ServiceName,
    raw_name: Option<&str>,
) {
    let shown: String = raw_name
        .unwrap_or_default()
        .chars()
        .take(RAW_NAME_AUDIT_LIMIT)
        .collect();
    let Ok(target) = McpToolName::try_new(format!("mcp__{service}__")) else {
        tracing::warn!(%service, "External MCP tools/call refused without an audit target");
        return;
    };
    let call_id = CallId::generate();
    let record = DecisionAudit {
        id: call_id.to_string(),
        call_id: call_id.clone(),
        origin: AuditOrigin::Governed,
        decision: Decision::Deny {
            reason: DenyReason::PolicyViolation {
                policy: INVALID_TOOL_NAME_POLICY.to_owned(),
                detail: Cow::Owned(format!(
                    "tools/call names no valid tool (received {shown:?})"
                )),
            },
        },
        principal: principal(request, access_scope(request)),
        target: AuditTarget {
            tool_name: target,
            plugin_id: None,
            tool_use_id: None,
        },
        chain: Vec::new(),
        approver: None,
        act_chain: request.auth.act_chain.clone(),
        context_id: Some(request.context_id().clone()),
        trace_id: Some(request.trace_id().clone()),
    };
    let pool = ctx.db_pool().write_pool();
    if let Err(error) = record_decision(&pool, &record).await {
        tracing::warn!(%error, %service, "Refused external MCP tools/call could not be audited");
    }
}

const INVALID_TOOL_NAME_POLICY: &str = "tool_name_validation";

fn principal(request: &RequestContext, scope: AccessScope) -> PrincipalSnapshot {
    PrincipalSnapshot {
        user_id: request.user_id().clone(),
        session_id: request.session_id().clone(),
        agent_session: None,
        agent_id: None,
        agent_scope: scope,
        client_id: request.client_id().cloned(),
    }
}
