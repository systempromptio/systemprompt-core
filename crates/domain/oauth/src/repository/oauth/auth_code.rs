use super::OAuthRepository;
use crate::models::PkceMethod;
use anyhow::Result;
use base64::Engine;
use chrono::Utc;
use systemprompt_identifiers::{AuthorizationCode, ClientId, UserId};

#[derive(Debug)]
pub struct AuthCodeParams<'a> {
    pub code: &'a AuthorizationCode,
    pub client_id: &'a ClientId,
    pub user_id: &'a UserId,
    pub redirect_uri: &'a str,
    pub scope: &'a str,
    pub code_challenge: Option<&'a str>,
    pub code_challenge_method: Option<&'a str>,
    pub resource: Option<&'a str>,
}

#[derive(Debug)]
pub struct AuthCodeParamsBuilder<'a> {
    code: &'a AuthorizationCode,
    client_id: &'a ClientId,
    user_id: &'a UserId,
    redirect_uri: &'a str,
    scope: &'a str,
    code_challenge: Option<&'a str>,
    code_challenge_method: Option<&'a str>,
    resource: Option<&'a str>,
}

impl<'a> AuthCodeParamsBuilder<'a> {
    pub const fn new(
        code: &'a AuthorizationCode,
        client_id: &'a ClientId,
        user_id: &'a UserId,
        redirect_uri: &'a str,
        scope: &'a str,
    ) -> Self {
        Self {
            code,
            client_id,
            user_id,
            redirect_uri,
            scope,
            code_challenge: None,
            code_challenge_method: None,
            resource: None,
        }
    }

    pub const fn with_pkce(mut self, challenge: &'a str, method: &'a str) -> Self {
        self.code_challenge = Some(challenge);
        self.code_challenge_method = Some(method);
        self
    }

    pub const fn with_resource(mut self, resource: &'a str) -> Self {
        self.resource = Some(resource);
        self
    }

    pub const fn build(self) -> AuthCodeParams<'a> {
        AuthCodeParams {
            code: self.code,
            client_id: self.client_id,
            user_id: self.user_id,
            redirect_uri: self.redirect_uri,
            scope: self.scope,
            code_challenge: self.code_challenge,
            code_challenge_method: self.code_challenge_method,
            resource: self.resource,
        }
    }
}

impl<'a> AuthCodeParams<'a> {
    pub const fn builder(
        code: &'a AuthorizationCode,
        client_id: &'a ClientId,
        user_id: &'a UserId,
        redirect_uri: &'a str,
        scope: &'a str,
    ) -> AuthCodeParamsBuilder<'a> {
        AuthCodeParamsBuilder::new(code, client_id, user_id, redirect_uri, scope)
    }
}

impl OAuthRepository {
    pub async fn store_authorization_code(&self, params: AuthCodeParams<'_>) -> Result<()> {
        let expires_at = Utc::now() + chrono::Duration::seconds(600);
        let now = Utc::now();
        let code = params.code.as_str();
        let client_id = params.client_id.as_str();
        let user_id = params.user_id.as_str();

        sqlx::query!(
            "INSERT INTO oauth_auth_codes
             (code, client_id, user_id, redirect_uri, scope, expires_at, code_challenge,
             code_challenge_method, resource, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            code,
            client_id,
            user_id,
            params.redirect_uri,
            params.scope,
            expires_at,
            params.code_challenge,
            params.code_challenge_method,
            params.resource,
            now
        )
        .execute(self.write_pool_ref())
        .await?;

        Ok(())
    }

    pub async fn get_client_id_from_auth_code(
        &self,
        code: &AuthorizationCode,
    ) -> Result<Option<ClientId>> {
        let code_str = code.as_str();
        let result = sqlx::query_scalar!(
            "SELECT client_id FROM oauth_auth_codes WHERE code = $1",
            code_str
        )
        .fetch_optional(self.pool_ref())
        .await?;

        Ok(result.map(ClientId::new))
    }

    pub async fn validate_authorization_code(
        &self,
        code: &AuthorizationCode,
        _client_id: &ClientId,
        redirect_uri: Option<&str>,
        code_verifier: Option<&str>,
    ) -> Result<AuthCodeValidationResult> {
        let now = Utc::now();
        let code_str = code.as_str();

        let row = sqlx::query!(
            "SELECT user_id, scope, expires_at, redirect_uri, used_at, code_challenge,
             code_challenge_method, resource
             FROM oauth_auth_codes WHERE code = $1",
            code_str
        )
        .fetch_optional(self.pool_ref())
        .await?
        .ok_or_else(|| anyhow::anyhow!("Invalid authorization code"))?;

        if row.used_at.is_some() {
            return Err(anyhow::anyhow!("Authorization code already used"));
        }

        if row.expires_at < now {
            return Err(anyhow::anyhow!("Authorization code expired"));
        }

        if let Some(expected_uri) = redirect_uri {
            if row.redirect_uri != expected_uri {
                return Err(anyhow::anyhow!("Redirect URI mismatch"));
            }
        }

        if let Some(ref challenge) = row.code_challenge {
            let verifier =
                code_verifier.ok_or_else(|| anyhow::anyhow!("code_verifier required for PKCE"))?;

            let method = row
                .code_challenge_method
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("code_challenge_method required for PKCE"))?;

            let computed_challenge = match method
                .parse::<PkceMethod>()
                .map_err(|e| {
                    tracing::debug!(method = %method, error = %e, "Failed to parse PKCE method");
                    e
                })
                .ok()
            {
                Some(PkceMethod::S256) => {
                    use sha2::{Digest, Sha256};
                    let mut hasher = Sha256::new();
                    hasher.update(verifier.as_bytes());
                    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
                },
                Some(PkceMethod::Plain) => {
                    return Err(anyhow::anyhow!(
                        "PKCE method 'plain' is not allowed. Only 'S256' is supported for \
                         security."
                    ));
                },
                None => {
                    return Err(anyhow::anyhow!(
                        "Unsupported code_challenge_method: {method}. Only 'S256' is allowed."
                    ))
                },
            };

            if computed_challenge != *challenge {
                return Err(anyhow::anyhow!("PKCE validation failed"));
            }
        }

        sqlx::query!(
            "UPDATE oauth_auth_codes SET used_at = $1 WHERE code = $2",
            now,
            code_str
        )
        .execute(self.write_pool_ref())
        .await?;

        Ok(AuthCodeValidationResult {
            user_id: UserId::new(row.user_id),
            scope: row.scope,
            resource: row.resource,
        })
    }
}

#[derive(Debug)]
pub struct AuthCodeValidationResult {
    pub user_id: UserId,
    pub scope: String,
    pub resource: Option<String>,
}
