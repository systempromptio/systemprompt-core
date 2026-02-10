use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::webauthn::WebAuthnManager;
use systemprompt_oauth::OAuthState;
use tracing::instrument;

use super::LinkError;

#[derive(Debug, Deserialize)]
pub struct StartLinkQuery {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct StartLinkUserInfo {
    pub id: String,
    pub email: String,
    pub name: String,
}

#[allow(unused_qualifications)]
#[instrument(skip(state, params), fields(token_prefix = %params.token.chars().take(12).collect::<String>()))]
pub async fn start_link(
    Query(params): Query<StartLinkQuery>,
    State(state): State<OAuthState>,
) -> impl IntoResponse {
    if params.token.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(LinkError {
                error: "invalid_request".to_string(),
                error_description: "Token is required".to_string(),
            }),
        )
            .into_response();
    }

    let oauth_repo = match OAuthRepository::new(state.db_pool()) {
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
        .start_registration_with_token(&params.token, state.link_states())
        .await
    {
        Ok((challenge, challenge_id, user_info)) => {
            let mut challenge_json = match serde_json::to_value(&challenge) {
                Ok(json) => json,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(LinkError {
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
                    Json(LinkError {
                        error: "server_error".to_string(),
                        error_description: format!("Invalid challenge ID format: {e}"),
                    }),
                )
                    .into_response()
            });

            match header_value {
                Ok(val) => {
                    headers.insert(HeaderName::from_static("x-challenge-id"), val);

                    let response = serde_json::json!({
                        "challenge": challenge_json,
                        "user": StartLinkUserInfo {
                            id: user_info.id,
                            email: user_info.email,
                            name: user_info.name,
                        }
                    });

                    (StatusCode::OK, headers, Json(response)).into_response()
                },
                Err(response) => response,
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "Failed to start credential linking");
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
