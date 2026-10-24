//! The session an API-key request is bound to.
//!
//! An `x-session-id` the gateway issued to this key's user is used as is. Any
//! other value — absent, unknown, revoked, or a harness's own id such as
//! `OpenCode`'s `ses_…` — is never rejected: the request lands on the session
//! bound to the key in `api_key_sessions` (minted on first use, re-minted if
//! it has gone), and the client's header is kept as the audit row's
//! `client_session_id` when the body names no client session of its own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, StatusCode};
use systemprompt_identifiers::{ApiKeyId, ClientSessionId, SessionId, SessionSource, UserId};
use systemprompt_oauth::services::SessionCreationService;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{AppContext as _, ExtractSignals};

use super::error::RejectionError;
use crate::services::middleware::session::{SessionAttestationError, attest_session};

const ROUTE: &str = "gateway/messages";
const GATEWAY_SESSION_PREFIX: &str = "sess_";

#[derive(Debug, Clone, Copy)]
pub struct SessionRequest<'a> {
    pub api_key_id: &'a ApiKeyId,
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
    ctx: &AppContext,
    request: SessionRequest<'_>,
) -> Result<SessionBinding, RejectionError> {
    if let Some(header) = request.header_session
        && is_usable(ctx, header, request.user_id).await?
    {
        return Ok(SessionBinding {
            session_id: header.clone(),
            client_session: None,
        });
    }
    let session_id = bound_session(ctx, request).await?;
    Ok(SessionBinding {
        session_id,
        client_session: request
            .header_session
            .and_then(|s| ClientSessionId::try_new(s.as_str().to_owned()).ok()),
    })
}

pub async fn is_client_owned(
    ctx: &AppContext,
    header: &SessionId,
    user_id: &UserId,
) -> Result<bool, RejectionError> {
    if header.as_str().starts_with(GATEWAY_SESSION_PREFIX) {
        return Ok(false);
    }
    let sessions = ctx.session_provider().ok_or_else(|| {
        RejectionError::server(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session provider unavailable: cannot attest session",
        )
    })?;
    match attest_session(&sessions, header, user_id, ROUTE).await {
        Err(SessionAttestationError::Missing) => Ok(true),
        Ok(()) | Err(SessionAttestationError::UserMismatch) => Ok(false),
        Err(lookup @ SessionAttestationError::Lookup(..)) => Err(RejectionError::server(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session attestation failed",
        )
        .with_cause(lookup)),
    }
}

async fn is_usable(
    ctx: &AppContext,
    session_id: &SessionId,
    user_id: &UserId,
) -> Result<bool, RejectionError> {
    let sessions = ctx.session_provider().ok_or_else(|| {
        RejectionError::server(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session provider unavailable: cannot attest session",
        )
    })?;
    match attest_session(&sessions, session_id, user_id, ROUTE).await {
        Ok(()) => Ok(true),
        Err(SessionAttestationError::Missing | SessionAttestationError::UserMismatch) => Ok(false),
        Err(lookup @ SessionAttestationError::Lookup(..)) => Err(RejectionError::server(
            StatusCode::INTERNAL_SERVER_ERROR,
            "session attestation failed",
        )
        .with_cause(lookup)),
    }
}

async fn bound_session(
    ctx: &AppContext,
    request: SessionRequest<'_>,
) -> Result<SessionId, RejectionError> {
    let repo = ctx.user_repository();
    let existing = repo
        .find_api_key_session(request.api_key_id)
        .await
        .map_err(|e| binding_error("api-key session lookup failed").with_cause(e))?;
    if let Some(session_id) = existing
        && is_usable(ctx, &session_id, request.user_id).await?
    {
        return Ok(session_id);
    }
    let session_id = mint(ctx, request).await?;
    repo.set_api_key_session(request.api_key_id, &session_id)
        .await
        .map_err(|e| binding_error("api-key session bind failed").with_cause(e))?;
    Ok(session_id)
}

async fn mint(ctx: &AppContext, request: SessionRequest<'_>) -> Result<SessionId, RejectionError> {
    let analytics = ctx
        .analytics_provider()
        .ok_or_else(|| binding_error("analytics provider unavailable"))?;
    let users = ctx
        .user_provider()
        .ok_or_else(|| binding_error("user provider unavailable"))?;
    let sessions = ctx
        .session_provider()
        .ok_or_else(|| binding_error("session provider unavailable"))?;
    let session_analytics = analytics.extract_analytics(request.headers, ExtractSignals::default());
    SessionCreationService::new(sessions, users, ctx.signing_keys().clone())
        .create_authenticated_session(request.user_id, &session_analytics, SessionSource::Api)
        .await
        .map_err(|e| binding_error("api-key session mint failed").with_cause(e))
}

fn binding_error(message: &'static str) -> RejectionError {
    RejectionError::server(StatusCode::INTERNAL_SERVER_ERROR, message)
}
