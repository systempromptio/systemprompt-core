//! At-rest hashing for secrets that must be looked up by exact match but
//! must not survive a database read.
//!
//! The deployment's `oauth_at_rest_pepper` is a process-resident HMAC key.
//! Refresh-token identifiers and authorisation codes are stored as the
//! lowercase-hex HMAC-SHA-256 of the raw value under that key, so a leaked
//! database backup or replica snapshot yields opaque digests rather than
//! live credentials.
//!
//! Where a stored secret has to be replayed upstream rather than merely
//! matched, a digest is no use and
//! [`AtRestCipher::seal`]/[`AtRestCipher::open`] encrypt it instead:
//! ChaCha20-Poly1305 under a key derived for the member that owns the column,
//! HMAC-SHA-256(`encryption_master_key`, `systemprompt.member-cipher.v1` NUL
//! member id NUL). Every sealed value carries the `spmc1.` envelope followed by
//! the hex nonce and ciphertext. Core members and install members seal the same
//! way; an install's conferred cipher is the same derivation for its own id, so
//! no member opens another's values and nothing is sealed under the master key
//! itself.
//!
//! A value without the envelope was sealed under the master key directly by a
//! release before 0.88. [`AtRestCipher::open`] refuses it with
//! [`AtRestCipherError::Unresealed`], which names the command that moves it;
//! the one opener of that scheme is [`pre_088::open`], read only by
//! `systemprompt admin secrets reseal`. The gateway journal keys its segments
//! with [`member_key`] under its own label.
//!
//! Rotation is out of scope here: rolling the pepper invalidates every
//! row hashed under the old key, and rolling the master key invalidates every
//! sealed value. The schema reserves no `pepper_version` column today; a
//! future migration would add one if graceful rotation is required.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use systemprompt_identifiers::ExtensionId;
use systemprompt_traits::MEMBER_CIPHER_ENVELOPE;

type HmacSha256 = Hmac<Sha256>;

const NONCE_BYTES: usize = 12;

/// Why a sealed value could not be produced or read back. Every variant is an
/// operator or key fault; none of them carries the value or the key.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum AtRestCipherError {
    #[error(
        "at-rest encryption requires the `encryption_master_key` secret (32 bytes as 64 hex \
         characters); with `secrets.source: env` it must also be listed in \
         SYSTEMPROMPT_CUSTOM_SECRETS"
    )]
    KeyUnavailable,

    #[error("encryption_master_key is not a 32-byte hex key")]
    KeyInvalid,

    #[error("sealed value is malformed")]
    Malformed,

    #[error("sealed value did not authenticate under its member's key")]
    NotAuthentic,

    #[error(
        "sealed value predates 0.88 (no member envelope); run `systemprompt admin secrets reseal \
         --member <owning member> --apply`"
    )]
    Unresealed,
}

pub const MEMBER_CIPHER_LABEL: &[u8] = b"systemprompt.member-cipher.v1";

/// The deployment's `encryption_master_key`, read from the secret store.
///
/// The one way to hold it is [`MasterKey::from_store`]: no derivation in this
/// module takes key bytes, so a crate that obtained bytes some other way has
/// no entry point here. The raw key never enters a child's environment (a
/// service cannot grant it), so a child or install process holds none.
#[derive(Clone)]
pub struct MasterKey(std::sync::Arc<[u8; 32]>);

impl std::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MasterKey(<redacted>)")
    }
}

impl MasterKey {
    pub fn from_store(
        store: &dyn systemprompt_traits::SecretStore,
    ) -> Result<Self, systemprompt_traits::SecretStoreError> {
        Ok(Self(std::sync::Arc::new(store.encryption_master_key()?)))
    }

    pub fn derive(&self, label: &[u8], member: &ExtensionId) -> Result<[u8; 32], AtRestHashError> {
        member_key(&self.0, label, member)
    }

    pub(crate) fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

pub(crate) fn member_key(
    master_key: &[u8; 32],
    label: &[u8],
    member: &ExtensionId,
) -> Result<[u8; 32], AtRestHashError> {
    let mut mac = keyed_hmac(master_key)?;
    mac.update(label);
    mac.update(&[0]);
    mac.update(member.as_str().as_bytes());
    mac.update(&[0]);
    Ok(mac.finalize().into_bytes().into())
}

/// One member's at-rest AEAD, keyed from `encryption_master_key` for that
/// member.
///
/// A host that does not hold the key (a child host the key was not granted
/// to) holds a [`withheld`](Self::withheld) cipher: every seal and open
/// answers [`AtRestCipherError::KeyUnavailable`].
#[derive(Clone)]
pub struct AtRestCipher {
    key: Option<std::sync::Arc<[u8; 32]>>,
}

impl std::fmt::Debug for AtRestCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AtRestCipher(<redacted>)")
    }
}

impl AtRestCipher {
    pub fn for_member(
        master_key: &MasterKey,
        member: &ExtensionId,
    ) -> Result<Self, AtRestHashError> {
        Ok(Self {
            key: Some(std::sync::Arc::new(member_key(
                master_key.bytes(),
                MEMBER_CIPHER_LABEL,
                member,
            )?)),
        })
    }

    #[must_use]
    pub const fn withheld() -> Self {
        Self { key: None }
    }

    fn aead(&self) -> Result<ChaCha20Poly1305, AtRestCipherError> {
        let key = self.key.as_ref().ok_or(AtRestCipherError::KeyUnavailable)?;
        Ok(ChaCha20Poly1305::new(&(**key).into()))
    }

    pub fn seal(&self, plaintext: &str) -> Result<String, AtRestCipherError> {
        self.seal_bytes(plaintext.as_bytes())
    }

    pub fn seal_bytes(&self, plaintext: &[u8]) -> Result<String, AtRestCipherError> {
        let nonce: [u8; NONCE_BYTES] = rand::random();
        let sealed = self
            .aead()?
            .encrypt(&Nonce::from(nonce), plaintext)
            .map_err(|_e| AtRestCipherError::NotAuthentic)?;
        let mut out = Vec::with_capacity(NONCE_BYTES + sealed.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&sealed);
        Ok(format!("{MEMBER_CIPHER_ENVELOPE}{}", hex::encode(out)))
    }

    pub fn open(&self, sealed: &str) -> Result<String, AtRestCipherError> {
        String::from_utf8(self.open_bytes(sealed)?).map_err(|_e| AtRestCipherError::Malformed)
    }

    pub fn open_bytes(&self, sealed: &str) -> Result<Vec<u8>, AtRestCipherError> {
        let sealed = sealed
            .strip_prefix(MEMBER_CIPHER_ENVELOPE)
            .ok_or(AtRestCipherError::Unresealed)?;
        let (nonce, ciphertext) = split_hex(sealed)?;
        open_with(&self.aead()?, &nonce, &ciphertext)
    }
}

fn split_hex(sealed: &str) -> Result<(Vec<u8>, Vec<u8>), AtRestCipherError> {
    let mut bytes = hex::decode(sealed).map_err(|_e| AtRestCipherError::Malformed)?;
    if bytes.len() <= NONCE_BYTES {
        return Err(AtRestCipherError::Malformed);
    }
    let ciphertext = bytes.split_off(NONCE_BYTES);
    Ok((bytes, ciphertext))
}

fn open_with(
    aead: &ChaCha20Poly1305,
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, AtRestCipherError> {
    let nonce: [u8; NONCE_BYTES] = nonce
        .try_into()
        .map_err(|_e| AtRestCipherError::Malformed)?;
    aead.decrypt(&Nonce::from(nonce), ciphertext)
        .map_err(|_e| AtRestCipherError::NotAuthentic)
}

/// The scheme releases before 0.88 sealed with: ChaCha20-Poly1305 directly
/// under `encryption_master_key`. Only `systemprompt admin secrets reseal`
/// reads it; no runtime reader accepts it.
pub mod pre_088 {
    use chacha20poly1305::{ChaCha20Poly1305, KeyInit};

    use super::{AtRestCipherError, MasterKey, open_with, split_hex};

    pub fn open(
        master_key: &MasterKey,
        nonce: &[u8],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, AtRestCipherError> {
        open_with(
            &ChaCha20Poly1305::new(&(*master_key.bytes()).into()),
            nonce,
            ciphertext,
        )
    }

    pub fn open_hex(master_key: &MasterKey, sealed: &str) -> Result<Vec<u8>, AtRestCipherError> {
        let (nonce, ciphertext) = split_hex(sealed)?;
        open(master_key, &nonce, &ciphertext)
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("at-rest pepper was rejected as an HMAC-SHA256 key")]
pub struct AtRestHashError(#[source] hmac::digest::InvalidLength);

pub fn keyed_hmac(key: &[u8]) -> Result<HmacSha256, AtRestHashError> {
    HmacSha256::new_from_slice(key).map_err(AtRestHashError)
}

pub fn hmac_sha256(pepper: &[u8], value: &[u8]) -> Result<[u8; 32], AtRestHashError> {
    let mut mac = keyed_hmac(pepper)?;
    mac.update(value);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    Ok(out)
}

pub fn hmac_sha256_hex(pepper: &[u8], value: &[u8]) -> Result<String, AtRestHashError> {
    hmac_sha256(pepper, value).map(hex::encode)
}
