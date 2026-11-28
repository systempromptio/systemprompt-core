//! Sealing for the current user on Linux: AES-256-GCM under a random key a
//! keyring holds for this app, or under a key of this process where no
//! keyring is usable.
//!
//! A ciphertext starts with the tier that sealed it, so it is opened with
//! that tier's key and no other: one sealed under the Secret Service key is
//! kept, not rejected, while a later process runs without a desktop session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::{fmt, io};

use aws_lc_rs::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce};
use aws_lc_rs::rand;
use parking_lot::Mutex;
use zeroize::{Zeroize as _, Zeroizing};

use super::keyrings::{Keyring, keeps_ciphertext};
use super::sealing_key;
use crate::secure::{Sealer, SecureError};

const MAGIC_LEN: usize = 6;
const HEADER_LEN: usize = MAGIC_LEN + NONCE_LEN;
const PROBE: &[u8] = b"sp-secure availability probe";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier {
    SecretService,
    Kernel,
    Process,
}

impl Tier {
    const ALL: [Self; 3] = [Self::SecretService, Self::Kernel, Self::Process];

    const fn of(keyring: Keyring) -> Self {
        match keyring {
            Keyring::SecretService => Self::SecretService,
            Keyring::Kernel => Self::Kernel,
        }
    }

    const fn magic(self) -> &'static [u8; MAGIC_LEN] {
        match self {
            Self::SecretService => b"spss1\0",
            Self::Kernel => b"spkk1\0",
            Self::Process => b"sppm1\0",
        }
    }

    const fn keyring(self) -> Option<Keyring> {
        match self {
            Self::SecretService => Some(Keyring::SecretService),
            Self::Kernel => Some(Keyring::Kernel),
            Self::Process => None,
        }
    }

    const fn key_absent(self) -> &'static str {
        match self {
            Self::SecretService => "this app's sealing key is not in the Secret Service",
            Self::Kernel => "this app's sealing key is not in the kernel keyring",
            Self::Process => "sealed in the memory of another process",
        }
    }

    fn split(ciphertext: &[u8]) -> Option<(Self, &[u8])> {
        Self::ALL.into_iter().find_map(|tier| {
            ciphertext
                .strip_prefix(tier.magic().as_slice())
                .map(|sealed| (tier, sealed))
        })
    }
}

#[derive(Default)]
struct Keys {
    secret_service: Option<LessSafeKey>,
    kernel: Option<LessSafeKey>,
    process: Option<LessSafeKey>,
    sealing: Option<Tier>,
}

impl Keys {
    const fn slot(&mut self, tier: Tier) -> &mut Option<LessSafeKey> {
        match tier {
            Tier::SecretService => &mut self.secret_service,
            Tier::Kernel => &mut self.kernel,
            Tier::Process => &mut self.process,
        }
    }

    fn sealing(&mut self, label: &str) -> Result<(Tier, &LessSafeKey), SecureError> {
        let tier = match self.sealing {
            Some(tier) => tier,
            None => self.choose(label)?,
        };
        self.sealing = Some(tier);
        self.slot(tier)
            .as_ref()
            .map(|key| (tier, key))
            .ok_or_else(|| SecureError::Unavailable {
                source: io::Error::other(tier.key_absent()),
            })
    }

    fn choose(&mut self, label: &str) -> Result<Tier, SecureError> {
        for keyring in Keyring::PREFERRED {
            let created = keyring
                .open()
                .and_then(|store| sealing_key::load_or_create(&store, label));
            match created {
                Ok(key) => {
                    let tier = Tier::of(keyring);
                    *self.slot(tier) = Some(key);
                    return Ok(tier);
                },
                Err(error) => tracing::warn!(
                    error = %error,
                    keyring = keyring.name(),
                    "this keyring cannot hold the sealing key"
                ),
            }
        }
        tracing::warn!(
            "no keyring is usable; secrets are sealed for this process only and a sign-in ends with it"
        );
        self.process = Some(sealing_key::for_this_process()?);
        Ok(Tier::Process)
    }

    fn opening(&mut self, tier: Tier, label: &str) -> Result<&LessSafeKey, SecureError> {
        if self.slot(tier).is_none()
            && let Some(keyring) = tier.keyring()
        {
            *self.slot(tier) = keyring
                .open()
                .and_then(|store| sealing_key::load(&store, label))
                .map_err(keeps_ciphertext)?;
        }
        self.slot(tier)
            .as_ref()
            .ok_or_else(|| SecureError::rejected(tier.key_absent()))
    }
}

/// Seals with an AES-256-GCM key the best keyring of the machine holds.
///
/// That is the Secret Service where the session has one, else the kernel
/// keyring, else no keyring at all: a key that lives only in this process.
/// The key's item is named from the app id, so each brand and channel has its
/// own key and opens only its own ciphertexts; the label is also the
/// authenticated data. The tier is chosen on the first seal and kept for the
/// process. A secret sealed under the kernel keyring opens until the machine
/// restarts; one sealed under the process key opens only in the process that
/// sealed it.
pub struct KeyringSealer {
    label: String,
    keys: Mutex<Keys>,
}

impl fmt::Debug for KeyringSealer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyringSealer")
            .field("label", &self.label)
            .field("sealing", &self.keys.lock().sealing)
            .finish_non_exhaustive()
    }
}

impl KeyringSealer {
    #[must_use]
    pub fn new(app_id: &str) -> Self {
        Self {
            label: format!("{app_id} sealing key"),
            keys: Mutex::new(Keys::default()),
        }
    }

    pub fn delete_key(&self) -> Result<(), SecureError> {
        let mut keys = self.keys.lock();
        for keyring in Keyring::PREFERRED {
            if let Ok(store) = keyring.open() {
                sealing_key::delete(&store, &self.label)?;
            }
        }
        *keys = Keys::default();
        drop(keys);
        Ok(())
    }

    fn aad(&self) -> Aad<&[u8]> {
        Aad::from(self.label.as_bytes())
    }
}

fn unavailable_crypto(error: aws_lc_rs::error::Unspecified) -> SecureError {
    SecureError::Unavailable {
        source: io::Error::other(error),
    }
}

impl Sealer for KeyringSealer {
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, SecureError> {
        let mut nonce = [0u8; NONCE_LEN];
        rand::fill(&mut nonce).map_err(unavailable_crypto)?;
        let mut keys = self.keys.lock();
        let (tier, key) = keys.sealing(&self.label)?;
        let mut sealed = Vec::with_capacity(HEADER_LEN + plaintext.len() + AES_256_GCM.tag_len());
        sealed.extend_from_slice(tier.magic());
        sealed.extend_from_slice(&nonce);
        sealed.extend_from_slice(plaintext);
        let (_, body) = sealed.split_at_mut(HEADER_LEN);
        let tag =
            key.seal_in_place_separate_tag(Nonce::assume_unique_for_key(nonce), self.aad(), body);
        drop(keys);
        match tag {
            Ok(tag) => {
                sealed.extend_from_slice(tag.as_ref());
                Ok(sealed)
            },
            Err(error) => {
                sealed.zeroize();
                Err(unavailable_crypto(error))
            },
        }
    }

    fn open(&self, ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, SecureError> {
        if ciphertext.is_empty() {
            return Err(SecureError::Empty);
        }
        let Some((tier, sealed)) = Tier::split(ciphertext) else {
            return Err(SecureError::rejected("not a keyring-sealed ciphertext"));
        };
        let Some((nonce, body)) = sealed.split_first_chunk::<NONCE_LEN>() else {
            return Err(SecureError::rejected(
                "keyring-sealed ciphertext is truncated",
            ));
        };
        let mut opened = Zeroizing::new(body.to_vec());
        let mut keys = self.keys.lock();
        let plaintext_len = keys
            .opening(tier, &self.label)?
            .open_in_place(
                Nonce::assume_unique_for_key(*nonce),
                self.aad(),
                opened.as_mut_slice(),
            )
            .map(|plaintext| plaintext.len())
            .map_err(|error| SecureError::Rejected {
                source: io::Error::new(io::ErrorKind::InvalidData, error),
            })?;
        drop(keys);
        opened.truncate(plaintext_len);
        Ok(opened)
    }

    fn available(&self) -> bool {
        let sealed = match self.seal(PROBE) {
            Ok(sealed) => sealed,
            Err(e) => {
                tracing::warn!(error = %e, "no key can seal for this user");
                return false;
            },
        };
        match self.open(&sealed) {
            Ok(opened) => opened.as_slice() == PROBE,
            Err(e) => {
                tracing::warn!(error = %e, "the sealing key cannot open what it sealed");
                false
            },
        }
    }
}
