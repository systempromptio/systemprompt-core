use anyhow::Result;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use systemprompt_identifiers::{JwtToken, SessionId, UserId};
use systemprompt_models::auth::{
    JwtAudience, JwtClaims, Permission, RateLimitTier, TokenType, UserType,
};

#[derive(Debug)]
pub struct AdminTokenParams<'a> {
    pub user_id: &'a UserId,
    pub session_id: &'a SessionId,
    pub email: &'a str,
    pub jwt_secret: &'a str,
    pub issuer: &'a str,
    pub duration: Duration,
}

#[derive(Copy, Clone, Debug)]
pub struct JwtService;

impl JwtService {
    pub fn generate_admin_token(params: &AdminTokenParams<'_>) -> Result<JwtToken> {
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
            &EncodingKey::from_secret(params.jwt_secret.as_bytes()),
        )?;

        Ok(JwtToken::new(token))
    }
}
