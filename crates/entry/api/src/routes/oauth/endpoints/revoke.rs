//! RFC 7009 token revocation endpoint.
//!
//! An access token is acted on only after its signature, issuer, audience and
//! expiry verify, and a refresh token only after it resolves to a stored row.
//! Either way the token must belong to the caller (or the caller is an admin)
//! and, when the request authenticates a client, must have been issued to that
//! client. A token that fails any of these checks is left untouched and the
//! endpoint still answers 200, as RFC 7009 §2.2 requires for unknown tokens.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Form;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use systemprompt_identifiers::{ClientId, RefreshTokenId, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_models::{Config, RequestContext};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::validate_jwt_token;
use systemprompt_oauth::services::validation::{get_audit_user, validate_client_credentials};
use tracing::instrument;

use crate::routes::oauth::extractors::OAuthRepo;
use crate::routes::oauth::{OAuthHttpError, internal};

#[derive(Debug, Deserialize)]
pub struct RevokeRequest {
    pub token: String,
    pub token_type_hint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

struct Caller<'a> {
    user_id: &'a UserId,
    is_admin: bool,
    client_id: Option<&'a ClientId>,
}

impl Caller<'_> {
    fn owns(&self, owner: &UserId) -> bool {
        self.is_admin || owner == self.user_id
    }

    fn check_client(&self, issued_to: Option<&ClientId>) -> Result<(), OAuthHttpError> {
        if let (Some(caller), Some(issued)) = (self.client_id, issued_to)
            && caller != issued
        {
            return Err(OAuthHttpError::invalid_request(
                "Token was not issued to the authenticating client",
            ));
        }
        Ok(())
    }
}

#[instrument(skip(state, req_ctx, request, repo))]
pub async fn handle_revoke(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<OAuthState>,
    OAuthRepo(repo): OAuthRepo,
    Form(request): Form<RevokeRequest>,
) -> Result<Response, OAuthHttpError> {
    let audit_user = get_audit_user(Some(&req_ctx.auth.actor.user_id)).map_err(|e| {
        internal::rejected(
            OAuthHttpError::invalid_request("Authenticated user required"),
            e,
        )
    })?;

    let client_id = match &request.client_id {
        Some(raw) => {
            let client_id = ClientId::new(raw.clone());
            validate_client_credentials(&repo, &client_id, request.client_secret.as_deref())
                .await?;
            Some(client_id)
        },
        None => None,
    };

    let caller = Caller {
        user_id: req_ctx.user_id(),
        is_admin: req_ctx.user_type() == UserType::Admin,
        client_id: client_id.as_ref(),
    };

    match request.token_type_hint.as_deref() {
        Some("refresh_token") => {
            revoke_refresh_token(&repo, &request.token, &caller).await?;
        },
        Some("access_token") => {
            revoke_access_token(&state, &repo, &request.token, &caller).await?;
        },
        _ => {
            if !revoke_refresh_token(&repo, &request.token, &caller).await? {
                revoke_access_token(&state, &repo, &request.token, &caller).await?;
            }
        },
    }

    tracing::info!(
        token_hash = %hash_token(&request.token),
        token_type = %request.token_type_hint.as_deref().unwrap_or("not_specified"),
        client_id = ?request.client_id,
        revocation_reason = "user_request",
        revoked_by = %audit_user,
        "Token revocation processed"
    );

    Ok(StatusCode::OK.into_response())
}

async fn revoke_refresh_token(
    repo: &OAuthRepository,
    token: &str,
    caller: &Caller<'_>,
) -> Result<bool, OAuthHttpError> {
    let token_id = RefreshTokenId::new(token);
    let Some(holder) = repo.find_refresh_token_holder(&token_id).await? else {
        return Ok(false);
    };
    caller.check_client(Some(&holder.client_id))?;

    let owner = holder.user_id;
    if !caller.owns(&owner) {
        tracing::warn!(
            caller = %caller.user_id,
            "Refused to revoke a refresh token owned by another user"
        );
        return Ok(true);
    }

    repo.revoke_refresh_token(&token_id).await?;
    Ok(true)
}

async fn revoke_access_token(
    state: &OAuthState,
    repo: &OAuthRepository,
    token: &str,
    caller: &Caller<'_>,
) -> Result<(), OAuthHttpError> {
    let config = Config::get()?;
    let claims = match validate_jwt_token(token, &config.jwt_issuer, &config.jwt_audiences) {
        Ok(claims) => claims,
        Err(e) => {
            tracing::debug!(error = %e, "Access token did not verify; nothing to revoke");
            return Ok(());
        },
    };
    caller.check_client(claims.client_id.as_ref())?;

    if !UserId::try_new(claims.sub.as_str()).is_ok_and(|subject| caller.owns(&subject)) {
        tracing::warn!(
            caller = %caller.user_id,
            "Refused to revoke an access token owned by another user"
        );
        return Ok(());
    }

    record_jti_revocation(repo, &claims.jti, &claims.sub, claims.exp).await?;

    if let Some(session_id) = &claims.session_id {
        state
            .session_provider()
            .revoke_session(session_id)
            .await
            .map_err(|e| internal::server_error("Failed to revoke session", e))?;
    }
    Ok(())
}

async fn record_jti_revocation(
    repo: &OAuthRepository,
    jti: &str,
    sub: &str,
    exp: i64,
) -> Result<(), OAuthHttpError> {
    if jti.is_empty() {
        tracing::debug!("Access token has no jti; nothing to record");
        return Ok(());
    }
    let user_uuid = match uuid::Uuid::parse_str(sub) {
        Ok(u) => u,
        Err(e) => {
            tracing::debug!(
                error = %e,
                sub = %sub,
                "Access token sub is not a UUID; cannot record jti"
            );
            return Ok(());
        },
    };
    let exp =
        chrono::DateTime::<chrono::Utc>::from_timestamp(exp, 0).unwrap_or_else(chrono::Utc::now);
    repo.revoke_jti(jti, user_uuid, exp).await?;
    Ok(())
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}
