//! JWKS document types per RFC 7517 / RFC 7518, scoped to the RS256 keys
//! systemprompt.io publishes and consumes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rsa::RsaPublicKey;
use rsa::traits::PublicKeyParts;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Jwk {
    pub kty: String,
    // RFC 7517 §4.4: `alg` is optional and Entra ID omits it; callers pin the
    // algorithm from the token header, never from the key.
    #[serde(default = "default_alg")]
    pub alg: String,
    #[serde(rename = "use")]
    pub use_: String,
    pub kid: String,
    pub n: String,
    pub e: String,
}

fn default_alg() -> String {
    "RS256".to_owned()
}

impl Jwk {
    pub fn from_rsa_public_key(public_key: &RsaPublicKey, kid: String) -> Self {
        let n = URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be());
        let e = URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be());
        Self {
            kty: "RSA".to_owned(),
            alg: "RS256".to_owned(),
            use_: "sig".to_owned(),
            kid,
            n,
            e,
        }
    }
}
