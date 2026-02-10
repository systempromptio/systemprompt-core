use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use systemprompt_models::Config;

use systemprompt_oauth::oauth::dynamic_registration::DynamicRegistrationResponse;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::OAuthState;

pub async fn get_client_configuration(
    State(state): State<OAuthState>,
    Path(client_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let repository = match OAuthRepository::new(state.db_pool()) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };
    let auth_header = match headers.get("authorization") {
        Some(header) => match header.to_str() {
            Ok(value) => value,
            Err(_) => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "error": "invalid_token",
                        "error_description": "Invalid authorization header format"
                    })),
                )
                    .into_response();
            },
        },
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "invalid_token",
                    "error_description": "Missing authorization header"
                })),
            )
                .into_response();
        },
    };

    let Some(token) = auth_header.strip_prefix("Bearer ") else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "invalid_token",
                "error_description": "Authorization header must use Bearer scheme"
            })),
        )
            .into_response();
    };

    if !token.starts_with("reg_") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "invalid_token",
                "error_description": "Invalid registration access token format"
            })),
        )
            .into_response();
    }

    match repository.find_client_by_id(&client_id).await {
        Ok(Some(client)) => {
            let base_url = match Config::get() {
                Ok(c) => c.api_server_url.clone(),
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({
                            "error": "server_error",
                            "error_description": format!("Configuration unavailable: {e}")
                        })),
                    )
                        .into_response();
                },
            };

            let response = DynamicRegistrationResponse {
                client_id: client.client_id.to_string(),
                client_secret: "***REDACTED***".to_string(),
                client_name: client.client_name,
                redirect_uris: client.redirect_uris,
                grant_types: client.grant_types,
                response_types: client.response_types,
                scope: client.scopes.join(" "),
                token_endpoint_auth_method: client.token_endpoint_auth_method,
                client_uri: client.client_uri,
                logo_uri: client.logo_uri,
                contacts: client.contacts,
                client_secret_expires_at: 0,
                client_id_issued_at: client.created_at,
                registration_access_token: token.to_string(),
                registration_client_uri: format!(
                    "{base_url}/api/v1/core/oauth/register/{client_id}"
                ),
            };

            (StatusCode::OK, Json(response)).into_response()
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "invalid_client_metadata",
                "error_description": "Client not found"
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "server_error",
                "error_description": format!("Database error: {e}")
            })),
        )
            .into_response(),
    }
}
