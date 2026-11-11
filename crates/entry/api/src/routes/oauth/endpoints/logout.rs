//! `POST /oauth/logout` — terminates a bearer JWT before its natural `exp`.
//!
//! Writes the token's `jti` to `oauth_jti_revocations`. The JTI tower layer
//! consults this table on every authenticated request, so once logout returns
//! the same bearer is rejected on the next call. Cookie is cleared in the
//! response so browser flows do not silently re-present the dead token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Extension;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{AccessTokenId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_oauth::repository::OAuthRepository;
use tracing::instrument;

use crate::routes::oauth::extractors::OAuthRepo;
use crate::routes::oauth::{OAuthHttpError, internal};
use crate::services::session_cookie::SessionCookie;
use std::sync::Arc;
use systemprompt_manifest::Config;

#[instrument(skip(repo, req_ctx, config))]
pub async fn handle_logout(
    Extension(req_ctx): Extension<RequestContext>,
    Extension(config): Extension<Arc<Config>>,
    OAuthRepo(repo): OAuthRepo,
) -> Result<Response, OAuthHttpError> {
    let jti = req_ctx
        .jti()
        .ok_or_else(|| OAuthHttpError::invalid_request("Missing bearer token"))?;

    let exp_dt = req_ctx
        .token_exp()
        .and_then(|exp_unix| DateTime::<Utc>::from_timestamp(exp_unix, 0))
        .ok_or_else(|| OAuthHttpError::invalid_request("Invalid token expiry"))?;

    revoke_jti(&repo, jti, req_ctx.user_id(), exp_dt).await?;

    let mut response = (StatusCode::NO_CONTENT).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        SessionCookie::from_config(&config).cleared(),
    );
    Ok(response)
}

async fn revoke_jti(
    repo: &OAuthRepository,
    jti: &AccessTokenId,
    user_id: &UserId,
    exp: DateTime<Utc>,
) -> Result<(), OAuthHttpError> {
    repo.revoke_jti(jti, user_id, exp)
        .await
        .map_err(|e| internal::server_error("Logout failed", e))
}
