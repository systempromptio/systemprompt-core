//! WebAuthn-issued JWT validator.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::TokenValidator;
use crate::error::OauthError;
use std::sync::Arc;
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::{AuthRequestError, AuthenticatedUser, JwtAudience, TokenAuthority};

use crate::services::validation::jwt;

#[derive(Clone, Debug)]
pub struct JwtTokenValidator {
    authority: Arc<dyn TokenAuthority>,
    audiences: Vec<JwtAudience>,
}

impl JwtTokenValidator {
    pub const fn new(authority: Arc<dyn TokenAuthority>, audiences: Vec<JwtAudience>) -> Self {
        Self {
            authority,
            audiences,
        }
    }

    pub fn from_config(
        config: &systemprompt_manifest::Config,
        authority: Arc<dyn TokenAuthority>,
    ) -> Self {
        Self {
            authority,
            audiences: config.jwt_audiences.clone(),
        }
    }
}

impl TokenValidator for JwtTokenValidator {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "async signature required by the TokenValidator trait; this \
                  validator decodes the JWT synchronously"
    )]
    async fn validate_token(&self, token: &str) -> Result<AuthenticatedUser, AuthRequestError> {
        let claims = jwt::validate_jwt_token(self.authority.as_ref(), token, &self.audiences)
            .map_err(|error| match error {
                OauthError::Expired(_) => AuthRequestError::TokenExpired,
                other => {
                    tracing::debug!(error = %other, "JWT validation failed");
                    AuthRequestError::AuthenticationFailed {
                        message: "JWT validation failed".to_owned(),
                    }
                },
            })?;

        let user_id = UserId::try_new(claims.registered.sub.as_str())
            .map_err(|_invalid| AuthRequestError::InvalidTokenFormat)?;

        let permissions = claims.get_permissions();
        let roles = claims.roles().to_vec();

        Ok(AuthenticatedUser::new_with_roles(
            user_id,
            claims.custom.username.clone(),
            claims.custom.email,
            permissions,
            roles,
        ))
    }
}
