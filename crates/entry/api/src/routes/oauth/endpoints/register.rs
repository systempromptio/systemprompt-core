//! RFC 7591 dynamic client registration endpoint.
//!
//! Registration is anonymous by design — MCP clients register on first
//! connect — so everything a registrant may claim is policed here: redirect
//! URIs by scheme and host, scopes by the self-registrable set, and a
//! secret only for confidential auth methods. The returned registration
//! access token is the only credential the RFC 7592 configuration endpoints
//! accept; its hash is stored with the client.
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
use systemprompt_oauth::models::TokenAuthMethod;
use systemprompt_oauth::oauth::dynamic_registration::{
    DynamicRegistrationRequest, DynamicRegistrationResponse,
};
use systemprompt_oauth::repository::{CreateClientParams, OAuthRepository};
use systemprompt_oauth::services::validation::{
    validate_client_metadata_uri, validate_registration_redirect_uris,
};
use systemprompt_oauth::services::{generate_registration_token, hash_registration_token};

use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;

fn is_unique_violation(err: &OauthError) -> bool {
    if let OauthError::Repository(sqlx::Error::Database(db_err)) = err {
        db_err.is_unique_violation()
    } else {
        false
    }
}

fn metadata_error(err: &OauthError) -> OAuthHttpError {
    OAuthHttpError::invalid_client_metadata(err.to_string())
}

struct ValidatedRegistration {
    client_name: String,
    application_type: String,
    redirect_uris: Vec<String>,
    grant_types: Vec<String>,
    response_types: Vec<String>,
    scopes: Vec<String>,
    token_endpoint_auth_method: TokenAuthMethod,
}

fn validate_registration(
    request: &DynamicRegistrationRequest,
) -> Result<ValidatedRegistration, OAuthHttpError> {
    let client_name = request.get_client_name().map_err(|e| metadata_error(&e))?;
    let application_type = request
        .get_application_type()
        .map_err(|e| metadata_error(&e))?;
    let redirect_uris = request
        .get_redirect_uris()
        .map_err(|e| metadata_error(&e))?;
    validate_registration_redirect_uris(&application_type, &redirect_uris)
        .map_err(|e| metadata_error(&e))?;
    validate_client_metadata_uri("client_uri", request.client_uri.as_deref())
        .map_err(|e| metadata_error(&e))?;
    validate_client_metadata_uri("logo_uri", request.logo_uri.as_deref())
        .map_err(|e| metadata_error(&e))?;
    let scopes = determine_scopes(request)
        .map_err(|e| OAuthHttpError::invalid_client_metadata(format!("Invalid scopes: {e}")))?;
    let token_endpoint_auth_method = request
        .get_token_endpoint_auth_method()
        .map_err(|e| metadata_error(&e))?;

    Ok(ValidatedRegistration {
        client_name,
        application_type,
        redirect_uris,
        grant_types: request.get_grant_types(),
        response_types: request.get_response_types(),
        scopes,
        token_endpoint_auth_method,
    })
}

pub async fn register_client(
    Extension(req_ctx): Extension<RequestContext>,
    OAuthRepo(repository): OAuthRepo,
    Json(request): Json<DynamicRegistrationRequest>,
) -> Result<Response, OAuthHttpError> {
    let config = Config::get()?;
    if !config.allow_dynamic_client_registration {
        return Err(OAuthHttpError::access_denied(
            "Dynamic client registration is disabled on this server",
        )
        .with_status(StatusCode::FORBIDDEN));
    }

    let validated = validate_registration(&request)?;

    let client_id = generate_client_id();
    let client_secret = match validated.token_endpoint_auth_method {
        TokenAuthMethod::None => None,
        TokenAuthMethod::ClientSecretPost | TokenAuthMethod::ClientSecretBasic => {
            Some(generate_opaque_token(32))
        },
    };
    let client_secret_hash = client_secret
        .as_deref()
        .map(|secret| hash(secret, 12))
        .transpose()
        .map_err(|e| OAuthHttpError::server_error(format!("Failed to hash client secret: {e}")))?;
    let registration_access_token = generate_registration_token();
    let registration_client_uri = format!(
        "{}/api/v1/core/oauth/register/{client_id}",
        config.api_server_url
    );

    let params = CreateClientParams {
        client_id: systemprompt_identifiers::ClientId::new(client_id.clone()),
        owner_user_id: req_ctx.auth.actor.user_id.clone(),
        client_secret_hash,
        registration_token_hash: Some(hash_registration_token(&registration_access_token)),
        client_name: validated.client_name.clone(),
        redirect_uris: validated.redirect_uris.clone(),
        grant_types: Some(validated.grant_types.clone()),
        response_types: Some(validated.response_types.clone()),
        scopes: validated.scopes.clone(),
        token_endpoint_auth_method: Some(validated.token_endpoint_auth_method.as_str().to_owned()),
        application_type: validated.application_type.clone(),
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
        client_id: systemprompt_identifiers::ClientId::new(client_id),
        client_secret_expires_at: client_secret.as_ref().map(|_| 0),
        client_secret,
        client_name: validated.client_name,
        redirect_uris: validated.redirect_uris,
        grant_types: validated.grant_types,
        response_types: validated.response_types,
        scope: validated.scopes.join(" "),
        token_endpoint_auth_method: validated.token_endpoint_auth_method.as_str().to_owned(),
        application_type: validated.application_type,
        client_uri: request.client_uri,
        logo_uri: request.logo_uri,
        contacts: request.contacts,
        client_id_issued_at: Utc::now(),
        registration_access_token,
        registration_client_uri,
    };

    Ok((StatusCode::CREATED, Json(response)).into_response())
}

fn generate_client_id() -> String {
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
            return OAuthRepository::validate_scopes_for_registration(&requested_scopes)
                .map_err(|e| format!("Invalid scopes requested: {e}"));
        }
    }

    let default_roles = OAuthRepository::get_default_roles();

    if default_roles.is_empty() {
        Ok(vec!["user".to_owned()])
    } else {
        Ok(default_roles)
    }
}
