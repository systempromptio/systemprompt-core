//! Process-wide RS256 signing-key authority.
//!
//! Loads the RSA private key once and caches it in a `OnceLock`. The key is
//! resolved from the `signing_key_pem` secret first (the cloud path, where the
//! filesystem is read-only and the PEM is injected as an env secret), falling
//! back to the file at `signing_key_path` (the local-dev path). [`init`] is
//! called at bootstrap so a missing key fails startup rather than the first
//! request; the lazy first-use fallback uses the same resolution.
//! Token-minting paths use [`encoding_key`] / [`active_kid`] to produce RS256
//! JWTs whose `kid` matches the JWKS this deployment publishes at
//! `/.well-known/jwks.json`. Token-verifying paths use [`decoding_key_for_kid`]
//! to look up the public half by `kid`.

use std::path::PathBuf;
use std::sync::OnceLock;

use jsonwebtoken::{DecodingKey, EncodingKey};
use rsa::pkcs1::{EncodeRsaPrivateKey, EncodeRsaPublicKey};
use thiserror::Error;

use crate::keys::{KeyError, RsaSigningKey};

#[derive(Debug, Error)]
pub enum TokenAuthorityError {
    #[error("signing_key_path is not configured")]
    PathMissing,

    #[error("signing key file not found at {0}")]
    FileMissing(PathBuf),

    #[error("config unavailable: {0}")]
    Config(String),

    #[error("signing key secret unavailable: {0}")]
    Secret(String),

    #[error("key load failed: {0}")]
    Key(#[from] KeyError),

    #[error("jwt key conversion failed: {0}")]
    KeyConvert(#[source] jsonwebtoken::errors::Error),

    #[error("RSA DER encoding failed: {0}")]
    Pkcs1Encode(#[source] rsa::pkcs1::Error),
}

pub type TokenAuthorityResult<T> = Result<T, TokenAuthorityError>;

#[expect(
    clippy::struct_field_names,
    reason = "all fields are RSA-derived keys; the `_key` suffix distinguishes their format"
)]
pub(crate) struct Authority {
    signing_key: RsaSigningKey,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

static CELL: OnceLock<Authority> = OnceLock::new();

pub fn init() -> TokenAuthorityResult<()> {
    if CELL.get().is_some() {
        return Ok(());
    }
    let authority = load_from_secret_or_file()?;
    drop(CELL.set(authority));
    Ok(())
}

fn load_from_secret_or_file() -> TokenAuthorityResult<Authority> {
    if let Some(pem) = systemprompt_config::SecretsBootstrap::signing_key_pem()
        .map_err(|e| TokenAuthorityError::Secret(e.to_string()))?
    {
        let signing_key = RsaSigningKey::from_pkcs8_pem(&pem)?;
        return build(signing_key);
    }
    load()
}

fn load() -> TokenAuthorityResult<Authority> {
    let config = systemprompt_models::Config::get()
        .map_err(|e| TokenAuthorityError::Config(e.to_string()))?;
    let path = &config.signing_key_path;
    if path.as_os_str().is_empty() {
        return Err(TokenAuthorityError::PathMissing);
    }
    if !path.exists() {
        return Err(TokenAuthorityError::FileMissing(path.clone()));
    }
    let signing_key = RsaSigningKey::load_from_pem_file(path)?;
    build(signing_key)
}

pub(crate) fn build(signing_key: RsaSigningKey) -> TokenAuthorityResult<Authority> {
    let der = signing_key
        .private_key()
        .to_pkcs1_der()
        .map_err(TokenAuthorityError::Pkcs1Encode)?;
    let encoding_key = EncodingKey::from_rsa_der(der.as_bytes());
    let pub_der = signing_key
        .public_key()
        .to_pkcs1_der()
        .map_err(TokenAuthorityError::Pkcs1Encode)?;
    let decoding_key = DecodingKey::from_rsa_der(pub_der.as_bytes());
    Ok(Authority {
        signing_key,
        encoding_key,
        decoding_key,
    })
}

fn authority() -> TokenAuthorityResult<&'static Authority> {
    if let Some(a) = CELL.get() {
        return Ok(a);
    }
    let a = load_from_secret_or_file()?;
    drop(CELL.set(a));
    CELL.get().ok_or(TokenAuthorityError::PathMissing)
}

pub fn signing_key() -> TokenAuthorityResult<&'static RsaSigningKey> {
    Ok(&authority()?.signing_key)
}

pub fn encoding_key() -> TokenAuthorityResult<&'static EncodingKey> {
    Ok(&authority()?.encoding_key)
}

pub fn active_kid() -> TokenAuthorityResult<&'static str> {
    Ok(authority()?.signing_key.kid())
}

pub fn decoding_key_for_kid(kid: &str) -> TokenAuthorityResult<Option<&'static DecodingKey>> {
    let a = authority()?;
    if a.signing_key.kid() == kid {
        Ok(Some(&a.decoding_key))
    } else {
        Ok(None)
    }
}

pub fn decoding_key() -> TokenAuthorityResult<&'static DecodingKey> {
    Ok(&authority()?.decoding_key)
}

#[doc(hidden)]
pub fn install_for_test(key: RsaSigningKey) {
    if CELL.get().is_some() {
        return;
    }
    if let Ok(a) = build(key) {
        drop(CELL.set(a));
    }
}
