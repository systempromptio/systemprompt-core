//! A2A server authentication middleware.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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
    let headers = request.headers().clone();
    let has_auth_header = headers.get("authorization").is_some();
    if !has_auth_header {
        tracing::warn!("Agent request refused: no Authorization header");
        return Err(StatusCode::UNAUTHORIZED);
    }
    let Some(authenticator) = state.authenticator.as_ref() else {
        tracing::warn!("Agent request refused: no request authenticator is composed");
        return Err(StatusCode::UNAUTHORIZED);
    };

    let context = authenticator
        .authenticate(&headers, &state.audiences)
        .await
        .map_err(|e| {
            tracing::warn!(
                has_auth_header = has_auth_header,
                error = %e,
                "Agent auth validation failed"
            );
            StatusCode::UNAUTHORIZED
        })?;

    let has_auth_token = context.auth_token().is_some();
    tracing::debug!(
        has_auth_header = has_auth_header,
        has_auth_token = has_auth_token,
        user_id = %context.user_id(),
        "Agent request authenticated"
    );

    request.extensions_mut().insert(context);

    Ok(next.run(request).await)
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
