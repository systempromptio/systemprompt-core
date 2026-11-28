//! The sealing key: 32 random bytes for AES-256-GCM, kept as a generic
//! password item of the login keychain under the app's label, created on the
//! first seal and never replaced.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;

use aws_lc_rs::aead::{AES_256_GCM, LessSafeKey, UnboundKey};
use aws_lc_rs::error::Unspecified;
use aws_lc_rs::rand;
use security_framework::base::Error;
use zeroize::Zeroizing;

use super::passwords::{Insertion, add, read, remove};
use super::status::{keystore, unavailable};
use crate::secure::SecureError;

const ACCOUNT: &str = "aes-256-gcm";
const KEY_LEN: usize = 32;

pub(super) enum KeyError {
    Keychain(Error),
    Malformed,
    Random(Unspecified),
    Contended,
}

impl From<Error> for KeyError {
    fn from(error: Error) -> Self {
        Self::Keychain(error)
    }
}

impl KeyError {
    pub(super) fn sealing(self) -> SecureError {
        match self {
            Self::Keychain(error) => keystore(error),
            Self::Malformed => SecureError::Keystore {
                source: io::Error::new(io::ErrorKind::InvalidData, MALFORMED),
            },
            Self::Random(error) => SecureError::Unavailable {
                source: io::Error::other(error),
            },
            Self::Contended => SecureError::Unavailable {
                source: io::Error::other("the sealing key was removed while it was being created"),
            },
        }
    }

    pub(super) fn opening(self) -> SecureError {
        match self {
            Self::Keychain(error) => unavailable(error),
            Self::Malformed => SecureError::rejected(MALFORMED),
            Self::Random(error) => SecureError::Unavailable {
                source: io::Error::other(error),
            },
            Self::Contended => SecureError::rejected(KEY_ABSENT),
        }
    }
}

const MALFORMED: &str = "the keychain's sealing key is not a 256-bit key";
pub(super) const KEY_ABSENT: &str = "this app's sealing key is not in the keychain";

pub(super) fn load(label: &str) -> Result<Option<LessSafeKey>, KeyError> {
    read(label, ACCOUNT)?
        .map(|bytes| aead_key(&bytes))
        .transpose()
}

pub(super) fn load_or_create(label: &str) -> Result<LessSafeKey, KeyError> {
    if let Some(key) = load(label)? {
        return Ok(key);
    }
    let mut fresh = Zeroizing::new([0u8; KEY_LEN]);
    rand::fill(fresh.as_mut_slice()).map_err(KeyError::Random)?;
    match add(label, ACCOUNT, fresh.as_slice())? {
        Insertion::Added => aead_key(fresh.as_slice()),
        Insertion::AlreadyPresent => load(label)?.ok_or(KeyError::Contended),
    }
}

pub(super) fn delete(label: &str) -> Result<(), Error> {
    remove(label, ACCOUNT)
}

fn aead_key(bytes: &[u8]) -> Result<LessSafeKey, KeyError> {
    if bytes.len() != KEY_LEN {
        return Err(KeyError::Malformed);
    }
    UnboundKey::new(&AES_256_GCM, bytes)
        .map(LessSafeKey::new)
        .map_err(|_unspecified| KeyError::Malformed)
}
