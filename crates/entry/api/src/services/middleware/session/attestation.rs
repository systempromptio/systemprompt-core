//! Server-side attestation of a claimed session id.
//!
//! A session id is only evidence if the server issued it. [`attest_session`] is
//! the single predicate both credential paths use: the JWT middleware checks
//! the `session_id` claim with it, and the gateway checks the `x-session-id`
//! header presented alongside an API key with it. Keeping one implementation is
//! the point — two copies would drift, and the audit spine would then mean
//! different things depending on which credential wrote the row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::AnalyticsProvider;

#[derive(Debug, thiserror::Error)]
pub enum SessionAttestationError {
    #[error("Session missing or revoked")]
    Missing,
    #[error("Session user mismatch")]
    UserMismatch,
    #[error("Failed to check session: {0}")]
    Lookup(String),
}

pub async fn attest_session(
    analytics_provider: &Arc<dyn AnalyticsProvider>,
    session_id: &SessionId,
    user_id: &UserId,
    route_context: &str,
) -> Result<(), SessionAttestationError> {
    let session = analytics_provider
        .find_active_session_by_id(session_id)
        .await
        .map_err(|e| SessionAttestationError::Lookup(e.to_string()))?;

    let Some(session) = session else {
        tracing::info!(
            session_id = %session_id.as_str(),
            user_id = %user_id.as_str(),
            route = %route_context,
            "session attestation failed: session missing or revoked"
        );
        return Err(SessionAttestationError::Missing);
    };

    if let Some(session_user_id) = session.user_id.as_ref()
        && session_user_id.as_str() != user_id.as_str()
    {
        tracing::warn!(
            session_id = %session_id.as_str(),
            claimed_user_id = %user_id.as_str(),
            session_user_id = %session_user_id.as_str(),
            route = %route_context,
            "session attestation failed: session user mismatch"
        );
        return Err(SessionAttestationError::UserMismatch);
    }

    Ok(())
}
