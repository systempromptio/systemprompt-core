use super::{TokenError, TokenResult};
use anyhow::Result;
use systemprompt_identifiers::{AuthorizationCode, ClientId, UserId};
use systemprompt_oauth::repository::OAuthRepository;
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

pub async fn validate_authorization_code(
    repo: &OAuthRepository,
    code: &AuthorizationCode,
    client_id: &ClientId,
    redirect_uri: Option<&str>,
    code_verifier: Option<&str>,
) -> Result<(UserId, String)> {
    let (user_id, scope) = repo
        .validate_authorization_code(code, client_id, redirect_uri, code_verifier)
        .await?;
    Ok((user_id, scope))
}
