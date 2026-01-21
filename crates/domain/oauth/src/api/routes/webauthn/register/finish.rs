use crate::repository::OAuthRepository;
use crate::services::webauthn::{FinishRegistrationParams, WebAuthnManager};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_users::{UserProviderImpl, UserService};
use tracing::instrument;
use webauthn_rs::prelude::*;

use super::RegisterError;

#[derive(Debug, Deserialize)]
pub struct FinishRegisterRequest {
    pub challenge_id: String,
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
    pub credential: RegisterPublicKeyCredential,
    #[serde(default)]
    pub session_id: Option<String>,
}

impl FinishRegisterRequest {
    fn validate(&self) -> Result<(), String> {
        if self.challenge_id.trim().is_empty() {
            return Err("Challenge ID is required".to_string());
        }

        if self.username.trim().is_empty() {
            return Err("Username is required and cannot be empty".to_string());
        }

        if self.username.len() > 50 {
            return Err("Username must be less than 50 characters".to_string());
        }

        if !self
            .username
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(
                "Username can only contain letters, numbers, underscores, and hyphens".to_string(),
            );
        }

        if self.email.trim().is_empty() {
            return Err("Email is required and cannot be empty".to_string());
        }

        if !self.email.contains('@') || !self.email.contains('.') {
            return Err("Email must be a valid email address".to_string());
        }

        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct FinishRegisterResponse {
    pub user_id: String,
    pub success: bool,
}

#[instrument(skip(ctx, request), fields(challenge_id = %request.challenge_id, username = %request.username))]
pub async fn finish_register(
    State(ctx): State<systemprompt_runtime::AppContext>,
    Json(request): Json<FinishRegisterRequest>,
) -> impl IntoResponse {
    if let Err(validation_error) = request.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(RegisterError {
                error: "invalid_request".to_string(),
                error_description: validation_error,
            }),
        )
            .into_response();
    }

    let oauth_repo = match OAuthRepository::new(Arc::clone(ctx.db_pool())) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };
    let user_service = match UserService::new(ctx.db_pool()) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create user service");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RegisterError {
                    error: "server_error".to_string(),
                    error_description: format!("Failed to create user service: {e}"),
                }),
            )
                .into_response();
        },
    };
    let user_provider = Arc::new(UserProviderImpl::new(user_service));

    let webauthn_service =
        match WebAuthnManager::get_or_create_service(oauth_repo, user_provider).await {
            Ok(service) => service,
            Err(e) => {
                tracing::error!(error = %e, "Failed to initialize WebAuthn");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(RegisterError {
                        error: "server_error".to_string(),
                        error_description: format!("Failed to initialize WebAuthn: {e}"),
                    }),
                )
                    .into_response();
            },
        };

    let mut builder = FinishRegistrationParams::builder(
        &request.challenge_id,
        &request.username,
        &request.email,
        &request.credential,
    );
    if let Some(ref name) = request.full_name {
        builder = builder.with_full_name(name);
    }

    match webauthn_service.finish_registration(builder.build()).await {
        Ok(user_id) => {
            if let Some(session_id_str) = &request.session_id {
                use systemprompt_analytics::SessionRepository;
                use systemprompt_identifiers::{SessionId, UserId};

                let session_id = SessionId::new(session_id_str.clone());
                let session_repo = SessionRepository::new(Arc::clone(ctx.db_pool()));

                match session_repo.find_by_id(&session_id).await {
                    Ok(Some(session)) => {
                        if let Some(old_user_id_str) = session.user_id {
                            let old_user_id = UserId::new(old_user_id_str);
                            let new_user_id = UserId::new(user_id.clone());

                            match session_repo
                                .migrate_user_sessions(&old_user_id, &new_user_id)
                                .await
                            {
                                Ok(count) => {
                                    tracing::info!(
                                        session_id = %session_id,
                                        old_user_id = %old_user_id,
                                        new_user_id = %new_user_id,
                                        records_migrated = count,
                                        "Successfully migrated user data"
                                    );
                                },
                                Err(e) => {
                                    tracing::error!(
                                        error = %e,
                                        session_id = %session_id,
                                        old_user_id = %old_user_id,
                                        new_user_id = %new_user_id,
                                        "Failed to migrate session"
                                    );
                                },
                            }
                        }
                    },
                    Ok(None) => {
                        tracing::warn!(session_id = %session_id, "Session not found for migration");
                    },
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            session_id = %session_id,
                            "Failed to retrieve session for migration"
                        );
                    },
                }
            }

            (
                StatusCode::OK,
                Json(FinishRegisterResponse {
                    user_id,
                    success: true,
                }),
            )
                .into_response()
        },
        Err(e) => {
            let error_msg = e.to_string();
            let (status, error_code, description) = if error_msg.contains("username_already_taken")
            {
                (
                    StatusCode::CONFLICT,
                    "username_unavailable",
                    "Username is already taken. Please choose a different username.".to_string(),
                )
            } else if error_msg.contains("email_already_registered") {
                (
                    StatusCode::CONFLICT,
                    "email_exists",
                    "An account with this email already exists.".to_string(),
                )
            } else if error_msg.contains("Registration state not found") {
                (
                    StatusCode::BAD_REQUEST,
                    "expired_challenge",
                    "Registration challenge has expired. Please start the registration process \
                     again."
                        .to_string(),
                )
            } else if error_msg.contains("finish_passkey_registration")
                || error_msg.contains("verification")
                || error_msg.contains("attestation")
            {
                (
                    StatusCode::BAD_REQUEST,
                    "invalid_credential",
                    "WebAuthn verification failed. Please ensure your authenticator and browser \
                     are compatible."
                        .to_string(),
                )
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "registration_failed",
                    format!("Registration failed: {error_msg}"),
                )
            };

            (
                status,
                Json(RegisterError {
                    error: error_code.to_string(),
                    error_description: description,
                }),
            )
                .into_response()
        },
    }
}
