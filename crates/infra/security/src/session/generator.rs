use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, Header, encode};
use systemprompt_identifiers::{SessionId, SessionToken, UserId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};

use crate::error::{JwtError, JwtResult};
use crate::keys::authority;

#[derive(Debug)]
pub struct SessionParams<'a> {
    pub user_id: &'a UserId,
    pub session_id: &'a SessionId,
    pub email: &'a str,
    pub duration: Duration,
    pub user_type: UserType,
    pub permissions: Vec<Permission>,
    pub roles: Vec<String>,
    pub department: Option<String>,
    pub rate_limit_tier: RateLimitTier,
}

#[derive(Debug)]
pub struct SessionGenerator {
    issuer: String,
}

impl SessionGenerator {
    pub fn new(issuer: impl Into<String>) -> Self {
        Self {
            issuer: issuer.into(),
        }
    }

    pub fn generate(&self, params: &SessionParams<'_>) -> JwtResult<SessionToken> {
        let now = Utc::now();
        let expiry = now + params.duration;

        let claims = JwtClaims {
            sub: params.user_id.to_string(),
            iat: now.timestamp(),
            exp: expiry.timestamp(),
            nbf: Some(now.timestamp()),
            iss: self.issuer.clone(),
            aud: JwtAudience::standard(),
            jti: uuid::Uuid::new_v4().to_string(),
            scope: params.permissions.clone(),
            username: params.email.to_string(),
            email: params.email.to_string(),
            user_type: params.user_type,
            roles: params.roles.clone(),
            department: params.department.clone(),
            client_id: None,
            token_type: TokenType::Bearer,
            auth_time: now.timestamp(),
            session_id: Some(params.session_id.clone()),
            rate_limit_tier: Some(params.rate_limit_tier),
            plugin_id: None,
            act: None,
        };

        let kid = authority::active_kid().map_err(|e| JwtError::Signing(e.to_string()))?;
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(kid.to_string());
        let key = authority::encoding_key().map_err(|e| JwtError::Signing(e.to_string()))?;
        let token = encode(&header, &claims, key).map_err(JwtError::from)?;

        Ok(SessionToken::new(token))
    }
}
