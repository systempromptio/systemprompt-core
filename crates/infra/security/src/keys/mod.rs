//! RSA signing-key infrastructure for systemprompt.io's federated JWT plane.
//!
//! Provides an [`RsaSigningKey`] wrapper around an `rsa::RsaPrivateKey` that
//! can be generated, loaded from PKCS#8 PEM, persisted to PEM, and exposes a
//! deterministic `kid` (SHA-256 of the DER-encoded `SubjectPublicKeyInfo`,
//! base64 URL-encoded, no padding). The accompanying [`jwks`] module turns the
//! public half into a JWKS document.

use std::fs;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use pkcs8::LineEnding;
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey};
use rsa::rand_core::OsRng;
use rsa::{RsaPrivateKey, RsaPublicKey};
use sha2::{Digest, Sha256};

pub mod jwks;
pub mod jwks_client;

pub use jwks::{Jwk, Jwks};
pub use jwks_client::{JwksClient, JwksClientError};

pub const DEFAULT_RSA_BITS: usize = 2048;

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("RSA key generation failed: {0}")]
    Generation(#[source] rsa::Error),
    #[error("PKCS#8 encoding failed: {0}")]
    Encode(#[source] pkcs8::Error),
    #[error("SPKI encoding failed: {0}")]
    EncodeSpki(#[source] pkcs8::spki::Error),
    #[error("PKCS#8 decoding failed: {0}")]
    Decode(#[source] pkcs8::Error),
    #[error("I/O error for {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Clone)]
pub struct RsaSigningKey {
    private_key: RsaPrivateKey,
    public_key: RsaPublicKey,
    kid: String,
}

impl std::fmt::Debug for RsaSigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RsaSigningKey")
            .field("kid", &self.kid)
            .finish_non_exhaustive()
    }
}

impl RsaSigningKey {
    pub fn generate() -> Result<Self, KeyError> {
        Self::generate_bits(DEFAULT_RSA_BITS)
    }

    pub fn generate_bits(bits: usize) -> Result<Self, KeyError> {
        let mut rng = OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, bits).map_err(KeyError::Generation)?;
        Self::from_private(private_key)
    }

    pub fn from_pkcs8_pem(pem: &str) -> Result<Self, KeyError> {
        let private_key = RsaPrivateKey::from_pkcs8_pem(pem).map_err(KeyError::Decode)?;
        Self::from_private(private_key)
    }

    pub fn load_from_pem_file(path: &Path) -> Result<Self, KeyError> {
        let pem = fs::read_to_string(path).map_err(|source| KeyError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_pkcs8_pem(&pem)
    }

    pub fn to_pkcs8_pem(&self) -> Result<String, KeyError> {
        self.private_key
            .to_pkcs8_pem(LineEnding::LF)
            .map(|s| s.to_string())
            .map_err(KeyError::Encode)
    }

    pub fn write_pem_file(&self, path: &Path) -> Result<(), KeyError> {
        let pem = self.to_pkcs8_pem()?;
        fs::write(path, pem).map_err(|source| KeyError::Io {
            path: path.display().to_string(),
            source,
        })
    }

    pub const fn public_key(&self) -> &RsaPublicKey {
        &self.public_key
    }

    pub const fn private_key(&self) -> &RsaPrivateKey {
        &self.private_key
    }

    pub fn kid(&self) -> &str {
        &self.kid
    }

    pub fn jwk(&self) -> Jwk {
        Jwk::from_rsa_public_key(&self.public_key, self.kid.clone())
    }

    pub fn jwks(&self) -> Jwks {
        Jwks {
            keys: vec![self.jwk()],
        }
    }

    fn from_private(private_key: RsaPrivateKey) -> Result<Self, KeyError> {
        let public_key = RsaPublicKey::from(&private_key);
        let kid = compute_kid(&public_key)?;
        Ok(Self {
            private_key,
            public_key,
            kid,
        })
    }
}

pub fn compute_kid(public_key: &RsaPublicKey) -> Result<String, KeyError> {
    let der = public_key
        .to_public_key_der()
        .map_err(KeyError::EncodeSpki)?;
    let digest = Sha256::digest(der.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(digest))
}
