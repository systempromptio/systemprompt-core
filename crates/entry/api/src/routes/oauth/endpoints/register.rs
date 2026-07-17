//! RFC 7591 dynamic client registration endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use bcrypt::hash;
use chrono::Utc;
use rand::Rng;
use systemprompt_models::{Config, RequestContext};
use uuid::Uuid;

use systemprompt_oauth::OauthError;
use systemprompt_oauth::oauth::dynamic_registration::{
    DynamicRegistrationRequest, DynamicRegistrationResponse,
};
use systemprompt_oauth::repository::{CreateClientParams, OAuthRepository};

use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;

fn is_unique_violation(err: &OauthError) -> bool {
    if let OauthError::Repository(sqlx::Error::Database(db_err)) = err {
        db_err.is_unique_violation()
    } else {
        false
    }
}

pub async fn register_client(
    Extension(req_ctx): Extension<RequestContext>,
    OAuthRepo(repository): OAuthRepo,
    Json(request): Json<DynamicRegistrationRequest>,
) -> Result<Response, OAuthHttpError> {
    let client_id = generate_client_id(&request);
    let client_secret = generate_opaque_token(32);
    let registration_access_token = format!("reg_{}", generate_opaque_token(32));

    let base_url = Config::get()?.api_server_url.clone();
    let registration_client_uri = format!("{base_url}/api/v1/core/oauth/register/{client_id}");

    let client_secret_hash = hash(&client_secret, 12)
        .map_err(|e| OAuthHttpError::server_error(format!("Failed to hash client secret: {e}")))?;

    let client_name = request
        .get_client_name()
        .map_err(|e| OAuthHttpError::invalid_client_metadata(e.to_string()))?;
    let redirect_uris = request
        .get_redirect_uris()
        .map_err(|e| OAuthHttpError::invalid_client_metadata(e.to_string()))?;
    let grant_types = request.get_grant_types();
    let response_types = request.get_response_types();
    let scopes = determine_scopes(&request)
        .map_err(|e| OAuthHttpError::invalid_client_metadata(format!("Invalid scopes: {e}")))?;
    let token_endpoint_auth_method = request.get_token_endpoint_auth_method();
    let application_type = request
        .get_application_type()
        .map_err(|e| OAuthHttpError::invalid_client_metadata(e.to_string()))?;

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
        application_type: application_type.clone(),
        client_uri: request.client_uri.clone(),
        logo_uri: request.logo_uri.clone(),
        contacts: request.contacts.clone(),
    };

    repository.create_client(params).await.map_err(|e| {
        if is_unique_violation(&e) {
            OAuthHttpError::invalid_client_metadata("Client with this ID already exists")
                .with_status(StatusCode::CONFLICT)
        } else {
            OAuthHttpError::invalid_client_metadata(format!("Failed to register client: {e}"))
        }
    })?;

    let response = DynamicRegistrationResponse {
        client_id: systemprompt_identifiers::ClientId::new(client_id.clone()),
        client_secret,
        client_name,
        redirect_uris,
        grant_types,
        response_types,
        scope: scopes.join(" "),
        token_endpoint_auth_method,
        application_type,
        client_uri: request.client_uri,
        logo_uri: request.logo_uri,
        contacts: request.contacts,
        client_secret_expires_at: 0,
        client_id_issued_at: Utc::now(),
        registration_access_token,
        registration_client_uri,
    };

    Ok((StatusCode::CREATED, Json(response)).into_response())
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
        let requested_scopes: Vec<String> =
            scope_string.split_whitespace().map(str::to_owned).collect();

        if !requested_scopes.is_empty() {
            let valid_requested = OAuthRepository::validate_scopes(&requested_scopes)
                .map_err(|e| format!("Invalid scopes requested: {e}"))?;

            return Ok(valid_requested);
        }
    }

    let default_roles = OAuthRepository::get_default_roles();

    if default_roles.is_empty() {
        Ok(vec!["user".to_owned()])
    } else {
        Ok(default_roles)
    }
}
