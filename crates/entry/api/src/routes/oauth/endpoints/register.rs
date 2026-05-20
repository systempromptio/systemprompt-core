use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use bcrypt::hash;
use chrono::Utc;
use rand::Rng;
use systemprompt_models::{Config, RequestContext};
use uuid::Uuid;

use systemprompt_oauth::oauth::dynamic_registration::{
    DynamicRegistrationRequest, DynamicRegistrationResponse,
};
use systemprompt_oauth::repository::{CreateClientParams, OAuthRepository};

use crate::routes::oauth::extractors::OAuthRepo;

pub async fn register_client(
    Extension(req_ctx): Extension<RequestContext>,
    OAuthRepo(repository): OAuthRepo,
    Json(request): Json<DynamicRegistrationRequest>,
) -> impl IntoResponse {
    let client_id = generate_client_id(&request);
    let client_secret = generate_opaque_token(32);
    let registration_access_token = format!("reg_{}", generate_opaque_token(32));
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
    let registration_client_uri = format!("{base_url}/api/v1/core/oauth/register/{client_id}");

    let client_secret_hash = match hash(&client_secret, 12) {
        Ok(hash) => hash,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "server_error",
                    "error_description": format!("Failed to hash client secret: {e}")
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
        Ok(uris) => uris,
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

    let grant_types = match request.get_grant_types() {
        Ok(types) => types,
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

    let response_types = match request.get_response_types() {
        Ok(types) => types,
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

    let scopes = match determine_scopes(&request) {
        Ok(scopes) => scopes,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_client_metadata",
                    "error_description": format!("Invalid scopes: {e}")
                })),
            )
                .into_response();
        },
    };

    let token_endpoint_auth_method = request.get_token_endpoint_auth_method();

    let params = CreateClientParams {
        client_id: systemprompt_identifiers::ClientId::new(client_id.clone()),
        owner_user_id: req_ctx.auth.actor.user_id.clone(),
        client_secret_hash,
        client_name: client_name.clone(),
        redirect_uris: redirect_uris.clone(),
        grant_types: Some(grant_types.clone()),
        response_types: Some(response_types.clone()),
        scopes: scopes.clone(),
        token_endpoint_auth_method: Some(token_endpoint_auth_method.clone()),
        client_uri: request.client_uri.clone(),
        logo_uri: request.logo_uri.clone(),
        contacts: request.contacts.clone(),
    };

    match repository.create_client(params).await {
        Ok(_) => {
            let response = DynamicRegistrationResponse {
                client_id: systemprompt_identifiers::ClientId::new(client_id.clone()),
                client_secret,
                client_name,
                redirect_uris,
                grant_types,
                response_types,
                scope: scopes.join(" "),
                token_endpoint_auth_method,
                client_uri: request.client_uri,
                logo_uri: request.logo_uri,
                contacts: request.contacts,
                client_secret_expires_at: 0,
                client_id_issued_at: Utc::now(),
                registration_access_token,
                registration_client_uri,
            };

            (StatusCode::CREATED, Json(response)).into_response()
        },
        Err(e) => {
            let error_msg = format!("Failed to register client: {e}");
            if error_msg.contains("UNIQUE constraint failed") {
                (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "error": "invalid_client_metadata",
                        "error_description": "Client with this ID already exists"
                    })),
                )
                    .into_response()
            } else {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "invalid_client_metadata",
                        "error_description": error_msg
                    })),
                )
                    .into_response()
            }
        },
    }
}

fn generate_client_id(_request: &DynamicRegistrationRequest) -> String {
    format!("client_{}", Uuid::new_v4().simple())
}

fn generate_opaque_token(byte_len: usize) -> String {
    let mut buf = vec![0u8; byte_len];
    rand::rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(&buf)
}

fn determine_scopes(request: &DynamicRegistrationRequest) -> Result<Vec<String>, String> {
    if let Some(scope_string) = &request.scope {
        let requested_scopes: Vec<String> = scope_string
            .split_whitespace()
            .map(ToString::to_string)
            .collect();

        if !requested_scopes.is_empty() {
            let valid_requested = OAuthRepository::validate_scopes(&requested_scopes)
                .map_err(|e| format!("Invalid scopes requested: {e}"))?;

            return Ok(valid_requested);
        }
    }

    let default_roles = OAuthRepository::get_default_roles();

    if default_roles.is_empty() {
        Ok(vec!["user".to_string()])
    } else {
        Ok(default_roles)
    }
}
