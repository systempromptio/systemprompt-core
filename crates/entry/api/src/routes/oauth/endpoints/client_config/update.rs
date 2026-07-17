//! RFC 7592 client-configuration update endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use systemprompt_models::Config;

use super::validation::validate_registration_token;
use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;
use systemprompt_oauth::oauth::dynamic_registration::{
    DynamicRegistrationRequest, DynamicRegistrationResponse,
};

pub async fn update_client_configuration(
    OAuthRepo(repository): OAuthRepo,
    Path(client_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<DynamicRegistrationRequest>,
) -> Result<Response, OAuthHttpError> {
    let registration_token = validate_registration_token(&headers)?;

    let client_id = systemprompt_identifiers::ClientId::new(&client_id);
    let existing_client = repository
        .find_client_by_id(&client_id)
        .await?
        .ok_or_else(|| OAuthHttpError::invalid_client_metadata("Client not found"))?;

    let client_name = request
        .get_client_name()
        .map_err(|e| OAuthHttpError::invalid_client_metadata(e.to_string()))?;
    let mut redirect_uris = request
        .get_redirect_uris()
        .map_err(|e| OAuthHttpError::invalid_client_metadata(e.to_string()))?;
    redirect_uris.sort();
    redirect_uris.dedup();

    repository
        .update_client(
            &client_id,
            Some(&client_name),
            Some(&redirect_uris),
            Some(&existing_client.scopes),
        )
        .await
        .map_err(|e| {
            OAuthHttpError::invalid_client_metadata(format!("Failed to update client: {e}"))
        })?;

    let base_url = Config::get()
        .map_err(|e| OAuthHttpError::server_error(format!("Configuration unavailable: {e}")))?
        .api_server_url
        .clone();

    let response = DynamicRegistrationResponse {
        client_id: client_id.clone(),
        client_secret: "***REDACTED***".to_owned(),
        client_name,
        redirect_uris,
        grant_types: existing_client.grant_types,
        response_types: existing_client.response_types,
        scope: existing_client.scopes.join(" "),
        token_endpoint_auth_method: existing_client.token_endpoint_auth_method,
        application_type: existing_client.application_type,
        client_uri: request.client_uri,
        logo_uri: request.logo_uri,
        contacts: request.contacts,
        client_secret_expires_at: 0,
        client_id_issued_at: existing_client.created_at,
        registration_access_token: registration_token,
        registration_client_uri: format!("{base_url}/api/v1/core/oauth/register/{client_id}"),
    };

    Ok((StatusCode::OK, Json(response)).into_response())
}
