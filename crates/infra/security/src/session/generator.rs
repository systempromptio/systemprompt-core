//! Session id and JWT minting for authenticated sessions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Duration, Utc};
use std::collections::BTreeMap;
use systemprompt_identifiers::{SessionId, SessionToken, UserId};
use systemprompt_models::auth::{
    CustomClaims, JwtAudience, JwtClaims, Permission, RateLimitTier, RegisteredClaims, TokenType,
    UserType,
};

use crate::error::{JwtError, JwtResult};
use crate::jwt::encode_rs256;
use crate::keys::SigningKeys;

#[derive(Debug)]
pub struct SessionParams<'a> {
    pub user_id: &'a UserId,
    pub session_id: &'a SessionId,
    pub email: &'a str,
    pub duration: Duration,
    pub user_type: UserType,
    pub permissions: Vec<Permission>,
    pub roles: Vec<String>,
    // JSON: ABAC attribute bag — JWT claim values are policy-defined and schema-less.
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub rate_limit_tier: RateLimitTier,
}

#[derive(Debug)]
pub struct SessionGenerator {
    issuer: String,
    keys: SigningKeys,
}

impl SessionGenerator {
    pub fn new(issuer: impl Into<String>, keys: SigningKeys) -> Self {
        Self {
            issuer: issuer.into(),
            keys,
        }
    }

    pub fn generate(&self, params: &SessionParams<'_>) -> JwtResult<SessionToken> {
        let now = Utc::now();
        let expiry = now + params.duration;

        let claims = JwtClaims {
            registered: RegisteredClaims {
                sub: params.user_id.to_string(),
                iat: now.timestamp(),
                exp: expiry.timestamp(),
                nbf: Some(now.timestamp()),
                iss: self.issuer.clone(),
                aud: JwtAudience::standard(),
                jti: uuid::Uuid::new_v4().to_string(),
            },
            custom: CustomClaims {
                scope: params.permissions.clone(),
                username: params.email.to_owned(),
                email: params.email.to_owned(),
                user_type: params.user_type,
                roles: params.roles.clone(),
                attributes: params.attributes.clone(),
                client_id: None,
                token_type: TokenType::Bearer,
                auth_time: now.timestamp(),
                session_id: Some(params.session_id.clone()),
                rate_limit_tier: Some(params.rate_limit_tier),
                plugin_id: None,
                act: None,
                purpose: None,
                interaction: None,
            },
        };

        let token = encode_rs256(&self.keys, &claims).map_err(JwtError::from)?;

        Ok(SessionToken::new(token))
    }
}
