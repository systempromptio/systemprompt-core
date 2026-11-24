//! The session an API-key request is bound to.
//!
//! An `x-session-id` the gateway issued to this key's user is used as is. Any
//! other value — absent, unknown, revoked, or a harness's own id such as
//! `OpenCode`'s `ses_…` — is never rejected: the request lands on the session
//! bound to the key in `api_key_sessions` (minted on first use, re-minted if
//! it has gone), and the client's header is kept as the audit row's
//! `client_session_id` when the body names no client session of its own.
//! The users member's sealed gateway session mint does all three, for the
//! verified owner of the presented key only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, StatusCode};
use systemprompt_identifiers::{ClientSessionId, SessionId, UserId};
use systemprompt_traits::ExtractSignals;

use crate::http::state::GatewayHttpState;

use super::error::RejectionError;

#[derive(Debug, Clone, Copy)]
pub struct SessionRequest<'a> {
    pub presented: &'a str,
    pub user_id: &'a UserId,
    pub header_session: Option<&'a SessionId>,
    pub headers: &'a HeaderMap,
}

#[derive(Debug, Clone)]
pub struct SessionBinding {
    pub session_id: SessionId,
    pub client_session: Option<ClientSessionId>,
}

pub async fn bind_api_key_session(
    state: &GatewayHttpState,
    request: SessionRequest<'_>,
) -> Result<SessionBinding, RejectionError> {
    let mint = &state.session_mint;
    let analytics = state
        .host
        .analytics
        .extract_analytics(request.headers, ExtractSignals::default());
    let bound = mint
        .bind_for_api_key(request.presented, request.header_session, &analytics)
        .await
        .map_err(|e| binding_error("api-key session bind failed").with_cause(e))?
        .ok_or_else(|| {
            RejectionError::client(StatusCode::UNAUTHORIZED, "Invalid or revoked API key")
        })?;
    if bound.user_id.as_str() != request.user_id.as_str() {
        return Err(binding_error("api-key session bound to another user"));
    }
    let client_session = if bound.presented_session_kept {
        None
    } else {
        request
            .header_session
            .and_then(|s| ClientSessionId::try_new(s.as_str().to_owned()).ok())
    };
    Ok(SessionBinding {
        session_id: bound.session_id,
        client_session,
    })
}

fn binding_error(message: &'static str) -> RejectionError {
    RejectionError::server(StatusCode::INTERNAL_SERVER_ERROR, message)
}
