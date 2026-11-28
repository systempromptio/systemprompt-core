//! Secrets kept in the OS credential store of the platform: the Windows
//! Credential Manager, the macOS login keychain, and on Linux the Secret
//! Service or, without one, the kernel keyring of the session. Where a machine
//! has none of them the store does not open and the caller keeps the secret in
//! memory; there is never a plaintext file. A test-only memory store exists
//! beside them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use parking_lot::Mutex;
use zeroize::Zeroizing;

use crate::platform::{CredentialStore, Native, Platform};
use crate::secure::error::SecureError;

/// A credential store addressed by `(service, account)`. The caller derives the
/// service from the build's identity, so brands and channels never share an
/// entry.
pub struct Keystore {
    store: Arc<dyn CredentialStore>,
}

#[derive(Default)]
struct MemoryStore(Mutex<HashMap<(String, String), Zeroizing<Vec<u8>>>>);

impl fmt::Debug for MemoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryStore")
            .field("entries", &self.0.lock().len())
            .finish()
    }
}

impl CredentialStore for MemoryStore {
    fn name(&self) -> &'static str {
        "memory (test only)"
    }

    fn store(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), SecureError> {
        self.0.lock().insert(
            (service.to_owned(), account.to_owned()),
            Zeroizing::new(secret.to_vec()),
        );
        Ok(())
    }

    fn load(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<Zeroizing<Vec<u8>>>, SecureError> {
        Ok(self
            .0
            .lock()
            .get(&(service.to_owned(), account.to_owned()))
            .map(|secret| Zeroizing::new(secret.to_vec())))
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), SecureError> {
        self.0
            .lock()
            .remove(&(service.to_owned(), account.to_owned()));
        Ok(())
    }
}

impl fmt::Debug for Keystore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keystore")
            .field("backend", &self.store.name())
            .finish()
    }
}

impl Keystore {
    pub fn platform() -> Result<Self, SecureError> {
        Native::credential_store().map(|store| Self { store })
    }

    #[must_use]
    pub fn memory_for_tests() -> Self {
        Self {
            store: Arc::new(MemoryStore::default()),
        }
    }

    pub fn store(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), SecureError> {
        self.store.store(service, account, secret)
    }

    pub fn load(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<Zeroizing<Vec<u8>>>, SecureError> {
        self.store.load(service, account)
    }

    pub fn delete(&self, service: &str, account: &str) -> Result<(), SecureError> {
        self.store.delete(service, account)
    }
}
