//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_security::{HeaderExtractor, JwtUserContext, TokenExtractor};

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
#[derive(Debug)]
pub struct BuildContextParams {
    pub jwt_context: JwtUserContext,
    pub session_id: SessionId,
    pub user_id: UserId,
    pub trace_id: TraceId,
    pub context_id: ContextId,
    pub agent_name: AgentName,
    pub task_id: Option<TaskId>,
    pub auth_token: Option<String>,
    pub user_type: UserType,
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn build_context(params: BuildContextParams) -> RequestContext {
    let BuildContextParams {
        jwt_context,
        session_id,
        user_id,
        trace_id,
        context_id,
        agent_name,
        task_id,
        auth_token,
        user_type,
    } = params;
    let mut ctx = RequestContext::new(session_id, trace_id, context_id, agent_name)
        .with_actor(systemprompt_identifiers::Actor::user(user_id))
        .with_user_type(user_type)
        .with_act_chain(jwt_context.act_chain)
        .with_jti(jwt_context.jti)
        .with_token_exp(jwt_context.exp);

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

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn extract_common_headers(
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
