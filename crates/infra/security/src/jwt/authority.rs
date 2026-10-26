//! [`DeploymentTokenAuthority`]: the deployment's [`TokenAuthority`].
//!
//! Binds the [`SigningKeys`] to the deployment's issuer and routes every
//! operation through the existing primitives: [`decode_rs256_claims`] and
//! [`decode_session_claims`] to verify, [`encode_rs256`] to sign. It adds no
//! second key-resolution path; the keys are the ones the composition root
//! resolved once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Duration, Utc};
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, Header, encode};
use serde::Serialize;
use systemprompt_models::auth::{
    CustomClaims, JwtAudience, JwtClaims, RegisteredClaims, TokenAuthority, TokenError,
    TokenResult, TokenType, UserJwtRequest, UserType,
};

use super::validate::{ValidationPolicy, decode_rs256_claims, decode_session_claims};
use crate::error::AuthError;
use crate::keys::SigningKeys;

const MAX_TOKEN_LIFETIME: Duration = Duration::days(365);

#[derive(Debug, Clone)]
pub struct DeploymentTokenAuthority {
    keys: SigningKeys,
    issuer: String,
}

impl DeploymentTokenAuthority {
    #[must_use]
    pub fn new(keys: SigningKeys, issuer: impl Into<String>) -> Self {
        Self {
            keys,
            issuer: issuer.into(),
        }
    }

    #[must_use]
    pub fn issuer(&self) -> &str {
        &self.issuer
    }
}

pub fn encode_rs256<C: Serialize>(
    keys: &SigningKeys,
    claims: &C,
) -> Result<String, jsonwebtoken::errors::Error> {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(keys.active_kid().to_owned());
    encode(&header, claims, keys.encoding_key())
}

pub fn user_claims(issuer: &str, req: &UserJwtRequest) -> TokenResult<JwtClaims> {
    if req.ttl <= Duration::zero() || req.ttl > MAX_TOKEN_LIFETIME {
        return Err(TokenError::InvalidLifetime {
            seconds: req.ttl.num_seconds(),
        });
    }
    let now = Utc::now().timestamp();
    let user_type = UserType::from_permissions(&req.permissions);
    Ok(JwtClaims {
        registered: RegisteredClaims {
            sub: req.user.id.to_string(),
            iat: now,
            exp: now.saturating_add(req.ttl.num_seconds()),
            nbf: Some(now),
            iss: issuer.to_owned(),
            aud: req.audience.clone(),
            jti: req.jti.to_string(),
        },
        custom: CustomClaims {
            scope: req.permissions.clone(),
            username: req.user.username.clone(),
            email: req.user.email.clone(),
            user_type,
            roles: req.user.roles().to_vec(),
            attributes: req.user.attributes().clone(),
            client_id: req.client_id.clone(),
            token_type: TokenType::Bearer,
            auth_time: now,
            session_id: Some(req.session_id.clone()),
            rate_limit_tier: Some(user_type.rate_tier()),
            plugin_id: req.plugin_id.clone(),
            act: None,
            purpose: req.purpose.clone(),
        },
    })
}

impl From<AuthError> for TokenError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::UnknownKid(kid) => Self::UnknownKid(kid),
            AuthError::MissingKid => Self::KidMismatch,
            AuthError::InvalidToken(inner) => match inner.kind() {
                ErrorKind::ExpiredSignature => Self::Expired,
                ErrorKind::InvalidSignature => Self::InvalidSignature,
                ErrorKind::InvalidAudience => Self::InvalidAudience,
                ErrorKind::InvalidIssuer => Self::InvalidIssuer,
                _ => Self::Malformed(Box::new(inner)),
            },
            AuthError::UnsupportedAlgorithm { got } => Self::UnsupportedAlgorithm { got },
            other => Self::Rejected(Box::new(other)),
        }
    }
}

impl TokenAuthority for DeploymentTokenAuthority {
    fn verify_session_cookie(&self, token: &str) -> TokenResult<JwtClaims> {
        let policy = ValidationPolicy::session_context(&self.issuer);
        Ok(decode_session_claims(&self.keys, token, &policy)?)
    }

    fn verify_bearer(&self, token: &str, audiences: &[JwtAudience]) -> TokenResult<JwtClaims> {
        let policy = ValidationPolicy::issuer_scoped(&self.issuer, audiences);
        Ok(decode_rs256_claims(&self.keys, token, &policy)?)
    }

    fn verify_hook_token(&self, token: &str) -> TokenResult<JwtClaims> {
        let policy = ValidationPolicy::issuer_scoped(&self.issuer, &[JwtAudience::Hook]);
        Ok(decode_rs256_claims(&self.keys, token, &policy)?)
    }

    fn sign_claims(&self, claims: &serde_json::Value) -> TokenResult<String> {
        encode_rs256(&self.keys, claims).map_err(|e| TokenError::Signing(Box::new(e)))
    }

    fn mint_user_jwt(&self, req: &UserJwtRequest) -> TokenResult<String> {
        let claims = user_claims(&self.issuer, req)?;
        encode_rs256(&self.keys, &claims).map_err(|e| TokenError::Signing(Box::new(e)))
    }
}
