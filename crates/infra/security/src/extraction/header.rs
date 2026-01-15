use axum::http::{HeaderMap, HeaderValue};
use systemprompt_identifiers::{headers, AgentName, ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::execution::context::RequestContext;

#[derive(Debug, Clone, Copy)]
pub struct HeaderExtractor;

impl HeaderExtractor {
    pub fn extract_trace_id(headers: &HeaderMap) -> TraceId {
        Self::extract_header(headers, headers::TRACE_ID)
            .map_or_else(TraceId::generate, TraceId::new)
    }

    pub fn extract_context_id(headers: &HeaderMap) -> ContextId {
        Self::extract_header(headers, headers::CONTEXT_ID)
            .filter(|s| !s.is_empty())
            .map_or_else(ContextId::empty, ContextId::new)
    }

    pub fn extract_task_id(headers: &HeaderMap) -> Option<TaskId> {
        Self::extract_header(headers, headers::TASK_ID).map(TaskId::new)
    }

    pub fn extract_agent_name(headers: &HeaderMap) -> AgentName {
        Self::extract_header(headers, headers::AGENT_NAME)
            .map_or_else(AgentName::system, AgentName::new)
    }

    pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
        Self::extract_header(headers, headers::AUTHORIZATION)
            .and_then(|s| s.strip_prefix("Bearer ").map(ToString::to_string))
    }

    fn extract_header(headers: &HeaderMap, name: &str) -> Option<String> {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(ToString::to_string)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HeaderInjector;

impl HeaderInjector {
    pub fn inject_session_id(headers: &mut HeaderMap, session_id: &SessionId) -> Result<(), ()> {
        Self::inject_header(headers, headers::SESSION_ID, session_id.as_str())
    }

    pub fn inject_user_id(headers: &mut HeaderMap, user_id: &UserId) -> Result<(), ()> {
        Self::inject_header(headers, headers::USER_ID, user_id.as_str())
    }

    pub fn inject_trace_id(headers: &mut HeaderMap, trace_id: &TraceId) -> Result<(), ()> {
        Self::inject_header(headers, headers::TRACE_ID, trace_id.as_str())
    }

    pub fn inject_context_id(headers: &mut HeaderMap, context_id: &ContextId) -> Result<(), ()> {
        if context_id.as_str().is_empty() {
            return Ok(());
        }
        Self::inject_header(headers, headers::CONTEXT_ID, context_id.as_str())
    }

    pub fn inject_task_id(headers: &mut HeaderMap, task_id: &TaskId) -> Result<(), ()> {
        Self::inject_header(headers, headers::TASK_ID, task_id.as_str())
    }

    pub fn inject_agent_name(headers: &mut HeaderMap, agent_name: &str) -> Result<(), ()> {
        Self::inject_header(headers, headers::AGENT_NAME, agent_name)
    }

    pub fn inject_from_request_context(
        headers: &mut HeaderMap,
        ctx: &RequestContext,
    ) -> Result<(), ()> {
        Self::inject_session_id(headers, &ctx.request.session_id)?;
        Self::inject_user_id(headers, &ctx.auth.user_id)?;
        Self::inject_trace_id(headers, &ctx.execution.trace_id)?;
        Self::inject_context_id(headers, &ctx.execution.context_id)?;
        Self::inject_agent_name(headers, ctx.execution.agent_name.as_str())?;
        Ok(())
    }

    fn inject_header(headers: &mut HeaderMap, name: &'static str, value: &str) -> Result<(), ()> {
        HeaderValue::from_str(value).map_or(Err(()), |header_value| {
            headers.insert(name, header_value);
            Ok(())
        })
    }
}
