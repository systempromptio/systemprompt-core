use crate::repository::OAuthRepository;
use crate::services::webauthn::WebAuthnManager;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;
use systemprompt_users::{UserProviderImpl, UserService};
use tracing::instrument;

use super::RegisterError;

#[derive(Debug, Deserialize)]
pub struct StartRegisterQuery {
    pub username: String,
    pub email: String,
    pub full_name: Option<String>,
}

impl StartRegisterQuery {
    fn validate(&self) -> Result<(), String> {
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

#[allow(unused_qualifications)]
#[instrument(skip(ctx, params), fields(username = %params.username, email = %params.email))]
pub async fn start_register(
    Query(params): Query<StartRegisterQuery>,
    State(ctx): State<systemprompt_runtime::AppContext>,
) -> impl IntoResponse {
    if let Err(validation_error) = params.validate() {
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
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
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

    match webauthn_service
        .start_registration(&params.username, &params.email, params.full_name.as_deref())
        .await
    {
        Ok((challenge, challenge_id)) => {
            let mut challenge_json = match serde_json::to_value(&challenge) {
                Ok(json) => json,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(RegisterError {
                            error: "server_error".to_string(),
                            error_description: format!("Failed to serialize challenge: {e}"),
                        }),
                    )
                        .into_response();
                },
            };

            if let Some(public_key) = challenge_json.get_mut("publicKey") {
                if let Some(authenticator_selection) = public_key.get_mut("authenticatorSelection")
                {
                    if let Some(obj) = authenticator_selection.as_object_mut() {
                        obj.remove("authenticatorAttachment");
                    }
                }
            }

            let mut headers = HeaderMap::new();
            let header_value = HeaderValue::from_str(&challenge_id).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(RegisterError {
                        error: "server_error".to_string(),
                        error_description: format!("Invalid challenge ID format: {e}"),
                    }),
                )
                    .into_response()
            });

            match header_value {
                Ok(val) => {
                    headers.insert(HeaderName::from_static("x-challenge-id"), val);
                    (StatusCode::OK, headers, Json(challenge_json)).into_response()
                },
                Err(response) => response,
            }
        },
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(RegisterError {
                error: "registration_failed".to_string(),
                error_description: e.to_string(),
            }),
        )
            .into_response(),
    }
}
