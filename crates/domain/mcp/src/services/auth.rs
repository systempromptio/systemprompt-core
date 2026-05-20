use crate::error::{McpDomainError, McpDomainResult};
use chrono::Utc;
use jsonwebtoken::{Algorithm, Validation, decode, decode_header};
use systemprompt_models::auth::{JwtAudience, JwtClaims};
use systemprompt_security::keys::authority;

pub fn validate_jwt_token(
    token: &str,
    issuer: &str,
    audiences: &[JwtAudience],
) -> McpDomainResult<JwtClaims> {
    let header = decode_header(token)
        .map_err(|e| McpDomainError::Internal(format!("JWT header decode failed: {e}")))?;
    if header.alg != Algorithm::RS256 {
        return Err(McpDomainError::Internal(
            "JWT must be RS256-signed".to_string(),
        ));
    }
    let kid = header
        .kid
        .as_deref()
        .ok_or_else(|| McpDomainError::Internal("JWT missing `kid` header".to_string()))?;
    let key = authority::decoding_key_for_kid(kid)
        .map_err(|e| McpDomainError::Internal(format!("signing key lookup failed: {e}")))?
        .ok_or_else(|| McpDomainError::Internal(format!("unknown `kid` `{kid}`")))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer]);
    let audience_strs: Vec<&str> = audiences.iter().map(JwtAudience::as_str).collect();
    validation.set_audience(&audience_strs);

    let token_data = decode::<JwtClaims>(token, key, &validation)
        .map_err(|e| McpDomainError::Internal(format!("JWT validation failed: {e}")))?;

    let now = Utc::now().timestamp();

    if token_data.claims.exp < now {
        return Err(McpDomainError::Internal("Token has expired".to_string()));
    }

    Ok(token_data.claims)
}
