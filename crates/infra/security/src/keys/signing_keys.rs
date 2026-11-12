//! The deployment's signing keys: the RS256 token key and the Ed25519
//! manifest key.
//!
//! [`SigningKeys`] is built once at the composition root and handed to every
//! token minter, token verifier, JWKS publisher and manifest signer. The RSA
//! key is resolved from the `signing_key_pem` secret first (the cloud path,
//! where the filesystem is read-only and the PEM is injected as an env
//! secret), falling back to the file at `signing_key_path` (the local-dev
//! path). The manifest key is derived from the 32-byte seed loaded by
//! [`systemprompt_config::SecretsService`].
//!
//! Token-minting paths use [`SigningKeys::encoding_key`] /
//! [`SigningKeys::active_kid`] to produce RS256 JWTs whose `kid` matches the
//! JWKS this deployment publishes at `/.well-known/jwks.json`.
//! Token-verifying paths use [`SigningKeys::decoding_key_for_kid`] to look up
//! the public half by `kid`. Besides the active key the set carries every
//! retired, verify-only key with its own expiry, so a rotation does not
//! invalidate outstanding tokens: tokens a retired key signed keep verifying,
//! and the JWKS keeps publishing it, until that instant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use ed25519_dalek::{Signer, SigningKey as ManifestKey};
use jsonwebtoken::{DecodingKey, EncodingKey};
use rsa::pkcs1::EncodeRsaPrivateKey;
use thiserror::Error;

use systemprompt_config::PreviousSigningKey;

use crate::keys::verification::{RetiredJwk, TokenVerificationKeys, VerificationKeyring};
use crate::keys::{Jwk, Jwks, KeyError, RsaSigningKey, compute_kid, public_key_from_spki_pem};

#[derive(Debug, Error)]
pub enum SigningKeysError {
    #[error("signing_key_path is not configured")]
    PathMissing,

    #[error("signing key file not found at {0}")]
    FileMissing(PathBuf),

    #[error("signing key secret unavailable: {0}")]
    Secret(#[source] systemprompt_config::SecretsBootstrapError),

    #[error("previous signing key secret unavailable: {0}")]
    PreviousSecret(#[source] systemprompt_config::SecretsBootstrapError),

    #[error("previous signing key '{stored}' does not match its public key (kid {computed})")]
    PreviousKidMismatch { stored: String, computed: String },

    #[error("no signing key is staged: run `admin keys stage` and restart every replica first")]
    NothingStaged,

    #[error("signing key {kid} is already staged: promote it before staging another")]
    AlreadyStaged { kid: String },

    #[error("manifest signing seed unavailable: {0}")]
    ManifestSeed(#[source] systemprompt_config::SecretsBootstrapError),

    #[error("key load failed: {0}")]
    Key(#[from] KeyError),

    #[error("RSA DER encoding failed: {0}")]
    Pkcs1Encode(#[source] rsa::pkcs1::Error),

    #[error("verification key could not be built: {0}")]
    VerificationKey(#[source] jsonwebtoken::errors::Error),
}

pub type SigningKeysResult<T> = Result<T, SigningKeysError>;

struct Inner {
    rsa: RsaSigningKey,
    encoding_key: EncodingKey,
    keyring: VerificationKeyring,
    manifest: ManifestKey,
}

#[derive(Clone)]
pub struct SigningKeys {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for SigningKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigningKeys")
            .field("kid", &self.inner.rsa.kid())
            .field(
                "retired_kids",
                &self
                    .inner
                    .keyring
                    .keys()
                    .retired
                    .iter()
                    .map(|retired| retired.jwk.kid.as_str())
                    .collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl SigningKeys {
    pub fn new(rsa: RsaSigningKey, manifest_seed: &[u8; 32]) -> SigningKeysResult<Self> {
        Self::with_verification_keys(rsa, None, Vec::new(), manifest_seed)
    }

    pub fn resolve(
        signing_key_path: &Path,
        secrets: &systemprompt_config::SecretsService,
    ) -> SigningKeysResult<Self> {
        let rsa = match secrets
            .signing_key_pem()
            .map_err(SigningKeysError::Secret)?
        {
            Some(pem) => RsaSigningKey::from_pkcs8_pem(&pem)?,
            None => load_from_file(signing_key_path)?,
        };
        let staged = secrets
            .signing_key_staged_pem()
            .map_err(SigningKeysError::Secret)?
            .map(|pem| RsaSigningKey::from_pkcs8_pem(&pem).map(|key| key.jwk()))
            .transpose()?;
        let retired = secrets
            .signing_keys_previous()
            .map_err(SigningKeysError::PreviousSecret)?
            .iter()
            .map(retired_jwk)
            .collect::<SigningKeysResult<Vec<_>>>()?;
        let seed = secrets
            .manifest_signing_secret_seed()
            .map_err(SigningKeysError::ManifestSeed)?;
        Self::with_verification_keys(rsa, staged, retired, &seed)
    }

    pub fn with_verification_keys(
        rsa: RsaSigningKey,
        staged: Option<Jwk>,
        retired: Vec<RetiredJwk>,
        manifest_seed: &[u8; 32],
    ) -> SigningKeysResult<Self> {
        let der = rsa
            .private_key()
            .to_pkcs1_der()
            .map_err(SigningKeysError::Pkcs1Encode)?;
        let encoding_key = EncodingKey::from_rsa_der(der.as_bytes());
        let retired = retired
            .into_iter()
            .filter(|retired| retired.jwk.kid != rsa.kid())
            .collect();
        let staged = staged.filter(|staged| staged.kid != rsa.kid());
        let keyring = VerificationKeyring::new(TokenVerificationKeys {
            active: rsa.jwk(),
            staged,
            retired,
        })
        .map_err(SigningKeysError::VerificationKey)?;
        Ok(Self {
            inner: Arc::new(Inner {
                rsa,
                encoding_key,
                keyring,
                manifest: ManifestKey::from_bytes(manifest_seed),
            }),
        })
    }

    #[must_use]
    pub fn signing_key(&self) -> &RsaSigningKey {
        &self.inner.rsa
    }

    #[must_use]
    pub fn encoding_key(&self) -> &EncodingKey {
        &self.inner.encoding_key
    }

    #[must_use]
    pub fn active_kid(&self) -> &str {
        self.inner.rsa.kid()
    }

    #[must_use]
    pub fn keyring(&self) -> &VerificationKeyring {
        &self.inner.keyring
    }

    #[must_use]
    pub fn verification_keys(&self) -> &TokenVerificationKeys {
        self.inner.keyring.keys()
    }

    #[must_use]
    pub fn jwks(&self) -> Jwks {
        self.inner.keyring.jwks()
    }

    #[must_use]
    pub fn decoding_key_for_kid(&self, kid: &str) -> Option<&DecodingKey> {
        self.inner.keyring.decoding_key_for_kid(kid)
    }

    #[must_use]
    pub fn sign_manifest_bytes(&self, payload: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD
            .encode(self.inner.manifest.sign(payload).to_bytes())
    }

    #[must_use]
    pub fn manifest_pubkey_b64(&self) -> String {
        base64::engine::general_purpose::STANDARD
            .encode(self.inner.manifest.verifying_key().to_bytes())
    }
}

pub fn retired_jwk(previous: &PreviousSigningKey) -> SigningKeysResult<RetiredJwk> {
    let public_key = public_key_from_spki_pem(&previous.public_pem)?;
    let kid = compute_kid(&public_key)?;
    if kid != previous.kid {
        return Err(SigningKeysError::PreviousKidMismatch {
            stored: previous.kid.clone(),
            computed: kid,
        });
    }
    Ok(RetiredJwk {
        jwk: Jwk::from_rsa_public_key(&public_key, kid),
        expires_at: previous.expires_at,
    })
}

fn load_from_file(path: &Path) -> SigningKeysResult<RsaSigningKey> {
    if path.as_os_str().is_empty() {
        return Err(SigningKeysError::PathMissing);
    }
    if !path.exists() {
        return Err(SigningKeysError::FileMissing(path.to_path_buf()));
    }
    Ok(RsaSigningKey::load_from_pem_file(path)?)
}
