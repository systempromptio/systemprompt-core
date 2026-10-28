//! JWKS document types per RFC 7517 / RFC 7518.
//!
//! A [`Jwks`] is the set systemprompt.io publishes: RS256 [`Jwk`]s only. A
//! [`VerificationKeySet`] is a set read from an issuer, leniently: members a
//! key does not need (`use`, `key_ops`, `x5c`, `x5t`, and any provider
//! extension such as Entra ID's `issuer` / `cloud_instance_name`) are optional
//! or ignored, and each key is pinned to exactly one algorithm — RS256 for an
//! RSA key, ES256 for a P-256 EC key. A key that cannot verify either — an
//! `OKP` key, another curve, an encryption key, a key bound to another
//! algorithm, or one without a `kid` — is dropped from the set rather than
//! failing it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::{Algorithm, DecodingKey};
use rsa::RsaPublicKey;
use rsa::traits::PublicKeyParts;
use serde::{Deserialize, Deserializer, Serialize};

pub const RS256: &str = "RS256";
pub const ES256: &str = "ES256";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Jwk {
    pub kty: String,
    pub alg: String,
    #[serde(rename = "use")]
    pub use_: String,
    pub kid: String,
    pub n: String,
    pub e: String,
}

impl Jwk {
    pub fn from_rsa_public_key(public_key: &RsaPublicKey, kid: String) -> Self {
        let n = URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be());
        let e = URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be());
        Self {
            kty: "RSA".to_owned(),
            alg: RS256.to_owned(),
            use_: "sig".to_owned(),
            kid,
            n,
            e,
        }
    }
}

/// An issuer's signature key, bound to the single algorithm it may verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationKey {
    Rsa { kid: String, n: String, e: String },
    Ec { kid: String, x: String, y: String },
}

impl VerificationKey {
    pub fn kid(&self) -> &str {
        match self {
            Self::Rsa { kid, .. } | Self::Ec { kid, .. } => kid,
        }
    }

    pub const fn algorithm(&self) -> Algorithm {
        match self {
            Self::Rsa { .. } => Algorithm::RS256,
            Self::Ec { .. } => Algorithm::ES256,
        }
    }

    pub fn decoding_key(&self) -> Result<DecodingKey, jsonwebtoken::errors::Error> {
        match self {
            Self::Rsa { n, e, .. } => DecodingKey::from_rsa_components(n, e),
            Self::Ec { x, y, .. } => DecodingKey::from_ec_components(x, y),
        }
    }
}

impl From<&Jwk> for VerificationKey {
    fn from(jwk: &Jwk) -> Self {
        Self::Rsa {
            kid: jwk.kid.clone(),
            n: jwk.n.clone(),
            e: jwk.e.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationKeySet {
    pub keys: Vec<VerificationKey>,
}

impl From<&Jwks> for VerificationKeySet {
    fn from(jwks: &Jwks) -> Self {
        Self {
            keys: jwks.keys.iter().map(VerificationKey::from).collect(),
        }
    }
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
    crv: Option<String>,
    #[serde(default)]
    n: Option<String>,
    #[serde(default)]
    e: Option<String>,
    #[serde(default)]
    x: Option<String>,
    #[serde(default)]
    y: Option<String>,
}

fn present(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

impl PublishedKey {
    // Why: RFC 7517 §4.2/§4.3 restrict a key's purpose and §4.4 its algorithm;
    // RFC 8725 §3.1 forbids verifying with a key bound to another algorithm.
    fn into_verification_key(self) -> Option<VerificationKey> {
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
        let pinned = match self.kty.as_deref() {
            Some("RSA") => RS256,
            // Why: RFC 7518 §3.4 binds ES256 to the P-256 curve.
            Some("EC") if self.crv.as_deref() == Some("P-256") => ES256,
            _ => return None,
        };
        if self.alg.as_deref().is_some_and(|alg| alg != pinned) {
            return None;
        }
        let kid = present(self.kid)?;
        if pinned == RS256 {
            Some(VerificationKey::Rsa {
                kid,
                n: present(self.n)?,
                e: present(self.e)?,
            })
        } else {
            Some(VerificationKey::Ec {
                kid,
                x: present(self.x)?,
                y: present(self.y)?,
            })
        }
    }
}

impl<'de> Deserialize<'de> for VerificationKeySet {
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
            .filter_map(PublishedKey::into_verification_key)
            .collect();
        Ok(Self { keys })
    }
}
