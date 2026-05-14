//! JWT minting service.
//!
//! Produces administrator-scoped HS256 tokens for the bridge management
//! plane and CLI bootstrap flows. Session-scoped tokens are minted by
//! [`crate::session::SessionGenerator`] instead.

use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use systemprompt_identifiers::{ClientId, JwtToken, SessionId, UserId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};

use crate::error::{JwtError, JwtResult};

#[derive(Debug)]
pub struct AdminTokenParams<'a> {
    pub user_id: &'a UserId,
    pub session_id: &'a SessionId,
    pub email: &'a str,
    pub jwt_secret: &'a str,
    pub issuer: &'a str,
    pub duration: Duration,
    pub client_id: Option<&'a ClientId>,
}

#[derive(Copy, Clone, Debug)]
pub struct JwtService;

impl JwtService {
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
            department: None,
            client_id: params.client_id.cloned(),
            token_type: TokenType::Bearer,
            auth_time: now.timestamp(),
            session_id: Some(params.session_id.clone()),
            rate_limit_tier: Some(RateLimitTier::Admin),
            plugin_id: None,
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
