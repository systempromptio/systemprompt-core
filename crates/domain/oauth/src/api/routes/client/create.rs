use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::response::{IntoResponse, Json};
use bcrypt::{hash, DEFAULT_COST};
use tracing::instrument;
use uuid::Uuid;

use crate::models::clients::api::{CreateOAuthClientRequest, OAuthClientResponse};
use crate::repository::{CreateClientParams, OAuthRepository};
use crate::OAuthState;
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::{ApiError, CreatedResponse, RequestContext};

#[instrument(skip(state, req_ctx, request), fields(client_id = %request.client_id))]
pub async fn create_client(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<OAuthState>,
    Json(request): Json<CreateOAuthClientRequest>,
) -> impl IntoResponse {
    let repository = match OAuthRepository::new(Arc::clone(state.db_pool())) {
        Ok(r) => r,
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };

    let client_secret = Uuid::new_v4().to_string();
    let client_secret_hash = match hash(&client_secret, DEFAULT_COST) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!(
                error = %e,
                client_id = %request.client_id,
                created_by = %req_ctx.auth.user_id,
                "OAuth client creation failed"
            );
            return ApiError::internal_error(format!("Failed to hash client secret: {e}"))
                .into_response();
        },
    };

    let params = CreateClientParams {
        client_id: request.client_id.clone(),
        client_secret_hash,
        client_name: request.name.clone(),
        redirect_uris: request.redirect_uris.clone(),
        grant_types: Some(vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ]),
        response_types: Some(vec!["code".to_string()]),
        scopes: request.scopes.clone(),
        token_endpoint_auth_method: Some("client_secret_basic".to_string()),
        client_uri: None,
        logo_uri: None,
        contacts: None,
    };

    match repository.create_client(params).await {
        Ok(client) => {
            tracing::info!(
                client_id = %client.client_id,
                client_name = ?client.name,
                redirect_uris = ?request.redirect_uris,
                scopes = ?request.scopes,
                created_by = %req_ctx.auth.user_id,
                "OAuth client created"
            );

            let location = ApiPaths::oauth_client_location(client.client_id.as_str());
            let response: OAuthClientResponse = client.into();

            match serde_json::to_value(response) {
                Ok(mut response_json) => {
                    response_json["client_secret"] = serde_json::Value::String(client_secret);
                    CreatedResponse::new(response_json, location).into_response()
                },
                Err(e) => ApiError::internal_error(format!("Failed to serialize response: {e}"))
                    .into_response(),
            }
        },
        Err(e) => {
            let error_msg = format!("Failed to create client: {e}");
            let is_duplicate = error_msg.contains("UNIQUE constraint failed");

            tracing::info!(
                client_id = %request.client_id,
                reason = if is_duplicate { "Client ID already exists" } else { &error_msg },
                created_by = %req_ctx.auth.user_id,
                "OAuth client creation rejected"
            );

            if is_duplicate {
                ApiError::conflict("Client with this ID already exists").into_response()
            } else {
                ApiError::bad_request(error_msg).into_response()
            }
        },
    }
}
