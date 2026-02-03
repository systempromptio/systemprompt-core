use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::webauthn::WebAuthnManager;
use systemprompt_oauth::OAuthState;
use tracing::instrument;
use webauthn_rs::prelude::RegisterPublicKeyCredential;

use super::LinkError;

#[derive(Debug, Deserialize)]
pub struct FinishLinkRequest {
    pub challenge_id: String,
    pub token: String,
    pub credential: RegisterPublicKeyCredential,
}

#[derive(Debug, Serialize)]
pub struct FinishLinkResponse {
    pub success: bool,
    pub user_id: String,
    pub message: String,
}

#[instrument(skip(state, request), fields(challenge_id = %request.challenge_id))]
pub async fn finish_link(
    State(state): State<OAuthState>,
    Json(request): Json<FinishLinkRequest>,
) -> impl IntoResponse {
    let oauth_repo = match OAuthRepository::new(Arc::clone(state.db_pool())) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "Failed to initialize repository");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(LinkError {
                    error: "server_error".to_string(),
                    error_description: format!("Repository initialization failed: {e}"),
                }),
            )
                .into_response();
        },
    };

    let user_provider = Arc::clone(state.user_provider());
    let webauthn_service =
        match WebAuthnManager::get_or_create_service(oauth_repo, user_provider).await {
            Ok(service) => service,
            Err(e) => {
                tracing::error!(error = %e, "Failed to initialize WebAuthn");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(LinkError {
                        error: "server_error".to_string(),
                        error_description: format!("Failed to initialize WebAuthn: {e}"),
                    }),
                )
                    .into_response();
            },
        };

    match webauthn_service
        .finish_registration_with_token(
            &request.challenge_id,
            &request.token,
            &request.credential,
            state.link_states(),
        )
        .await
    {
        Ok(user_id) => {
            tracing::info!(user_id = %user_id, "Credential linked successfully");
            (
                StatusCode::OK,
                Json(FinishLinkResponse {
                    success: true,
                    user_id,
                    message: "Passkey registered successfully".to_string(),
                }),
            )
                .into_response()
        },
        Err(e) => {
            tracing::warn!(error = %e, "Failed to finish credential linking");
            (
                StatusCode::BAD_REQUEST,
                Json(LinkError {
                    error: "link_failed".to_string(),
                    error_description: e.to_string(),
                }),
            )
                .into_response()
        },
    }
}
