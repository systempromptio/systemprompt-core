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
        field: field_name.to_string(),
        message: "is required".to_string(),
    })
}

pub async fn validate_client_credentials(
    repo: &OAuthRepository,
    client_id: &ClientId,
    client_secret: Option<&str>,
) -> Result<()> {
    validate_client_credentials_shared(repo, client_id.as_str(), client_secret).await
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

    if let Some(req_resource) = params.request_resource {
        if let Some(ref stored_resource) = result.resource {
            if req_resource != stored_resource {
                return Err(anyhow::anyhow!(
                    "Resource parameter mismatch: expected '{}', got '{}'",
                    stored_resource,
                    req_resource
                ));
            }
        }
    }

    Ok(result)
}
