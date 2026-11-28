//! Reading Security framework result codes: an absent item, an item already
//! present, a keychain that needs the user (locked, access not yet granted,
//! prompt refused), or a failure.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;

use security_framework::base::Error;

use crate::secure::error::SecureError;

// Why: the Security framework reports `OSStatus` codes (SecBase.h) that the
// safe bindings do not name; these are the ones that mean "no such item",
// "already there" or "the keychain needs the user right now", which keeps a
// ciphertext.
const ITEM_NOT_FOUND: i32 = -25300;
const DUPLICATE_ITEM: i32 = -25299;
const USER_CANCELED: i32 = -128;
const NOT_AVAILABLE: i32 = -25291;
const AUTH_FAILED: i32 = -25293;
const NO_SUCH_KEYCHAIN: i32 = -25294;
const NO_DEFAULT_KEYCHAIN: i32 = -25307;
const INTERACTION_NOT_ALLOWED: i32 = -25308;
const INTERACTION_REQUIRED: i32 = -25315;

pub(super) const fn is_not_found(error: Error) -> bool {
    error.code() == ITEM_NOT_FOUND
}

pub(super) const fn is_duplicate(error: Error) -> bool {
    error.code() == DUPLICATE_ITEM
}

pub(super) const fn needs_user(error: Error) -> bool {
    matches!(
        error.code(),
        USER_CANCELED
            | NOT_AVAILABLE
            | AUTH_FAILED
            | NO_SUCH_KEYCHAIN
            | NO_DEFAULT_KEYCHAIN
            | INTERACTION_NOT_ALLOWED
            | INTERACTION_REQUIRED
    )
}

pub(super) fn unavailable(error: Error) -> SecureError {
    SecureError::Unavailable {
        source: io::Error::other(error),
    }
}

pub(super) fn keystore(error: Error) -> SecureError {
    if needs_user(error) {
        unavailable(error)
    } else {
        SecureError::Keystore {
            source: io::Error::other(error),
        }
    }
}
