use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use systemprompt_identifiers::{SessionId, SessionToken, UserId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};

use crate::error::{JwtError, JwtResult};

/// Parameters required to mint a session-scoped JWT.
///
/// Borrowed rather than owned so callers can keep typed identifiers and
/// permission/role vectors alive without cloning at the call site.
#[derive(Debug)]
pub struct SessionParams<'a> {
    /// Subject of the token (the authenticated user).
    pub user_id: &'a UserId,
    /// Session id embedded as the `session_id` claim.
    pub session_id: &'a SessionId,
    /// Email address embedded as both `username` and `email`.
    pub email: &'a str,
    /// Lifetime of the issued token starting at the current wall clock.
    pub duration: Duration,
    /// Effective user type stored as the `user_type` claim.
    pub user_type: UserType,
    /// Permissions granted to this session (the `scope` claim).
    pub permissions: Vec<Permission>,
    /// Roles granted to this session (the `roles` claim).
    pub roles: Vec<String>,
    /// Rate-limit tier applied by the API gateway.
    pub rate_limit_tier: RateLimitTier,
}

/// Mints session-scoped HS256 JWTs.
///
/// Holds the signing secret and issuer, which are typically loaded once at
/// startup from the active profile.
#[derive(Debug)]
pub struct SessionGenerator {
    jwt_secret: String,
    issuer: String,
}

impl SessionGenerator {
    /// Constructs a session generator with the supplied secret and issuer.
    pub fn new(jwt_secret: impl Into<String>, issuer: impl Into<String>) -> Self {
        Self {
            jwt_secret: jwt_secret.into(),
            issuer: issuer.into(),
        }
    }

    /// Generates a new session JWT.
    ///
    /// # Errors
    ///
    /// Returns [`JwtError::Encoding`] if the underlying `jsonwebtoken`
    /// encoder rejects the claim set or signing key.
    pub fn generate(&self, params: &SessionParams<'_>) -> JwtResult<SessionToken> {
        let now = Utc::now();
        let expiry = now + params.duration;

        let claims = JwtClaims {
            sub: params.user_id.to_string(),
            iat: now.timestamp(),
            exp: expiry.timestamp(),
            iss: self.issuer.clone(),
            aud: JwtAudience::standard(),
            jti: uuid::Uuid::new_v4().to_string(),
            scope: params.permissions.clone(),
            username: params.email.to_string(),
            email: params.email.to_string(),
            user_type: params.user_type,
            roles: params.roles.clone(),
            client_id: None,
            token_type: TokenType::Bearer,
            auth_time: now.timestamp(),
            session_id: Some(params.session_id.to_string()),
            rate_limit_tier: Some(params.rate_limit_tier),
        };

        let header = Header::new(Algorithm::HS256);
        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(JwtError::from)?;

        Ok(SessionToken::new(token))
    }
}
