use anyhow::{anyhow, Result};
use chrono::Utc;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use systemprompt_models::auth::JwtAudience;

use crate::models::JwtClaims;

pub fn validate_jwt_token(
    token: &str,
    jwt_secret: &str,
    issuer: &str,
    audiences: &[JwtAudience],
) -> Result<JwtClaims> {
    let mut validation = Validation::new(Algorithm::HS256);

    validation.set_issuer(&[issuer]);

    let audience_strs: Vec<&str> = audiences.iter().map(JwtAudience::as_str).collect();
    validation.set_audience(&audience_strs);

    let token_data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|e| anyhow!("JWT validation failed: {e}"))?;

    let now = Utc::now().timestamp();

    if token_data.claims.exp < now {
        return Err(anyhow!("Token has expired"));
    }

    Ok(token_data.claims)
}
