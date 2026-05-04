use axum::http::HeaderMap;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_security::{HeaderExtractor, TokenExtractor};

use super::token::JwtUserContext;

pub(super) struct BuildContextParams {
    pub jwt_context: JwtUserContext,
    pub session_id: SessionId,
    pub user_id: UserId,
    pub trace_id: TraceId,
    pub context_id: ContextId,
    pub agent_name: AgentName,
    pub task_id: Option<TaskId>,
    pub auth_token: Option<String>,
}

pub(super) fn build_context(params: BuildContextParams) -> RequestContext {
    let BuildContextParams {
        jwt_context,
        session_id,
        user_id,
        trace_id,
        context_id,
        agent_name,
        task_id,
        auth_token,
    } = params;
    let mut ctx = RequestContext::new(session_id, trace_id, context_id, agent_name)
        .with_user_id(user_id)
        .with_user_type(jwt_context.user_type);

    if let Some(client_id) = jwt_context.client_id {
        ctx = ctx.with_client_id(client_id);
    }
    if let Some(t_id) = task_id {
        ctx = ctx.with_task_id(t_id);
    }
    if let Some(token) = auth_token {
        ctx = ctx.with_auth_token(token);
    }
    ctx
}

pub(super) fn extract_common_headers(
    token_extractor: &TokenExtractor,
    headers: &HeaderMap,
) -> (TraceId, Option<TaskId>, Option<String>, AgentName) {
    (
        HeaderExtractor::extract_trace_id(headers),
        HeaderExtractor::extract_task_id(headers),
        token_extractor.extract(headers).ok(),
        HeaderExtractor::extract_agent_name(headers),
    )
}
