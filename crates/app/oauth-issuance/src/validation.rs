//! Token-request validation: required fields and authorization-code
//! redemption.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AuthorizationCode, ClientId, RefreshTokenId};
use systemprompt_manifest::Config;
use systemprompt_oauth::OauthError;
use systemprompt_oauth::repository::{
    AuthCodeRedemption, AuthCodeValidationResult, OAuthRepository,
};

use crate::resource::{ResourceBinding, resolve_grant_resource};
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
    pub refresh_token_id: &'a RefreshTokenId,
    pub config: &'a Config,
}

pub async fn validate_authorization_code(
    params: AuthCodeValidationParams<'_>,
) -> IssuanceResult<AuthCodeValidationResult> {
    let redirect_uri = extract_required_field(params.redirect_uri, "redirect_uri")?;
    let code_verifier = extract_required_field(params.code_verifier, "code_verifier")?;
    let result = params
        .repo
        .validate_authorization_code(AuthCodeRedemption {
            code: params.code,
            client_id: params.client_id,
            redirect_uri,
            code_verifier,
            refresh_token_id: params.refresh_token_id,
        })
        .await
        .map_err(authorization_code_error)?;

    resolve_grant_resource(
        params.request_resource,
        ResourceBinding::Bound(result.resource.as_deref()),
        params.config,
    )?;

    Ok(result)
}

pub(crate) fn authorization_code_error(error: OauthError) -> IssuanceError {
    match error {
        OauthError::Validation(reason) => IssuanceError::InvalidGrant { reason },
        other => IssuanceError::Oauth(other),
    }
}
