//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{TokenError, TokenResult};
use anyhow::Result;
use systemprompt_identifiers::{AuthorizationCode, ClientId};
use systemprompt_oauth::repository::{AuthCodeValidationResult, OAuthRepository};
use systemprompt_oauth::services::validation::validate_client_credentials as validate_client_credentials_shared;

pub fn extract_required_field<'a>(
    field: Option<&'a str>,
    field_name: &str,
) -> TokenResult<&'a str> {
    field.ok_or_else(|| TokenError::InvalidRequest {
        field: field_name.to_owned(),
        message: "is required".to_owned(),
    })
}

pub async fn validate_client_credentials(
    repo: &OAuthRepository,
    client_id: &ClientId,
    client_secret: Option<&str>,
) -> Result<()> {
    validate_client_credentials_shared(repo, client_id, client_secret)
        .await
        .map_err(Into::into)
}

#[derive(Debug)]
pub struct AuthCodeValidationParams<'a> {
    pub repo: &'a OAuthRepository,
    pub code: &'a AuthorizationCode,
    pub client_id: &'a ClientId,
    pub redirect_uri: Option<&'a str>,
    pub code_verifier: Option<&'a str>,
    pub request_resource: Option<&'a str>,
}

pub async fn validate_authorization_code(
    params: AuthCodeValidationParams<'_>,
) -> Result<AuthCodeValidationResult> {
    let result = params
        .repo
        .validate_authorization_code(
            params.code,
            params.client_id,
            params.redirect_uri,
            params.code_verifier,
        )
        .await?;

    if let Some(req_resource) = params.request_resource
        && let Some(ref stored_resource) = result.resource
        && req_resource != stored_resource
    {
        return Err(anyhow::anyhow!(
            "Resource parameter mismatch: expected '{}', got '{}'",
            stored_resource,
            req_resource
        ));
    }

    Ok(result)
}
