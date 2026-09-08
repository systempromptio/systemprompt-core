//! Policy-chain enforcement before an external MCP tool call leaves the
//! gateway.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::audit::parse_tool_call;
use super::super::backend::ProxyError;
use systemprompt_identifiers::{CallId, McpToolName};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use systemprompt_security::authz::Decision;
use systemprompt_security::policy::governed::McpToolInput;
use systemprompt_security::policy::types::AccessScope;
use systemprompt_security::policy::{
    AgentScope, AuditOrigin, AuditTarget, DecisionAudit, GovernanceEngine, GovernedInput,
    GovernedTarget, PolicyContext, PrincipalSnapshot, record_decision,
};

pub(super) async fn enforce(
    ctx: &AppContext,
    request: &RequestContext,
    service: &str,
    body: &[u8],
) -> Result<(), ProxyError> {
    let denied = || ProxyError::Forbidden {
        service: service.to_owned(),
    };
    let Some(value) = tool_call(service, body)? else {
        return Ok(());
    };
    let call_id = CallId::generate();
    let scope = request
        .user
        .as_ref()
        .map_or(AccessScope::User, |u| AccessScope::from_roles(&u.roles));
    let tool = value
        .pointer("/params/name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(denied)?;
    let target = format!("mcp__{service}__{tool}");
    let arguments = value
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let input = GovernedInput::tool_arguments(McpToolInput::new(arguments));
    let evaluation = GovernanceEngine::global()
        .map_err(|error| {
            tracing::warn!(%error, service, "External MCP governance failed");
            denied()
        })?
        .evaluate(&PolicyContext {
            target: GovernedTarget::Tool {
                tool: McpToolName::new(&target),
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
        call_id: call_id.to_string(),
        origin: AuditOrigin::Governed,
        decision: evaluation.decision,
        principal: principal(request, scope),
        target: AuditTarget {
            tool_name: target,
            plugin_id: None,
        },
        chain: evaluation.chain,
        approver: None,
        act_chain: request.auth.act_chain.clone(),
        context_id: Some(request.context_id().to_string()),
        trace_id: Some(request.trace_id().to_string()),
    };
    let pool = ctx.db_pool().write_pool_arc().map_err(|error| {
        tracing::warn!(%error, service, "External MCP governance failed");
        denied()
    })?;
    record_decision(&pool, &record).await.map_err(|error| {
        tracing::warn!(%error, service, "External MCP governance failed");
        denied()
    })?;
    if allowed { Ok(()) } else { Err(denied()) }
}

fn principal(request: &RequestContext, scope: AccessScope) -> PrincipalSnapshot {
    PrincipalSnapshot {
        user_id: request.user_id().clone(),
        session_id: request.session_id().clone(),
        agent_session: None,
        agent_id: None,
        agent_scope: scope,
        client_id: request.client_id().cloned(),
        claimed: None,
    }
}

fn tool_call(service: &str, body: &[u8]) -> Result<Option<serde_json::Value>, ProxyError> {
    let denied = || ProxyError::Forbidden {
        service: service.to_owned(),
    };
    if body.is_empty() {
        return Ok(None);
    }
    let value: serde_json::Value = serde_json::from_slice(body).map_err(|error| {
        tracing::debug!(%error, service, "Invalid external MCP request");
        denied()
    })?;
    if !value.is_object() {
        return Err(denied());
    }
    if parse_tool_call(body).is_some() {
        return Ok(Some(value));
    }
    if value.get("method").and_then(serde_json::Value::as_str) == Some("tools/call") {
        return Err(denied());
    }
    Ok(None)
}
