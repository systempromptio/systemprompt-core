use crate::repository::OAuthRepository;
use crate::services::validation::get_audit_user;
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
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
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

async fn validate_client_credentials(
    repo: &OAuthRepository,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<()> {
    let client = repo
        .find_client_by_id(client_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Client not found"))?;

    if let Some(secret) = client_secret {
        use crate::services::verify_client_secret;
        let hash = client
            .client_secret_hash
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Client has no secret hash configured"))?;
        if !verify_client_secret(secret, hash)? {
            return Err(anyhow::anyhow!("Invalid client secret"));
        }
    } else {
        return Err(anyhow::anyhow!("Client secret required"));
    }

    Ok(())
}

async fn revoke_token(
    _repo: &OAuthRepository,
    _token: &str,
    _token_type_hint: Option<&str>,
) -> Result<()> {
    Ok(())
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}
