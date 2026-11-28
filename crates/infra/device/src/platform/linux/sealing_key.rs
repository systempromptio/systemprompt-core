//! The sealing key: 32 random bytes for AES-256-GCM, kept in a keyring under
//! the app's label, created on the first seal and never replaced; or, where no
//! keyring is usable, generated for this process alone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;

use aws_lc_rs::aead::{AES_256_GCM, LessSafeKey, UnboundKey};
use aws_lc_rs::rand;
use zeroize::Zeroizing;

use crate::platform::CredentialStore as _;
use crate::platform::keyring::KeyringStore;
use crate::secure::SecureError;

const ACCOUNT: &str = "aes-256-gcm";
const KEY_LEN: usize = 32;
const MALFORMED: &str = "the keyring's sealing key is not a 256-bit key";

pub(super) fn load(store: &KeyringStore, label: &str) -> Result<Option<LessSafeKey>, SecureError> {
    store
        .load(label, ACCOUNT)?
        .map(|bytes| aead_key(&bytes))
        .transpose()
}

// Why: neither keyring can add an item only when it is absent, so two
// processes sealing for the first time may both write a key; each then reads
// back the one the keyring kept and seals under that.
pub(super) fn load_or_create(
    store: &KeyringStore,
    label: &str,
) -> Result<LessSafeKey, SecureError> {
    if let Some(key) = load(store, label)? {
        return Ok(key);
    }
    let fresh = random_bytes()?;
    store.store(label, ACCOUNT, fresh.as_slice())?;
    load(store, label)?.ok_or_else(|| SecureError::Unavailable {
        source: io::Error::other("the sealing key was removed while it was being created"),
    })
}

pub(super) fn for_this_process() -> Result<LessSafeKey, SecureError> {
    aead_key(random_bytes()?.as_slice())
}

pub(super) fn delete(store: &KeyringStore, label: &str) -> Result<(), SecureError> {
    store.delete(label, ACCOUNT)
}

fn random_bytes() -> Result<Zeroizing<[u8; KEY_LEN]>, SecureError> {
    let mut fresh = Zeroizing::new([0u8; KEY_LEN]);
    rand::fill(fresh.as_mut_slice()).map_err(|error| SecureError::Unavailable {
        source: io::Error::other(error),
    })?;
    Ok(fresh)
}

fn aead_key(bytes: &[u8]) -> Result<LessSafeKey, SecureError> {
    if bytes.len() != KEY_LEN {
        return Err(SecureError::rejected(MALFORMED));
    }
    UnboundKey::new(&AES_256_GCM, bytes)
        .map(LessSafeKey::new)
        .map_err(|_unspecified| SecureError::rejected(MALFORMED))
}
