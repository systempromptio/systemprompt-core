//! A `keyring-core` credential store as this crate's credential store: the
//! Windows Credential Manager, and on Linux the Secret Service and the kernel
//! keyring. Each is an owned store, never the process-global default.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::{fmt, io};

use zeroize::{Zeroize as _, Zeroizing};

use crate::platform::CredentialStore;
use crate::secure::SecureError;

pub(super) struct KeyringStore {
    name: &'static str,
    store: Arc<keyring_core::CredentialStore>,
}

impl fmt::Debug for KeyringStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyringStore")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl KeyringStore {
    pub(super) const fn new(name: &'static str, store: Arc<keyring_core::CredentialStore>) -> Self {
        Self { name, store }
    }

    fn entry(&self, service: &str, account: &str) -> Result<keyring_core::Entry, SecureError> {
        self.store
            .build(service, account, None)
            .map_err(map_keyring)
    }
}

impl CredentialStore for KeyringStore {
    fn name(&self) -> &'static str {
        self.name
    }

    fn store(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), SecureError> {
        self.entry(service, account)?
            .set_secret(secret)
            .map_err(map_keyring)
    }

    fn load(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<Zeroizing<Vec<u8>>>, SecureError> {
        match self.entry(service, account)?.get_secret() {
            Ok(secret) => Ok(Some(Zeroizing::new(secret))),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(map_keyring(e)),
        }
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), SecureError> {
        match self.entry(service, account)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_keyring(e)),
        }
    }
}

// Why: `keyring_core::Error` carries the raw secret bytes in `BadEncoding` and
// `BadDataFormat` and prints them in `Debug`; those bytes are wiped here and
// never kept.
pub(super) fn map_keyring(error: keyring_core::Error) -> SecureError {
    match error {
        keyring_core::Error::NoStorageAccess(platform) => SecureError::Unavailable {
            source: io::Error::other(platform),
        },
        keyring_core::Error::BadEncoding(mut bytes) => {
            bytes.zeroize();
            SecureError::rejected("stored credential is not in the expected encoding")
        },
        keyring_core::Error::BadDataFormat(mut bytes, platform) => {
            bytes.zeroize();
            SecureError::Rejected {
                source: io::Error::new(io::ErrorKind::InvalidData, platform),
            }
        },
        other => SecureError::Keystore {
            source: io::Error::other(other),
        },
    }
}
