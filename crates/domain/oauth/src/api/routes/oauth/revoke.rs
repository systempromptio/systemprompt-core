use crate::repository::OAuthRepository;
use crate::services::validation::{get_audit_user, validate_client_credentials};
use anyhow::Result;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Form, Json};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use tracing::instrument;

#[derive(Debug, Deserialize)]
pub struct RevokeRequest {
    pub token: String,
    pub token_type_hint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RevokeError {
    pub error: String,
    pub error_description: Option<String>,
}

#[instrument(skip(ctx, req_ctx, request))]
pub async fn handle_revoke(
    Extension(req_ctx): Extension<RequestContext>,
    State(ctx): State<AppContext>,
    Form(request): Form<RevokeRequest>,
) -> impl IntoResponse {
    let repo = match OAuthRepository::new(Arc::clone(ctx.db_pool())) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };

    let audit_user = match get_audit_user(Some(req_ctx.auth.user_id.as_str())) {
        Ok(user) => user,
        Err(e) => {
            let error = RevokeError {
                error: "invalid_request".to_string(),
                error_description: Some(format!("Authenticated user required: {e}")),
            };
            return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
        },
    };

    tracing::info!("Token revocation request received");

    let token_type = request
        .token_type_hint
        .as_deref()
        .unwrap_or("not_specified");
    let token_hash = hash_token(&request.token);

    if let Some(client_id) = &request.client_id {
        if validate_client_credentials(&repo, client_id, request.client_secret.as_deref())
            .await
            .is_err()
        {
            tracing::info!(
                token_hash = %token_hash,
                token_type = %token_type,
                client_id = %client_id,
                revocation_reason = "invalid_client_credentials",
                error = "invalid_client",
                "Token revocation failed"
            );

            let error = RevokeError {
                error: "invalid_client".to_string(),
                error_description: Some("Invalid client credentials".to_string()),
            };
            return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
        }
    }

    match revoke_token(&repo, &request.token, request.token_type_hint.as_deref()).await {
        Ok(()) => {
            tracing::info!(
                token_hash = %token_hash,
                token_type = %token_type,
                client_id = ?request.client_id,
                revocation_reason = "user_request",
                revoked_by = %audit_user,
                "Token revoked"
            );

            StatusCode::OK.into_response()
        },
        Err(error) => {
            tracing::info!(
                token_hash = %token_hash,
                token_type = %token_type,
                client_id = ?request.client_id,
                revocation_reason = "server_error",
                error = %error,
                revoked_by = %audit_user,
                "Token revocation failed"
            );

            let error = RevokeError {
                error: "server_error".to_string(),
                error_description: Some(error.to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response()
        },
    }
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

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}
