//! Sealing for the current user on macOS: AES-256-GCM under a random key the
//! login keychain holds for this app as a generic password item.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::{fmt, io};

use aws_lc_rs::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce};
use aws_lc_rs::error::Unspecified;
use aws_lc_rs::rand;
use parking_lot::Mutex;
use zeroize::{Zeroize as _, Zeroizing};

use super::sealing_key::{self, KEY_ABSENT, KeyError};
use super::status::unavailable;
use crate::secure::{Sealer, SecureError};

const MAGIC: &[u8; 6] = b"spkc2\0";
const HEADER_LEN: usize = MAGIC.len() + NONCE_LEN;
const PROBE: &[u8] = b"sp-secure availability probe";

/// Seals with an AES-256-GCM key the login keychain holds for this app.
///
/// The key's item is named from the app id, so each brand and channel has its
/// own key and opens only its own ciphertexts; the label is also the
/// authenticated data. The key is generated on the first seal and read once
/// per process. Its item trusts the creating binary: a rebuild with another
/// code signature reads it after the user allows that build once.
pub struct KeychainSealer {
    label: String,
    key: Mutex<Option<LessSafeKey>>,
}

impl fmt::Debug for KeychainSealer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeychainSealer")
            .field("label", &self.label)
            .field("key_loaded", &self.key.lock().is_some())
            .finish()
    }
}

impl KeychainSealer {
    #[must_use]
    pub fn new(app_id: &str) -> Self {
        Self {
            label: format!("{app_id} sealing key"),
            key: Mutex::new(None),
        }
    }

    pub fn delete_key(&self) -> Result<(), SecureError> {
        let mut cached = self.key.lock();
        sealing_key::delete(&self.label).map_err(unavailable)?;
        *cached = None;
        drop(cached);
        Ok(())
    }

    fn aad(&self) -> Aad<&[u8]> {
        Aad::from(self.label.as_bytes())
    }

    fn sealing_with<T>(&self, operate: impl FnOnce(&LessSafeKey) -> T) -> Result<T, SecureError> {
        ensured_key(&mut self.key.lock(), &self.label)
            .map(operate)
            .map_err(KeyError::sealing)
    }

    fn opening_with<T>(&self, operate: impl FnOnce(&LessSafeKey) -> T) -> Result<T, SecureError> {
        existing_key(&mut self.key.lock(), &self.label)
            .map_err(KeyError::opening)?
            .map(operate)
            .ok_or_else(|| SecureError::rejected(KEY_ABSENT))
    }
}

fn ensured_key<'slot>(
    slot: &'slot mut Option<LessSafeKey>,
    label: &str,
) -> Result<&'slot LessSafeKey, KeyError> {
    let key = match slot.take() {
        Some(key) => key,
        None => sealing_key::load_or_create(label)?,
    };
    Ok(slot.insert(key))
}

fn existing_key<'slot>(
    slot: &'slot mut Option<LessSafeKey>,
    label: &str,
) -> Result<Option<&'slot LessSafeKey>, KeyError> {
    if slot.is_none() {
        *slot = sealing_key::load(label)?;
    }
    Ok(slot.as_ref())
}

fn unavailable_crypto(error: Unspecified) -> SecureError {
    SecureError::Unavailable {
        source: io::Error::other(error),
    }
}

impl Sealer for KeychainSealer {
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, SecureError> {
        let mut nonce = [0u8; NONCE_LEN];
        rand::fill(&mut nonce).map_err(unavailable_crypto)?;
        let mut sealed = Vec::with_capacity(HEADER_LEN + plaintext.len() + AES_256_GCM.tag_len());
        sealed.extend_from_slice(MAGIC);
        sealed.extend_from_slice(&nonce);
        sealed.extend_from_slice(plaintext);
        let (_, body) = sealed.split_at_mut(HEADER_LEN);
        let aad = self.aad();
        let tag = self
            .sealing_with(|key| {
                key.seal_in_place_separate_tag(Nonce::assume_unique_for_key(nonce), aad, body)
            })
            .and_then(|sealing| sealing.map_err(unavailable_crypto));
        match tag {
            Ok(tag) => {
                sealed.extend_from_slice(tag.as_ref());
                Ok(sealed)
            },
            Err(error) => {
                sealed.zeroize();
                Err(error)
            },
        }
    }

    fn open(&self, ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, SecureError> {
        if ciphertext.is_empty() {
            return Err(SecureError::Empty);
        }
        let Some(sealed) = ciphertext.strip_prefix(MAGIC.as_slice()) else {
            return Err(SecureError::rejected("not a keychain-sealed ciphertext"));
        };
        let Some((nonce, body)) = sealed.split_first_chunk::<NONCE_LEN>() else {
            return Err(SecureError::rejected(
                "keychain-sealed ciphertext is truncated",
            ));
        };
        let mut opened = Zeroizing::new(body.to_vec());
        let aad = self.aad();
        let plaintext_len = self
            .opening_with(|key| {
                key.open_in_place(Nonce::assume_unique_for_key(*nonce), aad, &mut opened)
                    .map(|plaintext| plaintext.len())
            })?
            .map_err(|error| SecureError::Rejected {
                source: io::Error::new(io::ErrorKind::InvalidData, error),
            })?;
        opened.truncate(plaintext_len);
        Ok(opened)
    }

    fn available(&self) -> bool {
        let sealed = match self.seal(PROBE) {
            Ok(sealed) => sealed,
            Err(e) => {
                tracing::warn!(error = %e, "the keychain cannot seal for this user");
                return false;
            },
        };
        match self.open(&sealed) {
            Ok(opened) => opened.as_slice() == PROBE,
            Err(e) => {
                tracing::warn!(error = %e, "the keychain cannot open what it sealed for this user");
                false
            },
        }
    }
}
