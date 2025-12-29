use crate::TokenValidator;
use async_trait::async_trait;
use systemprompt_models::auth::{AuthError, AuthenticatedUser, JwtAudience};
use uuid::Uuid;

use crate::services::validation::jwt;

#[derive(Clone, Debug)]
pub struct JwtTokenValidator {
    secret: String,
    issuer: String,
    audiences: Vec<JwtAudience>,
}

impl JwtTokenValidator {
    pub const fn new(secret: String, issuer: String, audiences: Vec<JwtAudience>) -> Self {
        Self {
            secret,
            issuer,
            audiences,
        }
    }

    pub fn from_config() -> Result<Self, AuthError> {
        let config =
            systemprompt_models::Config::get().map_err(|e| AuthError::AuthenticationFailed {
                message: format!("Failed to get config: {e}"),
            })?;
        let secret = systemprompt_models::SecretsBootstrap::jwt_secret()
            .map_err(|e| AuthError::AuthenticationFailed {
                message: format!("Failed to get JWT secret: {e}"),
            })?
            .to_string();
        Ok(Self {
            secret,
            issuer: config.jwt_issuer.clone(),
            audiences: config.jwt_audiences.clone(),
        })
    }
}

#[async_trait]
impl TokenValidator for JwtTokenValidator {
    async fn validate_token(&self, token: &str) -> Result<AuthenticatedUser, AuthError> {
        let claims = jwt::validate_jwt_token(token, &self.secret, &self.issuer, &self.audiences)
            .map_err(|e| AuthError::AuthenticationFailed {
                message: format!("JWT validation failed: {e}"),
            })?;

        let user_id =
            Uuid::parse_str(&claims.sub).map_err(|e| AuthError::AuthenticationFailed {
                message: format!("Invalid user ID in token: {e}"),
            })?;

        let permissions = claims.get_permissions();

        Ok(AuthenticatedUser::new(
            user_id,
            claims.username.clone(),
            Some(claims.email),
            permissions,
        ))
    }
}
