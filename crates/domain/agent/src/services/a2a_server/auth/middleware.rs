use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;

use super::types::AgentOAuthState;
use crate::services::a2a_server::handlers::AgentHandlerState;

pub async fn agent_oauth_middleware(
    State(state): State<AgentOAuthState>,
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let headers = request.headers();
    let has_auth_header = headers.get("authorization").is_some();

    let context = state
        .auth_service
        .validate_request(headers, state.auth_mode())
        .map_err(|e| {
            tracing::warn!(
                has_auth_header = has_auth_header,
                auth_mode = ?state.auth_mode(),
                error = %e,
                "Agent auth validation failed"
            );
            StatusCode::UNAUTHORIZED
        })?;

    let has_auth_token = !context.auth_token().as_str().is_empty();
    tracing::debug!(
        has_auth_header = has_auth_header,
        has_auth_token = has_auth_token,
        user_id = %context.user_id(),
        "Agent request authenticated"
    );

    request.extensions_mut().insert(context);

    Ok(next.run(request).await)
}

pub fn get_user_context(
    request: &Request<axum::body::Body>,
) -> Option<&super::types::AgentAuthenticatedUser> {
    request
        .extensions()
        .get::<super::types::AgentAuthenticatedUser>()
}

pub async fn agent_oauth_middleware_wrapper(
    State(handler_state): State<Arc<AgentHandlerState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    agent_oauth_middleware(
        State(handler_state.oauth_state.as_ref().clone()),
        request,
        next,
    )
    .await
}
