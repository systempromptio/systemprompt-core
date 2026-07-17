//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use systemprompt_models::Config;

use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;
use systemprompt_oauth::oauth::dynamic_registration::DynamicRegistrationResponse;

fn extract_registration_token(headers: &HeaderMap) -> Result<String, OAuthHttpError> {
    let auth_header = headers
        .get("authorization")
        .ok_or_else(|| OAuthHttpError::invalid_token("Missing authorization header"))?
        .to_str()
        .map_err(|_e| OAuthHttpError::invalid_token("Invalid authorization header format"))?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        OAuthHttpError::invalid_token("Authorization header must use Bearer scheme")
    })?;

    if !token.starts_with("reg_") {
        return Err(OAuthHttpError::invalid_token(
            "Invalid registration access token format",
        ));
    }

    Ok(token.to_owned())
}

pub async fn get_client_configuration(
    OAuthRepo(repository): OAuthRepo,
    Path(client_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, OAuthHttpError> {
    let token = extract_registration_token(&headers)?;

    let client_id = systemprompt_identifiers::ClientId::new(&client_id);
    let client = repository
        .find_client_by_id(&client_id)
        .await?
        .ok_or_else(|| OAuthHttpError::invalid_client_metadata("Client not found"))?;

    let base_url = Config::get()
        .map_err(|e| OAuthHttpError::server_error(format!("Configuration unavailable: {e}")))?
        .api_server_url
        .clone();

    let response = DynamicRegistrationResponse {
        client_id: client.client_id.clone(),
        client_secret: "***REDACTED***".to_owned(),
        client_name: client.client_name,
        redirect_uris: client.redirect_uris,
        grant_types: client.grant_types,
        response_types: client.response_types,
        scope: client.scopes.join(" "),
        token_endpoint_auth_method: client.token_endpoint_auth_method,
        application_type: client.application_type,
        client_uri: client.client_uri,
        logo_uri: client.logo_uri,
        contacts: client.contacts,
        client_secret_expires_at: 0,
        client_id_issued_at: client.created_at,
        registration_access_token: token,
        registration_client_uri: format!("{base_url}/api/v1/core/oauth/register/{client_id}"),
    };

    Ok((StatusCode::OK, Json(response)).into_response())
}
