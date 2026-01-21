use crate::repository::OAuthRepository;
use crate::services::webauthn::WebAuthnManager;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_users::{UserProviderImpl, UserService};
use tracing::instrument;
use webauthn_rs::prelude::*;

#[derive(Debug, Deserialize)]
pub struct StartAuthQuery {
    pub email: String,
    pub oauth_state: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StartAuthResponse {
    #[serde(rename = "publicKey")]
    pub public_key: serde_json::Value,
    pub challenge_id: String,
}

#[derive(Debug, Serialize)]
pub struct AuthError {
    pub error: String,
    pub error_description: String,
}

#[allow(unused_qualifications)]
#[instrument(skip(ctx, params), fields(email = %params.email))]
pub async fn start_auth(
    Query(params): Query<StartAuthQuery>,
    State(ctx): State<systemprompt_runtime::AppContext>,
) -> impl IntoResponse {
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
                Json(AuthError {
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
                    Json(AuthError {
                        error: "server_error".to_string(),
                        error_description: format!("Failed to initialize WebAuthn: {e}"),
                    }),
                )
                    .into_response();
            },
        };

    match webauthn_service
        .start_authentication(&params.email, params.oauth_state)
        .await
    {
        Ok((challenge, challenge_id)) => {
            let challenge_json = match serde_json::to_value(&challenge) {
                Ok(json) => json,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AuthError {
                            error: "server_error".to_string(),
                            error_description: format!("Failed to serialize challenge: {e}"),
                        }),
                    )
                        .into_response();
                },
            };

            let mut public_key = match challenge_json.get("publicKey") {
                Some(pk) => pk.clone(),
                None => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AuthError {
                            error: "server_error".to_string(),
                            error_description: "Missing publicKey in challenge".to_string(),
                        }),
                    )
                        .into_response();
                },
            };

            if let Some(obj) = public_key.as_object_mut() {
                obj.remove("authenticatorAttachment");
            }

            (
                StatusCode::OK,
                Json(StartAuthResponse {
                    public_key,
                    challenge_id,
                }),
            )
                .into_response()
        },
        Err(e) => {
            let status_code = if e.to_string().contains("User not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::BAD_REQUEST
            };

            (
                status_code,
                Json(AuthError {
                    error: "authentication_failed".to_string(),
                    error_description: e.to_string(),
                }),
            )
                .into_response()
        },
    }
}

#[derive(Debug, Deserialize)]
pub struct FinishAuthRequest {
    pub challenge_id: String,
    pub credential: PublicKeyCredential,
}

#[derive(Debug, Serialize)]
pub struct FinishAuthResponse {
    pub user_id: String,
    pub oauth_state: Option<String>,
    pub success: bool,
}

#[instrument(skip(ctx, request), fields(challenge_id = %request.challenge_id))]
pub async fn finish_auth(
    State(ctx): State<systemprompt_runtime::AppContext>,
    Json(request): Json<FinishAuthRequest>,
) -> impl IntoResponse {
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
                Json(AuthError {
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
                    Json(AuthError {
                        error: "server_error".to_string(),
                        error_description: format!("Failed to initialize WebAuthn: {e}"),
                    }),
                )
                    .into_response();
            },
        };

    match webauthn_service
        .finish_authentication(&request.challenge_id, &request.credential)
        .await
    {
        Ok((user_id, oauth_state)) => (
            StatusCode::OK,
            Json(FinishAuthResponse {
                user_id,
                oauth_state,
                success: true,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::UNAUTHORIZED,
            Json(AuthError {
                error: "authentication_failed".to_string(),
                error_description: e.to_string(),
            }),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct DevAuthQuery {
    pub email: String,
    pub oauth_state: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DevAuthResponse {
    pub user_id: String,
    pub oauth_state: Option<String>,
    pub success: bool,
}

fn is_dev_mode() -> bool {
    std::env::var("DANGEROUSLY_BYPASS_OAUTH")
        .map(|s| s.to_lowercase() == "true")
        .unwrap_or(false)
}

#[allow(unused_qualifications)]
#[instrument(skip(ctx, params), fields(email = %params.email))]
pub async fn dev_auth(
    Query(params): Query<DevAuthQuery>,
    State(ctx): State<systemprompt_runtime::AppContext>,
) -> impl IntoResponse {
    if !is_dev_mode() {
        return (
            StatusCode::FORBIDDEN,
            Json(AuthError {
                error: "forbidden".to_string(),
                error_description: "Development authentication not available in production"
                    .to_string(),
            }),
        )
            .into_response();
    }

    let user_service = match UserService::new(ctx.db_pool()) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthError {
                    error: "server_error".to_string(),
                    error_description: format!("Failed to create user service: {e}"),
                }),
            )
                .into_response();
        },
    };

    match user_service.find_by_email(&params.email).await {
        Ok(Some(user)) => {
            tracing::info!(email = %params.email, "DEV: Email-based authentication");

            (
                StatusCode::OK,
                Json(DevAuthResponse {
                    user_id: user.id.to_string(),
                    oauth_state: params.oauth_state,
                    success: true,
                }),
            )
                .into_response()
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(AuthError {
                error: "user_not_found".to_string(),
                error_description: format!("User not found: {}", params.email),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AuthError {
                error: "server_error".to_string(),
                error_description: format!("Database error: {e}"),
            }),
        )
            .into_response(),
    }
}
