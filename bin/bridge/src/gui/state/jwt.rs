use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;

use super::{VerifiedIdentity, now_unix};

#[derive(Debug, Deserialize)]
struct JwtClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    sub: Option<String>,
    #[serde(default)]
    tenant_id: Option<String>,
    #[serde(default)]
    exp: Option<u64>,
}

pub fn decode_jwt_identity_unverified(token: &str) -> Option<VerifiedIdentity> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.as_bytes()).ok()?;
    let claims: JwtClaims = serde_json::from_slice(&bytes).ok()?;
    Some(VerifiedIdentity {
        email: claims.email,
        user_id: claims.sub,
        tenant_id: claims.tenant_id,
        exp_unix: claims.exp,
        verified_at_unix: now_unix(),
    })
}
