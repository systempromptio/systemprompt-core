use anyhow::Result;
use axum::Form;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use jsonwebtoken::dangerous::insecure_decode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use systemprompt_identifiers::SessionId;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::JwtClaims;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::validation::{get_audit_user, validate_client_credentials};
use tracing::instrument;

use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;

#[derive(Debug, Deserialize)]
pub struct RevokeRequest {
    pub token: String,
    pub token_type_hint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

#[instrument(skip(state, req_ctx, request, repo))]
pub async fn handle_revoke(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<OAuthState>,
    OAuthRepo(repo): OAuthRepo,
    Form(request): Form<RevokeRequest>,
) -> Result<Response, OAuthHttpError> {
    let audit_user = get_audit_user(Some(&req_ctx.auth.actor.user_id))
        .map_err(|e| OAuthHttpError::invalid_request(format!("Authenticated user required: {e}")))?;

    let token_type = request
        .token_type_hint
        .as_deref()
        .unwrap_or("not_specified");
    let token_hash = hash_token(&request.token);

    if let Some(client_id_str) = &request.client_id {
        let client_id = systemprompt_identifiers::ClientId::new(client_id_str.clone());
        if validate_client_credentials(&repo, &client_id, request.client_secret.as_deref())
            .await
            .is_err()
        {
            return Err(OAuthHttpError::invalid_client("Invalid client credentials"));
        }
    }

    revoke_token(&repo, &request.token, request.token_type_hint.as_deref()).await?;

    if let Some(session_id) = extract_session_id_unverified(&request.token)
        && let Err(e) = state.analytics_provider().revoke_session(&session_id).await
    {
        tracing::warn!(
            session_id = %session_id,
            error = %e,
            "Failed to revoke session after token revocation"
        );
    }

    tracing::info!(
        token_hash = %token_hash,
        token_type = %token_type,
        client_id = ?request.client_id,
        revocation_reason = "user_request",
        revoked_by = %audit_user,
        "Token revoked"
    );

    Ok(StatusCode::OK.into_response())
}

async fn revoke_token(
    repo: &OAuthRepository,
    token: &str,
    token_type_hint: Option<&str>,
) -> Result<()> {
    use systemprompt_identifiers::RefreshTokenId;

    match token_type_hint {
        Some("refresh_token") => {
            let token_id = RefreshTokenId::new(token);
            repo.revoke_refresh_token(&token_id).await?;
        },
        Some("access_token") => {
            tracing::debug!("Access token revocation requested - JWT tokens are stateless");
        },
        _ => {
            let token_id = RefreshTokenId::new(token);
            if let Err(e) = repo.revoke_refresh_token(&token_id).await {
                tracing::debug!(error = %e, "Token revocation failed - may be access token");
            }
        },
    }

    Ok(())
}

/// Decode an access-token JWT without verifying its signature to recover the
/// `session_id` claim for server-side revocation.
///
/// Why: RFC 7009 token revocation already authenticates the *client*; the
/// signature on the token itself is irrelevant for the revocation decision
/// (we only need to know which session to mark revoked). A forged token will
/// produce a `session_id` that either doesn't exist or belongs to a different
/// principal — either way the `revoke_session` query is a no-op or rejected
/// downstream, and we have not accepted the token for authentication.
fn extract_session_id_unverified(token: &str) -> Option<SessionId> {
    let data = insecure_decode::<JwtClaims>(token).ok()?;
    data.claims.session_id
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}
