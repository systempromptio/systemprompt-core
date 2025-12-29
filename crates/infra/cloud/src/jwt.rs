use base64::prelude::*;
use chrono::{Duration, Utc};
use serde::Deserialize;

use crate::error::{CloudError, CloudResult};

#[derive(Deserialize)]
struct JwtClaims {
    exp: i64,
}

pub fn decode_expiry(token: &str) -> CloudResult<i64> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(CloudError::JwtDecode);
    }

    let payload = BASE64_URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| CloudError::JwtDecode)?;

    let claims: JwtClaims = serde_json::from_slice(&payload).map_err(|_| CloudError::JwtDecode)?;

    Ok(claims.exp)
}

pub fn is_expired(token: &str) -> bool {
    decode_expiry(token)
        .map(|exp| exp < Utc::now().timestamp())
        .unwrap_or(true)
}

pub fn expires_within(token: &str, duration: Duration) -> bool {
    decode_expiry(token)
        .map(|exp| {
            let threshold = Utc::now().timestamp() + duration.num_seconds();
            exp < threshold
        })
        .unwrap_or(true)
}
