//! Unverified JWT claim decoding for identity display and diagnostics.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use systemprompt_identifiers::{TenantId, UserId};

#[derive(Debug, Clone, Deserialize)]
pub struct JwtIdentity {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default, rename = "sub")]
    pub user_id: Option<UserId>,
    #[serde(default)]
    pub tenant_id: Option<TenantId>,
    #[serde(default)]
    pub exp: Option<u64>,
}

impl JwtIdentity {
    #[must_use]
    pub fn display_label(&self) -> Option<String> {
        match (&self.email, &self.user_id) {
            (Some(email), Some(id)) => Some(format!("{email} ({id})")),
            (Some(email), None) => Some(email.clone()),
            (None, Some(id)) => Some(id.to_string()),
            (None, None) => None,
        }
    }
}

#[must_use]
pub fn decode_unverified(token: &str) -> Option<JwtIdentity> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.as_bytes()).ok()?;
    serde_json::from_slice(&bytes).ok()
}
