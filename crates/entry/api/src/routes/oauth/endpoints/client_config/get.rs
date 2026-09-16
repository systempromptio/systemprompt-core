//! RFC 7592 client-configuration read endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use systemprompt_models::Config;

use super::validation::authenticate_client_configuration;
use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;
use systemprompt_oauth::oauth::dynamic_registration::DynamicRegistrationResponse;

pub async fn get_client_configuration(
    OAuthRepo(repository): OAuthRepo,
    Path(client_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, OAuthHttpError> {
    let client_id = systemprompt_identifiers::ClientId::new(&client_id);
    let (client, token) =
        authenticate_client_configuration(&repository, &headers, &client_id).await?;

    let base_url = Config::get()?.api_server_url.clone();

    let response = DynamicRegistrationResponse {
        client_id: client.client_id.clone(),
        client_secret: None,
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
        client_secret_expires_at: None,
        client_id_issued_at: client.created_at,
        registration_access_token: token,
        registration_client_uri: format!("{base_url}/api/v1/core/oauth/register/{client_id}"),
    };

    Ok((StatusCode::OK, Json(response)).into_response())
}
