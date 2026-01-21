use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json};
use std::sync::Arc;

use super::validation::validate_registration_token;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::OAuthState;

pub async fn delete_client_configuration(
    State(state): State<OAuthState>,
    Path(client_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let repository = match OAuthRepository::new(Arc::clone(state.db_pool())) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };
    if let Err(response) = validate_registration_token(&headers) {
        return *response;
    }

    match repository.find_client_by_id(&client_id).await {
        Ok(Some(_)) => match repository.delete_client(&client_id).await {
            Ok(_) => StatusCode::NO_CONTENT.into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "server_error",
                    "error_description": format!("Failed to delete client: {e}")
                })),
            )
                .into_response(),
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
