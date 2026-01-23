use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use bcrypt::{hash, DEFAULT_COST};
use tracing::instrument;
use uuid::Uuid;

use super::super::responses::{created_response, error_response};
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::RequestContext;
use systemprompt_oauth::clients::api::{CreateOAuthClientRequest, OAuthClientResponse};
use systemprompt_oauth::repository::{CreateClientParams, OAuthRepository};
use systemprompt_oauth::OAuthState;

fn init_error(e: impl std::fmt::Display) -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "server_error",
        format!("Repository initialization failed: {e}"),
    )
}

#[instrument(skip(state, req_ctx, request), fields(client_id = %request.client_id))]
pub async fn create_client(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<OAuthState>,
    Json(request): Json<CreateOAuthClientRequest>,
) -> impl IntoResponse {
    let repository = match OAuthRepository::new(Arc::clone(state.db_pool())) {
        Ok(r) => r,
        Err(e) => return init_error(e),
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
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "server_error",
                format!("Failed to hash client secret: {e}"),
            );
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
                    created_response(response_json, location)
                },
                Err(e) => error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "server_error",
                    format!("Failed to serialize response: {e}"),
                ),
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
                error_response(
                    StatusCode::CONFLICT,
                    "conflict",
                    "Client with this ID already exists".to_string(),
                )
            } else {
                error_response(StatusCode::BAD_REQUEST, "bad_request", error_msg)
            }
        },
    }
}
