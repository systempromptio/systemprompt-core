//! `POST /oauth/logout` — terminates a bearer JWT before its natural `exp`.
//!
//! Writes the token's `jti` to `oauth_jti_revocations`. The JTI tower layer
//! consults this table on every authenticated request, so once logout returns
//! the same bearer is rejected on the next call. Cookie is cleared in the
//! response so browser flows do not silently re-present the dead token.

use axum::extract::Extension;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use systemprompt_models::RequestContext;
use systemprompt_oauth::repository::OAuthRepository;
use tracing::instrument;
use uuid::Uuid;

use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;

#[instrument(skip(repo, req_ctx))]
pub async fn handle_logout(
    Extension(req_ctx): Extension<RequestContext>,
    OAuthRepo(repo): OAuthRepo,
) -> Result<Response, OAuthHttpError> {
    let jti = req_ctx.jti().to_string();
    if jti.is_empty() {
        return Err(OAuthHttpError::invalid_request("Missing bearer token"));
    }

    let exp_unix = req_ctx.token_exp();
    let exp_dt = DateTime::<Utc>::from_timestamp(exp_unix, 0)
        .ok_or_else(|| OAuthHttpError::invalid_request("Invalid token expiry"))?;

    let user_uuid = Uuid::parse_str(req_ctx.user_id().as_str())
        .map_err(|_| OAuthHttpError::invalid_request("Invalid user id"))?;

    revoke_jti(&repo, &jti, user_uuid, exp_dt).await?;

    let mut response = (StatusCode::NO_CONTENT).into_response();
    if let Ok(cookie) =
        HeaderValue::from_str("access_token=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Strict")
    {
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    Ok(response)
}

async fn revoke_jti(
    repo: &OAuthRepository,
    jti: &str,
    user_id: Uuid,
    exp: DateTime<Utc>,
) -> Result<(), OAuthHttpError> {
    repo.revoke_jti(jti, user_id, exp).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to persist JTI revocation on logout");
        OAuthHttpError::server_error("Logout failed")
    })
}
