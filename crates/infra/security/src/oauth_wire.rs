//! Lenient readers for OAuth 2.0 token-endpoint response members.
//!
//! RFC 6749 §5.1 types `expires_in` as a number and makes it optional, but
//! deployed authorization servers (Microsoft's v1 endpoint, Salesforce,
//! several on-prem ADFS builds) send it as a numeric string. Refusing the
//! whole response over that spelling ends the grant, so both are read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Deserializer};

// Why: servers send `expires_in` as a number, a numeric string, `null`, or
// not at all; a negative or non-numeric value reads as absent so the caller
// falls back to its own default lifetime.
pub fn expires_in<'de, D: Deserializer<'de>>(de: D) -> Result<Option<u64>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Lifetime {
        Number(u64),
        Text(String),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Option::<Lifetime>::deserialize(de)? {
        Some(Lifetime::Number(secs)) => Some(secs),
        Some(Lifetime::Text(text)) => text.trim().parse::<u64>().ok(),
        Some(Lifetime::Other(_)) | None => None,
    })
}
