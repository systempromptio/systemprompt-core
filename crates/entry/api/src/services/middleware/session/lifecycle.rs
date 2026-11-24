//! Session-row creation and touch on request, through the oauth member's
//! sealed session issuer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ClientId, SessionId, SessionSource, UserId};
use systemprompt_models::api::ApiError;
use systemprompt_traits::{DynSessionIssuer, SessionIssueError};

use super::RequestMeta;
use crate::error::ApiHttpError;

pub(super) async fn create_new_session(
    issuer: &DynSessionIssuer,
    meta: &RequestMeta<'_>,
) -> Result<(SessionId, UserId, String, bool, String), ApiHttpError> {
    let client_id = ClientId::new("sp_web".to_owned());

    issuer
        .issue_anonymous(meta.analytics, &client_id, SessionSource::Web)
        .await
        .map(|session_info| {
            (
                session_info.session_id,
                session_info.user_id,
                session_info.jwt_token.as_str().to_owned(),
                session_info.is_new,
                session_info.fingerprint_hash,
            )
        })
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to create anonymous session");
            ApiHttpError::internal("Service temporarily unavailable", e)
        })
}

pub(super) async fn refresh_session_for_user(
    issuer: &DynSessionIssuer,
    user_id: &UserId,
    meta: &RequestMeta<'_>,
) -> Result<(SessionId, UserId, String, bool, String), ApiHttpError> {
    let session_id = issuer
        .issue_for_user(user_id, meta.analytics, SessionSource::Web)
        .await
        .map_err(|e| match e {
            SessionIssueError::UserNotFound { ref user_id } => ApiHttpError::from(
                ApiError::not_found(format!("User not found: {}", user_id))
                    .with_error_key("user_not_found"),
            ),
            other => {
                tracing::error!(error = %other, user_id = %user_id, "Failed to create session for user");
                ApiHttpError::internal("Failed to refresh session", other)
            },
        })?;

    let token = issuer
        .mint_anonymous_jwt(user_id, &session_id, &ClientId::new("sp_web".to_owned()))
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to generate JWT during session refresh");
            ApiHttpError::internal("Failed to refresh session", e)
        })?;

    Ok((
        session_id,
        user_id.clone(),
        token.as_str().to_owned(),
        true,
        meta.analytics.compute_fingerprint(),
    ))
}
