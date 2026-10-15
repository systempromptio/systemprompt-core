//! JWKS document types per RFC 7517 / RFC 7518, scoped to the RS256 keys
//! systemprompt.io publishes and consumes.
//!
//! A [`Jwk`] is an RSA signature key. A published [`Jwks`] is read leniently:
//! members a key does not need (`alg`, `use`, `key_ops`, `x5c`, `x5t`, and any
//! provider extension such as Entra ID's `issuer` / `cloud_instance_name`) are
//! optional or ignored, and a key that cannot verify an RS256 signature — an
//! `EC` / `OKP` key, an encryption key, a key bound to another algorithm, or
//! one without a `kid` — is dropped from the set rather than failing it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rsa::RsaPublicKey;
use rsa::traits::PublicKeyParts;
use serde::{Deserialize, Deserializer, Serialize};

pub const RS256: &str = "RS256";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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
    // RFC 7517 §4.2: `use` is optional; an absent one does not restrict the key.
    #[serde(rename = "use", default = "default_use")]
    pub use_: String,
    pub kid: String,
    pub n: String,
    pub e: String,
}

fn default_alg() -> String {
    RS256.to_owned()
}

fn default_use() -> String {
    "sig".to_owned()
}

#[derive(Deserialize)]
struct PublishedKey {
    #[serde(default)]
    kty: Option<String>,
    #[serde(default)]
    alg: Option<String>,
    #[serde(rename = "use", default)]
    use_: Option<String>,
    #[serde(default)]
    key_ops: Option<Vec<String>>,
    #[serde(default)]
    kid: Option<String>,
    #[serde(default)]
    n: Option<String>,
    #[serde(default)]
    e: Option<String>,
}

impl PublishedKey {
    // RFC 7517 §4.2/§4.3 restrict a key's purpose and §4.4 its algorithm;
    // RFC 8725 §3.1 forbids verifying with a key bound to another algorithm.
    fn into_rs256_verification_key(self) -> Option<Jwk> {
        if self.kty.as_deref() != Some("RSA") {
            return None;
        }
        if self.use_.as_deref().is_some_and(|u| u != "sig") {
            return None;
        }
        if self
            .key_ops
            .as_ref()
            .is_some_and(|ops| !ops.iter().any(|op| op == "verify"))
        {
            return None;
        }
        if self.alg.as_deref().is_some_and(|alg| alg != RS256) {
            return None;
        }
        let kid = self.kid.filter(|kid| !kid.is_empty())?;
        Some(Jwk {
            kty: "RSA".to_owned(),
            alg: RS256.to_owned(),
            use_: default_use(),
            kid,
            n: self.n.filter(|n| !n.is_empty())?,
            e: self.e.filter(|e| !e.is_empty())?,
        })
    }
}

impl<'de> Deserialize<'de> for Jwks {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct PublishedSet {
            // JSON: each member is read on its own so one key of an unknown
            // shape cannot fail the whole set (RFC 7517 §5).
            keys: Vec<serde_json::Value>,
        }
        let set = PublishedSet::deserialize(deserializer)?;
        let keys = set
            .keys
            .into_iter()
            .filter_map(|raw| serde_json::from_value::<PublishedKey>(raw).ok())
            .filter_map(PublishedKey::into_rs256_verification_key)
            .collect();
        Ok(Self { keys })
    }
}

impl Jwk {
    pub fn from_rsa_public_key(public_key: &RsaPublicKey, kid: String) -> Self {
        let n = URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be());
        let e = URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be());
        Self {
            kty: "RSA".to_owned(),
            alg: RS256.to_owned(),
            use_: default_use(),
            kid,
            n,
            e,
        }
    }
}
