//! JWT and client-secret generation primitives.
//!
//! The `purpose` claim carries the caller purpose. Only in-process issuers set
//! it; no endpoint copies it from a request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::OauthResult as Result;
use chrono::Duration;
use jsonwebtoken::{Algorithm, Header, encode};
use serde::{Deserialize, Serialize};

use crate::models::JwtClaims;
use systemprompt_identifiers::{AccessTokenId, ClientId, PluginId, SessionId, UserId};
use systemprompt_models::auth::{
    AccountKind, ActClaim, AuthenticatedUser, JwtAudience, Permission, TokenAuthority,
    TokenInteraction, UserJwtRequest,
};
use systemprompt_security::jwt::{encode_rs256, user_claims};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction: Option<TokenInteraction>,
    #[serde(default)]
    pub browser_session: bool,
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
    let mut audience = config.audience;
    if let Some(resource) = config.resource {
        audience.push(JwtAudience::Resource(resource));
    }
    let request = UserJwtRequest {
        user: user.clone(),
        session_id: session_id.clone(),
        jti,
        permissions: config.permissions,
        audience,
        ttl: config.expires_in,
        client_id: config.client_id,
        purpose: config.purpose,
        plugin_id: config.plugin_id,
        interaction: config.interaction,
        browser_session: config.browser_session,
    };
    Ok(user_claims(signing.issuer, &request)?)
}

fn encode_with_keys(keys: &SigningKeys, claims: &JwtClaims) -> Result<String> {
    Ok(encode_rs256(keys, claims)?)
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
    authority: &dyn TokenAuthority,
    user_id: &UserId,
    session_id: &SessionId,
    client_id: &ClientId,
    expires_in_seconds: i64,
) -> Result<String> {
    let request = UserJwtRequest {
        user: AuthenticatedUser {
            id: user_id.clone(),
            username: user_id.to_string(),
            email: user_id.to_string(),
            permissions: vec![Permission::Anonymous],
            roles: vec!["anonymous".to_owned()],
            attributes: std::collections::BTreeMap::new(),
            account_kind: AccountKind::Person,
        },
        session_id: session_id.clone(),
        jti: AccessTokenId::generate(),
        permissions: vec![Permission::Anonymous],
        audience: JwtAudience::standard(),
        ttl: validated_expiry(Duration::seconds(expires_in_seconds))?,
        client_id: Some(client_id.clone()),
        purpose: None,
        plugin_id: None,
        interaction: None,
        browser_session: true,
    };
    Ok(authority.mint_user_jwt(&request)?)
}
