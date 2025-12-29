use anyhow::{anyhow, Result};
use axum::http::{header, HeaderMap};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

use systemprompt_core_oauth::models::JwtClaims;
use systemprompt_identifiers::{ClientId, SessionId, UserId};
use systemprompt_models::auth::UserType;

pub fn extract_token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(auth_header) = headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
    }

    if let Some(cookie_header) = headers.get(header::COOKIE) {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for cookie in cookie_str.split(';') {
                let cookie = cookie.trim();
                if let Some(value) = cookie.strip_prefix("access_token=") {
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }

    None
}

#[derive(Debug, Clone)]
pub struct JwtUserContext {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub role: systemprompt_models::auth::Permission,
    pub user_type: UserType,
    pub client_id: Option<ClientId>,
}

pub struct JwtExtractor {
    decoding_key: DecodingKey,
    validation: Validation,
}

impl std::fmt::Debug for JwtExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtExtractor")
            .field("decoding_key", &"<DecodingKey>")
            .field("validation", &self.validation)
            .finish()
    }
}

impl JwtExtractor {
    pub fn new(jwt_secret: &str) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.validate_aud = false;

        Self {
            decoding_key: DecodingKey::from_secret(jwt_secret.as_bytes()),
            validation,
        }
    }

    pub fn validate_token(&self, token: &str) -> Result<(), String> {
        match decode::<JwtClaims>(token, &self.decoding_key, &self.validation) {
            Ok(_) => Ok(()),
            Err(err) => {
                let reason = err.to_string();
                if reason.contains("InvalidSignature") || reason.contains("invalid signature") {
                    Err("Invalid signature".to_string())
                } else if reason.contains("ExpiredSignature") || reason.contains("token expired") {
                    Err("Token expired".to_string())
                } else if reason.contains("MissingRequiredClaim") || reason.contains("missing") {
                    Err("Missing required claim".to_string())
                } else {
                    Err("Invalid token".to_string())
                }
            },
        }
    }

    pub fn extract_user_context(&self, token: &str) -> Result<JwtUserContext> {
        let token_data = decode::<JwtClaims>(token, &self.decoding_key, &self.validation)?;

        let session_id_str = token_data
            .claims
            .session_id
            .ok_or_else(|| anyhow!("JWT must contain session_id claim"))?;

        let role = *token_data
            .claims
            .scope
            .first()
            .ok_or_else(|| anyhow!("JWT must contain valid scope claim"))?;

        let client_id = token_data.claims.client_id.map(ClientId::new);

        Ok(JwtUserContext {
            user_id: UserId::new(token_data.claims.sub),
            session_id: SessionId::new(session_id_str),
            role,
            user_type: token_data.claims.user_type,
            client_id,
        })
    }
}
