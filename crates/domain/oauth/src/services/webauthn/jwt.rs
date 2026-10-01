//! WebAuthn-issued JWT validator.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::TokenValidator;
use crate::error::OauthError;
use systemprompt_models::auth::{AuthError, AuthenticatedUser, JwtAudience};
use uuid::Uuid;

use crate::services::validation::jwt;

#[derive(Clone, Debug)]
pub struct JwtTokenValidator {
    issuer: String,
    audiences: Vec<JwtAudience>,
}

impl JwtTokenValidator {
    pub const fn new(issuer: String, audiences: Vec<JwtAudience>) -> Self {
        Self { issuer, audiences }
    }

    pub fn from_config() -> Result<Self, AuthError> {
        let config = systemprompt_models::Config::get().map_err(|error| {
            tracing::error!(%error, "JWT validator could not read the configuration");
            AuthError::AuthenticationFailed {
                message: "token validator is not configured".to_owned(),
            }
        })?;
        Ok(Self {
            issuer: config.jwt_issuer.clone(),
            audiences: config.jwt_audiences.clone(),
        })
    }
}

impl TokenValidator for JwtTokenValidator {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "async signature required by the TokenValidator trait; this \
                  validator decodes the JWT synchronously"
    )]
    async fn validate_token(&self, token: &str) -> Result<AuthenticatedUser, AuthError> {
        let claims =
            jwt::validate_jwt_token(token, &self.issuer, &self.audiences).map_err(|error| {
                match error {
                    OauthError::Expired(_) => AuthError::TokenExpired,
                    other => {
                        tracing::debug!(error = %other, "JWT validation failed");
                        AuthError::AuthenticationFailed {
                            message: "JWT validation failed".to_owned(),
                        }
                    },
                }
            })?;

        let user_id =
            Uuid::parse_str(&claims.sub).map_err(|_invalid| AuthError::InvalidTokenFormat)?;

        let permissions = claims.get_permissions();
        let roles = claims.roles().to_vec();

        Ok(AuthenticatedUser::new_with_roles(
            user_id,
            claims.username.clone(),
            claims.email,
            permissions,
            roles,
        ))
    }
}
