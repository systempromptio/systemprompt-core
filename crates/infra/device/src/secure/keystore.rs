//! Secrets kept in the OS credential store: Windows Credential Manager through
//! `keyring-core` and `windows-native-keyring-store`, built per call from an
//! owned store rather than the process-global default; on macOS generic
//! password items in the login keychain. Elsewhere only a test-only memory
//! store exists; there is never a plaintext file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fmt;
#[cfg(windows)]
use std::sync::Arc;

use parking_lot::Mutex;
use zeroize::Zeroizing;

use crate::error::SecureError;

/// A credential store addressed by `(service, account)`. The caller derives the
/// service from the build's identity, so brands and channels never share an
/// entry.
pub struct Keystore {
    backend: Backend,
}

type MemoryEntries = HashMap<(String, String), Zeroizing<Vec<u8>>>;

enum Backend {
    #[cfg(windows)]
    Native(Arc<keyring_core::CredentialStore>),
    #[cfg(target_os = "macos")]
    Keychain,
    Memory(Box<Mutex<MemoryEntries>>),
}

impl fmt::Debug for Keystore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let backend = match &self.backend {
            #[cfg(windows)]
            Backend::Native(_) => "credential-manager",
            #[cfg(target_os = "macos")]
            Backend::Keychain => "login-keychain",
            Backend::Memory(_) => "memory (test only)",
        };
        f.debug_struct("Keystore")
            .field("backend", &backend)
            .finish()
    }
}

impl Keystore {
    #[cfg_attr(
        not(any(windows, target_os = "macos")),
        expect(
            clippy::missing_const_for_fn,
            reason = "the Windows and macOS bodies open the native store"
        )
    )]
    pub fn platform() -> Result<Self, SecureError> {
        #[cfg(windows)]
        {
            let store: Arc<keyring_core::CredentialStore> =
                windows_native_keyring_store::Store::new().map_err(map_keyring)?;
            Ok(Self {
                backend: Backend::Native(store),
            })
        }
        #[cfg(target_os = "macos")]
        {
            crate::mac::default_keychain()?;
            Ok(Self {
                backend: Backend::Keychain,
            })
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            Err(SecureError::Unsupported)
        }
    }

    #[must_use]
    pub fn memory_for_tests() -> Self {
        Self {
            backend: Backend::Memory(Box::new(Mutex::new(HashMap::new()))),
        }
    }

    #[cfg_attr(
        not(any(windows, target_os = "macos")),
        expect(
            clippy::unnecessary_wraps,
            reason = "the Windows and macOS credential stores are fallible"
        )
    )]
    pub fn store(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), SecureError> {
        match &self.backend {
            #[cfg(windows)]
            Backend::Native(store) => entry(store, service, account)?
                .set_secret(secret)
                .map_err(map_keyring),
            #[cfg(target_os = "macos")]
            Backend::Keychain => crate::mac::store_password(service, account, secret),
            Backend::Memory(map) => {
                map.lock().insert(
                    (service.to_owned(), account.to_owned()),
                    Zeroizing::new(secret.to_vec()),
                );
                Ok(())
            },
        }
    }

    #[cfg_attr(
        not(any(windows, target_os = "macos")),
        expect(
            clippy::unnecessary_wraps,
            reason = "the Windows and macOS credential stores are fallible"
        )
    )]
    pub fn load(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<Zeroizing<Vec<u8>>>, SecureError> {
        match &self.backend {
            #[cfg(windows)]
            Backend::Native(store) => match entry(store, service, account)?.get_secret() {
                Ok(secret) => Ok(Some(Zeroizing::new(secret))),
                Err(keyring_core::Error::NoEntry) => Ok(None),
                Err(e) => Err(map_keyring(e)),
            },
            #[cfg(target_os = "macos")]
            Backend::Keychain => crate::mac::load_password(service, account),
            Backend::Memory(map) => Ok(map
                .lock()
                .get(&(service.to_owned(), account.to_owned()))
                .map(|secret| Zeroizing::new(secret.to_vec()))),
        }
    }

    #[cfg_attr(
        not(any(windows, target_os = "macos")),
        expect(
            clippy::unnecessary_wraps,
            reason = "the Windows and macOS credential stores are fallible"
        )
    )]
    pub fn delete(&self, service: &str, account: &str) -> Result<(), SecureError> {
        match &self.backend {
            #[cfg(windows)]
            Backend::Native(store) => match entry(store, service, account)?.delete_credential() {
                Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
                Err(e) => Err(map_keyring(e)),
            },
            #[cfg(target_os = "macos")]
            Backend::Keychain => crate::mac::delete_password(service, account),
            Backend::Memory(map) => {
                map.lock().remove(&(service.to_owned(), account.to_owned()));
                Ok(())
            },
        }
    }
}

#[cfg(windows)]
fn entry(
    store: &Arc<keyring_core::CredentialStore>,
    service: &str,
    account: &str,
) -> Result<keyring_core::Entry, SecureError> {
    store.build(service, account, None).map_err(map_keyring)
}

// Why: `keyring_core::Error` carries the raw secret bytes in `BadEncoding` and
// `BadDataFormat` and prints them in `Debug`; those bytes are wiped here and
// never kept.
#[cfg(windows)]
fn map_keyring(error: keyring_core::Error) -> SecureError {
    use std::io;
    use zeroize::Zeroize as _;
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
