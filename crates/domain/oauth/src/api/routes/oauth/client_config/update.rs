use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use std::sync::Arc;
use systemprompt_models::Config;
use systemprompt_runtime::AppContext;

use super::validation::validate_registration_token;
use crate::models::oauth::dynamic_registration::{
    DynamicRegistrationRequest, DynamicRegistrationResponse,
};
use crate::repository::OAuthRepository;

pub async fn update_client_configuration(
    State(ctx): State<AppContext>,
    Path(client_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<DynamicRegistrationRequest>,
) -> impl IntoResponse {
    let repository = match OAuthRepository::new(Arc::clone(ctx.db_pool())) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };
    let registration_token = match validate_registration_token(&headers) {
        Ok(token) => token,
        Err(response) => return *response,
    };

    let existing_client = match repository.find_client_by_id(&client_id).await {
        Ok(Some(client)) => client,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "invalid_client_metadata",
                    "error_description": "Client not found"
                })),
            )
                .into_response();
        },
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "server_error",
                    "error_description": format!("Database error: {e}")
                })),
            )
                .into_response();
        },
    };

    let client_name = match request.get_client_name() {
        Ok(name) => name,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_client_metadata",
                    "error_description": e
                })),
            )
                .into_response();
        },
    };
    let redirect_uris = match request.get_redirect_uris() {
        Ok(mut uris) => {
            uris.sort();
            uris.dedup();
            uris
        },
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_client_metadata",
                    "error_description": e
                })),
            )
                .into_response();
        },
    };

    match repository
        .update_client(
            &client_id,
            Some(&client_name),
            Some(&redirect_uris),
            Some(&existing_client.scopes),
        )
        .await
    {
        Ok(_) => {
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
                client_id: client_id.clone(),
                client_secret: "***REDACTED***".to_string(),
                client_name,
                redirect_uris,
                grant_types: existing_client.grant_types,
                response_types: existing_client.response_types,
                scope: existing_client.scopes.join(" "),
                token_endpoint_auth_method: existing_client.token_endpoint_auth_method,
                client_uri: request.client_uri,
                logo_uri: request.logo_uri,
                contacts: request.contacts,
                client_secret_expires_at: 0,
                client_id_issued_at: existing_client.created_at,
                registration_access_token: registration_token.clone(),
                registration_client_uri: format!(
                    "{base_url}/api/v1/core/oauth/register/{client_id}"
                ),
            };

            (StatusCode::OK, Json(response)).into_response()
        },
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_client_metadata",
                "error_description": format!("Failed to update client: {e}")
            })),
        )
            .into_response(),
    }
}
