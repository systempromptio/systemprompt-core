//! JWT minting service.
//!
//! Produces administrator-scoped RS256 tokens for the bridge management
//! plane and CLI bootstrap flows. Session-scoped tokens are minted by
//! [`crate::session::SessionGenerator`] instead.

use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, Header, encode};
use systemprompt_identifiers::{ClientId, JwtToken, SessionId, UserId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};

use crate::error::{JwtError, JwtResult};
use crate::keys::authority;

#[derive(Debug)]
pub struct AdminTokenParams<'a> {
    pub user_id: &'a UserId,
    pub session_id: &'a SessionId,
    pub email: &'a str,
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
            act: None,
        };

        let kid = authority::active_kid().map_err(|e| JwtError::Signing(e.to_string()))?;
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(kid.to_string());
        let key = authority::encoding_key().map_err(|e| JwtError::Signing(e.to_string()))?;
        let token = encode(&header, &claims, key).map_err(JwtError::from)?;

        Ok(JwtToken::new(token))
    }
}
