use base64::prelude::*;
use chrono::{Duration, Utc};
use serde::Deserialize;
use systemprompt_identifiers::CloudAuthToken;

use crate::error::{CloudError, CloudResult};

#[derive(Deserialize)]
struct JwtClaims {
    exp: i64,
}

pub fn decode_expiry(token: &CloudAuthToken) -> CloudResult<i64> {
    let token_str = token.as_str();
    let parts: Vec<&str> = token_str.split('.').collect();
    if parts.len() != 3 {
        return Err(CloudError::JwtDecode);
    }

    let payload = BASE64_URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| CloudError::JwtDecode)?;

    let claims: JwtClaims = serde_json::from_slice(&payload).map_err(|_| CloudError::JwtDecode)?;

    Ok(claims.exp)
}

pub fn is_expired(token: &CloudAuthToken) -> bool {
    match decode_expiry(token) {
        Ok(exp) => exp < Utc::now().timestamp(),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to decode token expiry, treating as expired");
            true
        },
    }
}

pub fn expires_within(token: &CloudAuthToken, duration: Duration) -> bool {
    match decode_expiry(token) {
        Ok(exp) => {
            let threshold = Utc::now().timestamp() + duration.num_seconds();
            exp < threshold
        },
        Err(e) => {
            tracing::warn!(error = %e, "Failed to decode token expiry, treating as expiring");
            true
        },
    }
}
