//! JWT middleware construction parameters.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, JwtToken, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::auth::{TokenInteraction, UserType};
use systemprompt_models::execution::context::{RequestContext, UserInteractionMode};
use systemprompt_security::{ExtractionMethod, HeaderExtractor, JwtUserContext, TokenExtractor};

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
    pub browser_session: bool,
}

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
        browser_session,
    } = params;
    let mut ctx = RequestContext::new(
        session_id,
        trace_id,
        context_id,
        agent_name,
        Actor::user(user_id),
    )
    .with_user_type(user_type)
    .with_act_chain(jwt_context.act_chain)
    .with_token_exp(jwt_context.exp);

    if let Some(jti) = jwt_context.jti {
        ctx = ctx.with_jti(jti);
    }
    if let Some(client_id) = jwt_context.client_id {
        ctx = ctx.with_client_id(client_id);
    }
    if let Some(t_id) = task_id {
        ctx = ctx.with_task_id(t_id);
    }
    if let Some(token) = auth_token.filter(|token| !token.is_empty()) {
        ctx = ctx.with_auth_token(JwtToken::new(token));
    }
    // Why: the caller purpose comes only from the signed `purpose` claim an
    // in-process issuer stamped, never from a request header the bearer sent.
    if let Some(purpose) = jwt_context.purpose {
        ctx = ctx.with_caller_purpose(purpose);
    }
    if let Some(interaction) = jwt_context.interaction {
        ctx = ctx.with_interaction_mode(match interaction {
            TokenInteraction::Interactive => UserInteractionMode::Interactive,
            TokenInteraction::NonInteractive => UserInteractionMode::NonInteractive,
        });
    }
    if browser_session {
        ctx = ctx.with_browser_session();
    }
    ctx
}

/// Whether the token the chain found came from the browser session cookie
/// rather than a bearer header.
#[must_use]
pub fn is_browser_session(token_extractor: &TokenExtractor, headers: &HeaderMap) -> bool {
    matches!(
        token_extractor.extract_with_method(headers),
        Ok((_, ExtractionMethod::Cookie))
    )
}

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
