//! Policy-chain enforcement before an external MCP tool call leaves the
//! gateway.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::audit::{ToolCallFrame, classify_tool_call};
use super::super::backend::ProxyError;
use std::borrow::Cow;
use systemprompt_identifiers::{CallId, McpToolName, ServiceName};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use systemprompt_security::authz::{Decision, DenyReason};
use systemprompt_security::policy::governed::McpToolInput;
use systemprompt_security::policy::types::AccessScope;
use systemprompt_security::policy::{
    AgentScope, AuditOrigin, AuditTarget, DecisionAudit, GovernedInput, GovernedTarget,
    PolicyContext, PrincipalSnapshot, record_decision,
};

pub(super) async fn enforce(
    ctx: &AppContext,
    request: &RequestContext,
    service: &ServiceName,
    body: &[u8],
) -> Result<(), ProxyError> {
    let denied = || ProxyError::Forbidden {
        service: service.to_string(),
    };
    let value = match tool_call(service, body)? {
        GovernedCall::None => return Ok(()),
        GovernedCall::Valid(value) => value,
        GovernedCall::InvalidName { raw_name } => {
            record_invalid_name(ctx, request, service, raw_name.as_deref()).await;
            return Err(denied());
        },
    };
    let call_id = CallId::generate();
    let scope = access_scope(request);
    let tool = value
        .pointer("/params/name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(denied)?;
    let target = McpToolName::try_new(format!("mcp__{service}__{tool}")).map_err(|error| {
        tracing::warn!(%error, %service, "External MCP tool name rejected");
        denied()
    })?;
    let arguments = value
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let input = GovernedInput::tool_arguments(McpToolInput::new(arguments));
    let evaluation = ctx.governance().evaluate(&PolicyContext {
        target: GovernedTarget::Tool {
            tool: target.clone(),
        },
        agent_scope: AgentScope::User {
            user_id: request.user_id().clone(),
        },
        access_scope: scope,
        session_id: request.session_id(),
        user_id: request.user_id(),
        input: &input,
        call_id: &call_id,
    });
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

enum GovernedCall {
    None,
    // JSON: MCP JSON-RPC request frame — forwarded verbatim to the external MCP
    // server.
    Valid(serde_json::Value),
    InvalidName { raw_name: Option<String> },
}

fn tool_call(service: &ServiceName, body: &[u8]) -> Result<GovernedCall, ProxyError> {
    let denied = || ProxyError::Forbidden {
        service: service.to_string(),
    };
    if body.is_empty() {
        return Ok(GovernedCall::None);
    }
    let value: serde_json::Value = serde_json::from_slice(body).map_err(|error| {
        tracing::debug!(%error, %service, "Invalid external MCP request");
        denied()
    })?;
    if !value.is_object() {
        return Err(denied());
    }
    match classify_tool_call(body) {
        ToolCallFrame::Call(_) => Ok(GovernedCall::Valid(value)),
        ToolCallFrame::InvalidName { raw_name } => Ok(GovernedCall::InvalidName { raw_name }),
        ToolCallFrame::NotToolCall
            if value.get("method").and_then(serde_json::Value::as_str) == Some("tools/call") =>
        {
            Ok(GovernedCall::InvalidName { raw_name: None })
        },
        ToolCallFrame::NotToolCall => Ok(GovernedCall::None),
    }
}
