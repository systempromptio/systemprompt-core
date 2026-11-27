//! The manifest key of an account, as a small state machine over the stored
//! record.
//!
//! A served key is only ever *offered*: the first key of an instance, and any
//! key that differs from the pinned one. `trust` pins the offered key only when
//! its fingerprint is exactly the one the user was shown, so a key offered in
//! between is never trusted by accident. While an offer is pending the account
//! has no trusted key, which stops manifest use until the user decides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::client::ManifestKey;
use sp_protocol::view::{KeyFingerprint, KeyOffer};

use crate::accounts::store::AccountRecord;

pub(crate) fn offer(record: &mut AccountRecord, key: &ManifestKey) -> bool {
    let already_pinned =
        pinned_key(record).is_some_and(|pinned| pinned.fingerprint() == key.fingerprint());
    let next = if already_pinned {
        None
    } else {
        Some(key.to_base64())
    };
    let changed = record.offered_key != next;
    record.offered_key = next;
    changed
}

pub(crate) fn trust(record: &mut AccountRecord, fingerprint: &KeyFingerprint) -> bool {
    let Some(offered) = offered_key(record) else {
        return false;
    };
    if offered.fingerprint() != fingerprint {
        return false;
    }
    record.pinned_key = Some(offered.to_base64());
    record.pinned_fingerprint = Some(offered.fingerprint().clone());
    record.offered_key = None;
    true
}

pub(crate) fn offered_key(record: &AccountRecord) -> Option<ManifestKey> {
    decode(record.offered_key.as_deref()?)
}

pub(crate) fn trusted_key(record: &AccountRecord) -> Option<ManifestKey> {
    if record.offered_key.is_some() {
        return None;
    }
    pinned_key(record)
}

fn pinned_key(record: &AccountRecord) -> Option<ManifestKey> {
    let key = decode(record.pinned_key.as_deref()?)?;
    (record.pinned_fingerprint.as_ref() == Some(key.fingerprint())).then_some(key)
}

pub(crate) fn verification_key(record: &AccountRecord) -> Option<ManifestKey> {
    offered_key(record).or_else(|| trusted_key(record))
}

pub(crate) fn pending(record: &AccountRecord) -> Option<KeyOffer> {
    offered_key(record).map(|key| KeyOffer {
        pinned: record.pinned_fingerprint.clone(),
        offered: key.fingerprint().clone(),
    })
}

fn decode(encoded: &str) -> Option<ManifestKey> {
    match ManifestKey::from_base64(encoded) {
        Ok(key) => Some(key),
        Err(error) => {
            tracing::warn!(error = %error, "a stored manifest key does not decode");
            None
        },
    }
}
