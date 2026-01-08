use anyhow::Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use systemprompt_identifiers::{SessionId, SessionToken, UserId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};

#[derive(Debug)]
pub struct SessionParams<'a> {
    pub user_id: &'a UserId,
    pub session_id: &'a SessionId,
    pub email: &'a str,
    pub duration: Duration,
}

#[derive(Debug)]
pub struct SessionGenerator {
    jwt_secret: String,
    issuer: String,
}

impl SessionGenerator {
    pub fn new(jwt_secret: impl Into<String>, issuer: impl Into<String>) -> Self {
        Self {
            jwt_secret: jwt_secret.into(),
            issuer: issuer.into(),
        }
    }

    pub fn generate(&self, params: &SessionParams<'_>) -> Result<SessionToken> {
        let now = Utc::now();
        let expiry = now + params.duration;

        let claims = JwtClaims {
            sub: params.user_id.to_string(),
            iat: now.timestamp(),
            exp: expiry.timestamp(),
            iss: self.issuer.clone(),
            aud: JwtAudience::standard(),
            jti: uuid::Uuid::new_v4().to_string(),
            scope: vec![Permission::Admin],
            username: params.email.to_string(),
            email: params.email.to_string(),
            user_type: UserType::Admin,
            client_id: Some("sp_tui".to_string()),
            token_type: TokenType::Bearer,
            auth_time: now.timestamp(),
            session_id: Some(params.session_id.to_string()),
            rate_limit_tier: Some(RateLimitTier::Admin),
        };

        let header = Header::new(Algorithm::HS256);
        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;

        Ok(SessionToken::new(token))
    }
}
