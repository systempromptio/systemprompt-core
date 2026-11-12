//! JWT validation provider: verifies a bearer against the deployment's
//! public verification key set. It holds no signing key and mints nothing, so
//! a spawned agent runs it on the key set the API handed it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use jsonwebtoken::errors::ErrorKind;
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::JwtAudience;
use systemprompt_security::AuthError;
use systemprompt_security::jwt::{ValidationPolicy, decode_rs256_claims};
use systemprompt_security::keys::VerificationKeyring;
use systemprompt_traits::{AgentJwtClaims, JwtProviderError, JwtResult, JwtValidationProvider};

use super::generation::generate_secure_token;

#[derive(Debug)]
pub struct JwtValidationProviderImpl {
    keys: VerificationKeyring,
    issuer: String,
    audiences: Vec<JwtAudience>,
}

impl JwtValidationProviderImpl {
    #[must_use]
    pub const fn new(
        keys: VerificationKeyring,
        issuer: String,
        audiences: Vec<JwtAudience>,
    ) -> Self {
        Self {
            keys,
            issuer,
            audiences,
        }
    }

    #[must_use]
    pub fn from_config(config: &systemprompt_manifest::Config, keys: VerificationKeyring) -> Self {
        Self::new(
            keys,
            config.jwt_issuer.clone(),
            config.jwt_audiences.clone(),
        )
    }
}

impl JwtValidationProvider for JwtValidationProviderImpl {
    fn validate_token(&self, token: &str) -> JwtResult<AgentJwtClaims> {
        let policy = ValidationPolicy::issuer_scoped(&self.issuer, &self.audiences);
        let claims = decode_rs256_claims(&self.keys, token, &policy).map_err(|e| match e {
            AuthError::InvalidToken(inner)
                if matches!(inner.kind(), ErrorKind::ExpiredSignature) =>
            {
                JwtProviderError::TokenExpired
            },
            _ => JwtProviderError::InvalidToken,
        })?;

        let is_admin = claims.is_admin();
        let subject =
            UserId::try_new(claims.registered.sub).map_err(|_e| JwtProviderError::InvalidToken)?;
        Ok(AgentJwtClaims {
            subject,
            username: claims.custom.username,
            user_type: claims.custom.user_type.to_string(),
            audiences: claims
                .registered
                .aud
                .iter()
                .map(ToString::to_string)
                .collect(),
            permissions: claims
                .custom
                .scope
                .iter()
                .map(ToString::to_string)
                .collect(),
            is_admin,
            expires_at: claims.registered.exp,
            issued_at: claims.registered.iat,
        })
    }

    fn generate_secure_token(&self, prefix: &str) -> String {
        generate_secure_token(prefix)
    }
}
