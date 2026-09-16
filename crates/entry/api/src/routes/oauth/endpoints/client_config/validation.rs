//! Client-configuration request authentication (RFC 7592 §2.1).
//!
//! The registration access token issued at registration is the sole
//! credential: it is compared against the hash stored with the client. A
//! client registered without one (admin-provisioned) has no configuration
//! endpoint. Unknown and unauthenticated clients are indistinguishable to the
//! caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_identifiers::ClientId;
use systemprompt_oauth::models::OAuthClient;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::{REGISTRATION_TOKEN_PREFIX, verify_registration_token};

use crate::routes::oauth::OAuthHttpError;

pub fn validate_registration_token(headers: &HeaderMap) -> Result<String, OAuthHttpError> {
    let auth_header = headers
        .get("authorization")
        .ok_or_else(|| OAuthHttpError::invalid_token("Missing authorization header"))?
        .to_str()
        .map_err(|_e| OAuthHttpError::invalid_token("Invalid authorization header format"))?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        OAuthHttpError::invalid_token("Authorization header must use Bearer scheme")
    })?;

    if !token.starts_with(REGISTRATION_TOKEN_PREFIX) {
        return Err(OAuthHttpError::invalid_token(
            "Invalid registration access token format",
        ));
    }

    Ok(token.to_owned())
}

pub async fn authenticate_client_configuration(
    repository: &OAuthRepository,
    headers: &HeaderMap,
    client_id: &ClientId,
) -> Result<(OAuthClient, String), OAuthHttpError> {
    let token = validate_registration_token(headers)?;

    let stored_hash = repository.find_registration_token_hash(client_id).await?;
    if !verify_registration_token(&token, stored_hash.as_deref()) {
        return Err(OAuthHttpError::invalid_token(
            "Registration access token does not match this client",
        ));
    }

    let client = repository
        .find_client_by_id(client_id)
        .await?
        .ok_or_else(|| OAuthHttpError::invalid_token("Client not found"))?;

    Ok((client, token))
}
