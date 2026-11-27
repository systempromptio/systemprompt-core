//! Generic password items addressed by `(service, account)` in the default
//! keychain: set (created or replaced), added only when absent, read and
//! removed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use core_foundation::data::CFData;
use security_framework::base::Error;
use security_framework::item::{ItemAddOptions, ItemAddValue, ItemClass, Location};
use security_framework::os::macos::keychain::SecKeychain;
use security_framework::passwords::{
    delete_generic_password, generic_password, set_generic_password,
};
use security_framework::passwords_options::PasswordOptions;
use zeroize::Zeroizing;

use super::status::{is_duplicate, is_not_found, keystore};
use crate::error::SecureError;

pub(super) enum Insertion {
    Added,
    AlreadyPresent,
}

pub(super) fn read(service: &str, account: &str) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
    match generic_password(PasswordOptions::new_generic_password(service, account)) {
        Ok(secret) => Ok(Some(Zeroizing::new(secret))),
        Err(error) if is_not_found(error) => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn add(service: &str, account: &str, secret: &[u8]) -> Result<Insertion, Error> {
    let mut options = ItemAddOptions::new(ItemAddValue::Data {
        class: ItemClass::generic_password(),
        data: CFData::from_buffer(secret),
    });
    options
        .set_service(service)
        .set_account_name(account)
        .set_label(service)
        .set_location(Location::DefaultFileKeychain);
    match options.add() {
        Ok(()) => Ok(Insertion::Added),
        Err(error) if is_duplicate(error) => Ok(Insertion::AlreadyPresent),
        Err(error) => Err(error),
    }
}

pub(super) fn remove(service: &str, account: &str) -> Result<(), Error> {
    match delete_generic_password(service, account) {
        Ok(()) => Ok(()),
        Err(error) if is_not_found(error) => Ok(()),
        Err(error) => Err(error),
    }
}

pub(crate) fn default_keychain() -> Result<(), SecureError> {
    SecKeychain::default().map(drop).map_err(keystore)
}

pub(crate) fn store_password(
    service: &str,
    account: &str,
    secret: &[u8],
) -> Result<(), SecureError> {
    set_generic_password(service, account, secret).map_err(keystore)
}

pub(crate) fn load_password(
    service: &str,
    account: &str,
) -> Result<Option<Zeroizing<Vec<u8>>>, SecureError> {
    read(service, account).map_err(keystore)
}

pub(crate) fn delete_password(service: &str, account: &str) -> Result<(), SecureError> {
    remove(service, account).map_err(keystore)
}
