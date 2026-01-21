use anyhow::Result;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_traits::{ToolContext, ToolProviderError};

use crate::services::deployment::DeploymentService;

pub fn create_request_context(ctx: &ToolContext) -> Result<RequestContext, ToolProviderError> {
    let session_id = ctx
        .session_id
        .as_ref()
        .map_or_else(SessionId::system, |s| SessionId::new(s.clone()));

    let trace_id = ctx
        .trace_id
        .as_ref()
        .map_or_else(TraceId::generate, |t| TraceId::new(t.clone()));

    let context_id = ctx
        .headers
        .get("x-context-id")
        .filter(|s| !s.is_empty())
        .map(|s| ContextId::new(s.clone()))
        .ok_or_else(|| {
            ToolProviderError::ConfigurationError(
                "Missing x-context-id header - context must be propagated from parent request"
                    .into(),
            )
        })?;

    let agent_name = ctx
        .headers
        .get("x-agent-name")
        .filter(|s| !s.is_empty())
        .map(|s| AgentName::new(s.clone()))
        .ok_or_else(|| {
            ToolProviderError::ConfigurationError(
                "Missing x-agent-name header - agent context must be propagated from parent \
                 request"
                    .into(),
            )
        })?;

    let mut request_ctx = RequestContext::new(session_id, trace_id, context_id, agent_name)
        .with_auth_token(ctx.auth_token.clone());

    if let Some(user_id) = ctx.headers.get("x-user-id").filter(|s| !s.is_empty()) {
        request_ctx = request_ctx.with_user_id(UserId::new(user_id.clone()));
    }

    if let Some(task_id) = ctx.headers.get("x-task-id").filter(|s| !s.is_empty()) {
        request_ctx = request_ctx.with_task_id(TaskId::new(task_id.clone()));
    }

    if let Some(ai_tool_call_id) = &ctx.ai_tool_call_id {
        request_ctx = request_ctx.with_ai_tool_call_id(ai_tool_call_id.clone().into());
    }

    Ok(request_ctx)
}

pub fn load_agent_servers(agent_name: &str) -> Result<Vec<String>> {
    let config = DeploymentService::load_config()?;
    let agent_name_type = AgentName::new(agent_name);

    let agent = config
        .agents
        .get(agent_name_type.as_str())
        .ok_or_else(|| anyhow::anyhow!("Agent '{agent_name}' not found in services.yaml"))?;

    Ok(agent.metadata.mcp_servers.clone())
}
