//! JWT and client-secret generation primitives.
//!
//! The `purpose` claim carries the caller purpose. Only in-process issuers set
//! it; no endpoint copies it from a request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::OauthResult as Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, Header, encode};
use serde::{Deserialize, Serialize};

use crate::models::JwtClaims;
use systemprompt_identifiers::{AccessTokenId, ClientId, PluginId, SessionId, UserId};
use systemprompt_models::auth::{
    ActClaim, AuthenticatedUser, CustomClaims, JwtAudience, Permission, RateLimitTier,
    RegisteredClaims, TokenType, UserType,
};
use systemprompt_security::keys::SigningKeys;

pub mod id_jag;
mod secret;

pub use id_jag::{IdJagGrant, mint_id_jag};
pub use secret::{
    generate_client_secret, generate_secure_token, hash_client_secret, verify_client_secret,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    pub permissions: Vec<Permission>,
    pub audience: Vec<JwtAudience>,
    #[serde(with = "expires_in_seconds")]
    pub expires_in: Duration,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<PluginId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<ClientId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
}

const MAX_TOKEN_LIFETIME: Duration = Duration::days(365);

fn validated_expiry(expires_in: Duration) -> Result<Duration> {
    if expires_in <= Duration::zero() || expires_in > MAX_TOKEN_LIFETIME {
        return Err(crate::error::OauthError::InvalidTokenLifetime {
            seconds: expires_in.num_seconds(),
        });
    }
    Ok(expires_in)
}

mod expires_in_seconds {
    use chrono::Duration;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<S: Serializer>(
        value: &Duration,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.num_seconds().serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Duration, D::Error> {
        i64::deserialize(deserializer).map(Duration::seconds)
    }
}

#[derive(Debug, Clone)]
pub struct JwtSigningParams<'a> {
    pub issuer: &'a str,
    pub keys: &'a SigningKeys,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            permissions: vec![Permission::User],
            audience: JwtAudience::standard(),
            expires_in: Duration::hours(24),
            resource: None,
            plugin_id: None,
            client_id: None,
            purpose: None,
        }
    }
}

/// Inputs for minting an RFC 8693 delegated access token carrying an `act`
/// claim.
#[derive(Debug)]
pub struct DelegatedJwtParams<'a> {
    pub user: &'a AuthenticatedUser,
    pub config: JwtConfig,
    pub jti: AccessTokenId,
    pub session_id: &'a SessionId,
    pub signing: &'a JwtSigningParams<'a>,
    pub act: ActClaim,
}

pub fn generate_jwt_with_act(params: DelegatedJwtParams<'_>) -> Result<String> {
    let mut token = build_claims(
        params.user,
        params.config,
        params.jti,
        params.session_id,
        params.signing,
    )?;
    token.custom.act = Some(params.act);
    encode_with_keys(params.signing.keys, &token)
}

fn build_claims(
    user: &AuthenticatedUser,
    config: JwtConfig,
    jti: AccessTokenId,
    session_id: &SessionId,
    signing: &JwtSigningParams<'_>,
) -> Result<JwtClaims> {
    let expires_in = validated_expiry(config.expires_in)?;
    let expiration = Utc::now()
        .checked_add_signed(expires_in)
        .ok_or(crate::error::OauthError::Internal(
            "Failed to calculate token expiration",
        ))?
        .timestamp();
    let now = Utc::now().timestamp();
    let user_type = UserType::from_permissions(&config.permissions);
    let mut audience = config.audience.clone();
    if let Some(ref resource) = config.resource {
        audience.push(JwtAudience::Resource(resource.clone()));
    }
    Ok(JwtClaims {
        registered: RegisteredClaims {
            sub: user.id.to_string(),
            iat: now,
            exp: expiration,
            nbf: Some(now),
            iss: signing.issuer.to_owned(),
            aud: audience,
            jti: String::from(jti),
        },
        custom: CustomClaims {
            scope: config.permissions,
            username: user.username.clone(),
            email: user.email.clone(),
            user_type,
            roles: user.roles().to_vec(),
            attributes: user.attributes().clone(),
            client_id: config.client_id,
            token_type: TokenType::Bearer,
            auth_time: now,
            session_id: Some(session_id.clone()),
            rate_limit_tier: Some(user_type.rate_tier()),
            plugin_id: config.plugin_id,
            act: None,
            purpose: config.purpose,
        },
    })
}

fn encode_with_keys(keys: &SigningKeys, claims: &JwtClaims) -> Result<String> {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(keys.active_kid().to_owned());
    Ok(encode(&header, claims, keys.encoding_key())?)
}

fn encode_id_jag_with_keys(
    keys: &SigningKeys,
    claims: &crate::services::validation::id_jag::IdJagClaims,
) -> Result<String> {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(keys.active_kid().to_owned());
    header.typ = Some(crate::services::validation::id_jag::ID_JAG_TYP.to_owned());
    Ok(encode(&header, claims, keys.encoding_key())?)
}

pub fn generate_jwt(
    user: &AuthenticatedUser,
    config: JwtConfig,
    jti: AccessTokenId,
    session_id: &SessionId,
    signing: &JwtSigningParams<'_>,
) -> Result<String> {
    let claims = build_claims(user, config, jti, session_id, signing)?;
    encode_with_keys(signing.keys, &claims)
}

pub fn generate_anonymous_jwt_with_expiry(
    user_id: &UserId,
    session_id: &SessionId,
    client_id: &ClientId,
    signing: &JwtSigningParams<'_>,
    expires_in_seconds: i64,
) -> Result<String> {
    let expires_in = validated_expiry(Duration::seconds(expires_in_seconds))?;
    let expiration = Utc::now()
        .checked_add_signed(expires_in)
        .ok_or(crate::error::OauthError::Internal(
            "Failed to calculate token expiration",
        ))?
        .timestamp();

    let now = Utc::now().timestamp();

    let claims = JwtClaims {
        registered: RegisteredClaims {
            sub: user_id.to_string(),
            iat: now,
            exp: expiration,
            nbf: Some(now),
            iss: signing.issuer.to_owned(),
            aud: JwtAudience::standard(),
            jti: String::from(AccessTokenId::generate()),
        },
        custom: CustomClaims {
            scope: vec![Permission::Anonymous],
            username: user_id.to_string(),
            email: user_id.to_string(),
            user_type: UserType::Anon,
            roles: vec!["anonymous".to_owned()],
            attributes: std::collections::BTreeMap::new(),
            client_id: Some(client_id.clone()),
            token_type: TokenType::Bearer,
            auth_time: now,
            session_id: Some(session_id.clone()),
            rate_limit_tier: Some(RateLimitTier::Anon),
            plugin_id: None,
            act: None,
            purpose: None,
        },
    };

    encode_with_keys(signing.keys, &claims)
}
