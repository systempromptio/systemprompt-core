//! JWT and client-secret generation primitives.

use crate::error::OauthResult as Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, Header, encode};
use serde::{Deserialize, Serialize};

use crate::models::JwtClaims;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_models::Config;
use systemprompt_models::auth::{
    ActClaim, AuthenticatedUser, JwtAudience, Permission, RateLimitTier, TokenType, UserType,
};
use systemprompt_security::keys::authority;

mod secret;

pub use secret::{
    generate_access_token_jti, generate_client_secret, generate_secure_token, hash_client_secret,
    verify_client_secret,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    pub permissions: Vec<Permission>,
    pub audience: Vec<JwtAudience>,
    pub expires_in_hours: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JwtSigningParams<'a> {
    pub issuer: &'a str,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            permissions: vec![Permission::User],
            audience: JwtAudience::standard(),
            expires_in_hours: Some(24),
            resource: None,
            plugin_id: None,
        }
    }
}

/// Mint a delegated access token carrying an RFC 8693 `act` claim chain.
///
/// `act` is the outermost actor that requested this exchange (typically the
/// authenticated client). Any pre-existing `act` chain inside the subject
/// token is preserved by chaining it underneath the new outer actor.
#[expect(
    clippy::too_many_arguments,
    reason = "JWT minting needs the full set of claim-shaping inputs; bundling into a struct \
              would obscure the call sites"
)]
pub fn generate_jwt_with_act(
    user: &AuthenticatedUser,
    config: JwtConfig,
    jti: String,
    session_id: &SessionId,
    signing: &JwtSigningParams<'_>,
    act: ActClaim,
) -> Result<String> {
    let mut token = build_claims(user, config, jti, session_id, signing)?;
    token.act = Some(act);
    encode_claims(&token, signing)
}

fn build_claims(
    user: &AuthenticatedUser,
    config: JwtConfig,
    jti: String,
    session_id: &SessionId,
    signing: &JwtSigningParams<'_>,
) -> Result<JwtClaims> {
    let expires_in_hours = config.expires_in_hours.unwrap_or(24);
    if expires_in_hours <= 0 || expires_in_hours > 8760 {
        return Err(crate::error::OauthError::Internal(format!(
            "Invalid token expiry: {expires_in_hours} hours. Must be between 1 and 8760 (1 year)"
        )));
    }
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(expires_in_hours))
        .ok_or_else(|| {
            crate::error::OauthError::Internal("Failed to calculate token expiration".to_owned())
        })?
        .timestamp();
    let now = Utc::now().timestamp();
    let user_type = user.user_type();
    let mut audience = config.audience.clone();
    if let Some(ref resource) = config.resource {
        audience.push(JwtAudience::Resource(resource.clone()));
    }
    Ok(JwtClaims {
        sub: user.id.to_string(),
        iat: now,
        exp: expiration,
        nbf: Some(now),
        iss: signing.issuer.to_owned(),
        aud: audience,
        jti,
        scope: config.permissions,
        username: user.username.clone(),
        email: user.email.clone(),
        user_type,
        roles: user.roles().to_vec(),
        department: user.department().map(str::to_owned),
        client_id: None,
        token_type: TokenType::Bearer,
        auth_time: now,
        session_id: Some(session_id.clone()),
        rate_limit_tier: Some(user_type.rate_tier()),
        plugin_id: config.plugin_id,
        act: None,
    })
}

fn encode_claims(claims: &JwtClaims, _signing: &JwtSigningParams<'_>) -> Result<String> {
    encode_with_authority(claims)
}

fn encode_with_authority(claims: &JwtClaims) -> Result<String> {
    let kid = authority::active_kid()
        .map_err(|e| crate::error::OauthError::Internal(format!("signing key unavailable: {e}")))?;
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_owned());
    let key = authority::encoding_key()
        .map_err(|e| crate::error::OauthError::Internal(format!("signing key unavailable: {e}")))?;
    let token = encode(&header, claims, key)?;
    Ok(token)
}

pub fn generate_jwt(
    user: &AuthenticatedUser,
    config: JwtConfig,
    jti: String,
    session_id: &SessionId,
    signing: &JwtSigningParams<'_>,
) -> Result<String> {
    let claims = build_claims(user, config, jti, session_id, signing)?;
    encode_with_authority(&claims)
}

pub fn generate_anonymous_jwt(
    user_id: &UserId,
    session_id: &SessionId,
    client_id: &systemprompt_identifiers::ClientId,
    signing: &JwtSigningParams<'_>,
) -> Result<String> {
    let expires_in_seconds = Config::get()?.jwt_access_token_expiration;
    generate_anonymous_jwt_with_expiry(user_id, session_id, client_id, signing, expires_in_seconds)
}

pub fn generate_anonymous_jwt_with_expiry(
    user_id: &UserId,
    session_id: &SessionId,
    client_id: &systemprompt_identifiers::ClientId,
    signing: &JwtSigningParams<'_>,
    expires_in_seconds: i64,
) -> Result<String> {
    let expires_in_hours = expires_in_seconds / 3600;
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(expires_in_hours))
        .ok_or_else(|| {
            crate::error::OauthError::Internal("Failed to calculate token expiration".to_owned())
        })?
        .timestamp();

    let now = Utc::now().timestamp();

    let claims = JwtClaims {
        sub: user_id.to_string(),
        iat: now,
        exp: expiration,
        nbf: Some(now),
        iss: signing.issuer.to_owned(),
        aud: JwtAudience::standard(),
        jti: uuid::Uuid::new_v4().to_string(),
        scope: vec![Permission::Anonymous],
        username: user_id.to_string(),
        email: user_id.to_string(),
        user_type: UserType::Anon,
        roles: vec!["anonymous".to_owned()],
        department: None,
        client_id: Some(client_id.clone()),
        token_type: TokenType::Bearer,
        auth_time: now,
        session_id: Some(session_id.clone()),
        rate_limit_tier: Some(RateLimitTier::Anon),
        plugin_id: None,
        act: None,
    };

    encode_with_authority(&claims)
}

pub fn generate_admin_jwt(
    user_id: &UserId,
    session_id: &SessionId,
    email: &str,
    client_id: &systemprompt_identifiers::ClientId,
    signing: &JwtSigningParams<'_>,
) -> Result<String> {
    let expires_in_seconds = Config::get()?.jwt_access_token_expiration;
    generate_admin_jwt_with_expiry(
        user_id,
        session_id,
        email,
        client_id,
        signing,
        expires_in_seconds,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "admin JWT minting needs the full set of claim-shaping inputs; bundling into a \
              struct would obscure the call sites"
)]
pub fn generate_admin_jwt_with_expiry(
    user_id: &UserId,
    session_id: &SessionId,
    email: &str,
    client_id: &systemprompt_identifiers::ClientId,
    signing: &JwtSigningParams<'_>,
    expires_in_seconds: i64,
) -> Result<String> {
    let expires_in_hours = expires_in_seconds / 3600;
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(expires_in_hours))
        .ok_or_else(|| {
            crate::error::OauthError::Internal("Failed to calculate token expiration".to_owned())
        })?
        .timestamp();

    let now = Utc::now().timestamp();

    let claims = JwtClaims {
        sub: user_id.to_string(),
        iat: now,
        exp: expiration,
        nbf: Some(now),
        iss: signing.issuer.to_owned(),
        aud: JwtAudience::standard(),
        jti: uuid::Uuid::new_v4().to_string(),
        scope: vec![Permission::Admin],
        username: email.to_owned(),
        email: email.to_owned(),
        user_type: UserType::Admin,
        roles: vec!["admin".to_owned(), "user".to_owned()],
        department: None,
        client_id: Some(client_id.clone()),
        token_type: TokenType::Bearer,
        auth_time: now,
        session_id: Some(session_id.clone()),
        rate_limit_tier: Some(RateLimitTier::Admin),
        plugin_id: None,
        act: None,
    };

    encode_with_authority(&claims)
}
