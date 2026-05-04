//! JWT minting service.
//!
//! Produces administrator-scoped HS256 tokens for the cowork management
//! plane and CLI bootstrap flows. Session-scoped tokens are minted by
//! [`crate::session::SessionGenerator`] instead.

use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use systemprompt_identifiers::{ClientId, JwtToken, SessionId, UserId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};

use crate::error::{JwtError, JwtResult};

/// Parameters required to mint an administrator JWT.
///
/// Borrowed rather than owned so callers can keep their typed identifiers
/// alive without cloning.
#[derive(Debug)]
pub struct AdminTokenParams<'a> {
    /// Subject of the token (the admin user).
    pub user_id: &'a UserId,
    /// Session id embedded as the `session_id` claim.
    pub session_id: &'a SessionId,
    /// Email address embedded as both `username` and `email`.
    pub email: &'a str,
    /// HMAC-SHA256 signing secret.
    pub jwt_secret: &'a str,
    /// Issuer string used as the `iss` claim.
    pub issuer: &'a str,
    /// Lifetime of the issued token starting at the current wall clock.
    pub duration: Duration,
    /// Optional OAuth client id embedded as `client_id` for downstream
    /// audit attribution.
    pub client_id: Option<&'a ClientId>,
}

/// Stateless JWT minting service.
#[derive(Copy, Clone, Debug)]
pub struct JwtService;

impl JwtService {
    /// Generates a fully-scoped administrator JWT.
    ///
    /// # Errors
    ///
    /// Returns [`JwtError::Encoding`] if the underlying `jsonwebtoken`
    /// encoder rejects the claim set or signing key (e.g. the key is the
    /// wrong length).
    pub fn generate_admin_token(params: &AdminTokenParams<'_>) -> JwtResult<JwtToken> {
        let now = Utc::now();
        let expiry = now + params.duration;

        let claims = JwtClaims {
            sub: params.user_id.to_string(),
            iat: now.timestamp(),
            exp: expiry.timestamp(),
            iss: params.issuer.to_string(),
            aud: JwtAudience::standard(),
            jti: uuid::Uuid::new_v4().to_string(),
            scope: vec![Permission::Admin],
            username: params.email.to_string(),
            email: params.email.to_string(),
            user_type: UserType::Admin,
            roles: vec!["admin".to_string(), "user".to_string()],
            client_id: params.client_id.map(ToString::to_string),
            token_type: TokenType::Bearer,
            auth_time: now.timestamp(),
            session_id: Some(params.session_id.to_string()),
            rate_limit_tier: Some(RateLimitTier::Admin),
        };

        let header = Header::new(Algorithm::HS256);
        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_secret(params.jwt_secret.as_bytes()),
        )
        .map_err(JwtError::from)?;

        Ok(JwtToken::new(token))
    }
}
