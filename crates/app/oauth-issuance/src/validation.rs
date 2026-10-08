//! Token-request validation: required fields and authorization-code
//! redemption.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AuthorizationCode, ClientId};
use systemprompt_oauth::OauthError;
use systemprompt_oauth::repository::{AuthCodeValidationResult, OAuthRepository};

use crate::{IssuanceError, IssuanceResult};

pub fn extract_required_field<'a>(
    field: Option<&'a str>,
    field_name: &str,
) -> IssuanceResult<&'a str> {
    field.ok_or_else(|| IssuanceError::InvalidRequest {
        field: field_name.to_owned(),
        message: "is required".to_owned(),
    })
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
) -> IssuanceResult<AuthCodeValidationResult> {
    let redirect_uri = extract_required_field(params.redirect_uri, "redirect_uri")?;
    let code_verifier = extract_required_field(params.code_verifier, "code_verifier")?;
    let result = params
        .repo
        .validate_authorization_code(params.code, params.client_id, redirect_uri, code_verifier)
        .await
        .map_err(authorization_code_error)?;

    if let Some(req_resource) = params.request_resource
        && let Some(ref stored_resource) = result.resource
        && req_resource != stored_resource
    {
        return Err(IssuanceError::InvalidGrant {
            reason: format!(
                "Resource parameter mismatch: expected '{stored_resource}', got '{req_resource}'"
            ),
        });
    }

    Ok(result)
}

fn authorization_code_error(error: OauthError) -> IssuanceError {
    match error {
        OauthError::Validation(reason) => IssuanceError::InvalidGrant { reason },
        other => IssuanceError::Oauth(other),
    }
}
