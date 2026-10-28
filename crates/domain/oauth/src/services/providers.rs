//! JWT validation provider implementations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_identifiers::{AccessTokenId, UserId};
use systemprompt_models::auth::{
    AuthenticatedUser, JwtAudience, Permission, TokenAuthority, UserJwtRequest,
};
use systemprompt_traits::{
    AgentJwtClaims, GenerateTokenParams, JwtProviderError, JwtResult, JwtValidationProvider,
};

use crate::error::OauthError;

use super::generation::{JwtConfig, generate_secure_token};
use super::validation::jwt::validate_jwt_token;

#[derive(Debug)]
pub struct JwtValidationProviderImpl {
    authority: Arc<dyn TokenAuthority>,
    audiences: Vec<JwtAudience>,
}

impl JwtValidationProviderImpl {
    #[must_use]
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

impl JwtValidationProvider for JwtValidationProviderImpl {
    fn validate_token(&self, token: &str) -> JwtResult<AgentJwtClaims> {
        let claims = validate_jwt_token(self.authority.as_ref(), token, &self.audiences).map_err(
            |e| match e {
                OauthError::Expired(_) => JwtProviderError::TokenExpired,
                _ => JwtProviderError::InvalidToken,
            },
        )?;

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
            interaction: None,
        };

        let request = UserJwtRequest {
            user,
            session_id: params.session_id,
            jti: AccessTokenId::new(generate_secure_token("jwt")),
            permissions: config.permissions,
            audience: config.audience,
            ttl: config.expires_in,
            client_id: None,
            purpose: None,
            plugin_id: None,
            interaction: None,
        };
        self.authority
            .mint_user_jwt(&request)
            .map_err(|e| JwtProviderError::Internal(e.into()))
    }

    fn generate_secure_token(&self, prefix: &str) -> String {
        generate_secure_token(prefix)
    }
}
