//! JWT validation provider implementations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AccessTokenId, UserId};
use systemprompt_models::auth::{AuthenticatedUser, JwtAudience, Permission};
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{
    AgentJwtClaims, GenerateTokenParams, JwtProviderError, JwtResult, JwtValidationProvider,
};

use crate::error::OauthError;

use super::generation::{JwtConfig, JwtSigningParams, generate_jwt, generate_secure_token};
use super::validation::jwt::validate_jwt_token;

#[derive(Debug)]
pub struct JwtValidationProviderImpl {
    issuer: String,
    audiences: Vec<JwtAudience>,
    keys: SigningKeys,
}

impl JwtValidationProviderImpl {
    #[must_use]
    pub const fn new(issuer: String, audiences: Vec<JwtAudience>, keys: SigningKeys) -> Self {
        Self {
            issuer,
            audiences,
            keys,
        }
    }

    pub fn from_config(config: &systemprompt_manifest::Config, keys: SigningKeys) -> Self {
        Self {
            issuer: config.jwt_issuer.clone(),
            audiences: config.jwt_audiences.clone(),
            keys,
        }
    }
}

impl JwtValidationProvider for JwtValidationProviderImpl {
    fn validate_token(&self, token: &str) -> JwtResult<AgentJwtClaims> {
        let claims =
            validate_jwt_token(&self.keys, token, &self.issuer, &self.audiences).map_err(|e| {
                match e {
                    OauthError::Expired(_) => JwtProviderError::TokenExpired,
                    _ => JwtProviderError::InvalidToken,
                }
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

    fn generate_token(&self, params: GenerateTokenParams) -> JwtResult<String> {
        let user = AuthenticatedUser {
            id: params.user_id.clone(),
            username: params.username.clone(),
            email: params.username.clone(),
            roles: vec![],
            permissions: vec![],
            attributes: std::collections::BTreeMap::new(),
            account_kind: systemprompt_models::auth::AccountKind::Person,
        };

        let permissions = params
            .permissions
            .iter()
            .map(|p| {
                p.parse::<Permission>().map_err(|_e| {
                    JwtProviderError::Internal(Box::new(OauthError::Validation(format!(
                        "unknown permission {p:?}"
                    ))))
                })
            })
            .collect::<JwtResult<Vec<_>>>()?;

        let audiences = params
            .audiences
            .iter()
            .map(|a| match a.parse::<JwtAudience>() {
                Ok(JwtAudience::Resource(_)) | Err(_) => {
                    Err(JwtProviderError::MissingAudience(a.clone()))
                },
                Ok(audience) => Ok(audience),
            })
            .collect::<JwtResult<Vec<_>>>()?;
        if audiences.is_empty() {
            return Err(JwtProviderError::MissingAudience(
                "at least one audience is required".to_owned(),
            ));
        }

        let config = JwtConfig {
            permissions,
            audience: audiences,
            expires_in: params.expires_in_hours.map_or_else(
                || JwtConfig::default().expires_in,
                |hours| chrono::Duration::hours(i64::from(hours)),
            ),
            resource: None,
            plugin_id: None,
            client_id: None,
            purpose: None,
        };

        let jti = AccessTokenId::new(generate_secure_token("jwt"));
        let signing = JwtSigningParams {
            issuer: &self.issuer,
            keys: &self.keys,
        };

        generate_jwt(&user, config, jti, &params.session_id, &signing)
            .map_err(|e| JwtProviderError::Internal(e.into()))
    }

    fn generate_secure_token(&self, prefix: &str) -> String {
        generate_secure_token(prefix)
    }
}
