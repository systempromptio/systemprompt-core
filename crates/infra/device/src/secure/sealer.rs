//! Sealing secrets for the current user. The platform's sealer is DPAPI on
//! Windows, a login keychain key on macOS, and on Linux a key the Secret
//! Service or the kernel keyring holds, or this process alone where the
//! machine has no keyring. A process-memory sealer exists for tests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use parking_lot::Mutex;
use zeroize::Zeroizing;

use crate::platform::{Native, Platform};
use crate::secure::error::SecureError;

/// Seals secrets so that only the current user, through this app, can open
/// them.
///
/// `open` distinguishes storage that cannot be used right now
/// ([`SecureError::Unavailable`], keep the ciphertext) from a ciphertext that
/// will never open here ([`SecureError::Rejected`]). Without `available()` no
/// secret should be accepted for keeping.
pub trait Sealer: Send + Sync + fmt::Debug {
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, SecureError>;
    fn open(&self, ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, SecureError>;
    fn available(&self) -> bool;
}

#[must_use]
pub fn platform_sealer(app_id: &str) -> Arc<dyn Sealer> {
    Native::sealer(app_id)
}

const MEMORY_MAGIC: &[u8; 8] = b"spmem01\0";
const ID_LEN: usize = 16;
const MEMORY_CIPHERTEXT_LEN: usize = MEMORY_MAGIC.len() + 2 * ID_LEN;

/// TEST AND DEVELOPMENT ONLY. Keeps each plaintext in this process's memory and
/// hands out an opaque handle as its "ciphertext"; nothing survives the process
/// and nothing touches disk.
///
/// Only [`MemorySealer::for_tests`] reports `available() == true`.
/// [`MemorySealer::disabled`] refuses to seal or open anything, as a machine
/// whose secure store cannot be used does.
pub struct MemorySealer {
    enabled: bool,
    id: [u8; ID_LEN],
    vault: Mutex<HashMap<[u8; ID_LEN], Zeroizing<Vec<u8>>>>,
}

impl fmt::Debug for MemorySealer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemorySealer")
            .field("enabled", &self.enabled)
            .field("entries", &self.vault.lock().len())
            .finish_non_exhaustive()
    }
}

impl MemorySealer {
    #[must_use]
    pub fn for_tests() -> Self {
        Self::build(true)
    }

    #[must_use]
    pub fn disabled() -> Self {
        Self::build(false)
    }

    fn build(enabled: bool) -> Self {
        Self {
            enabled,
            id: uuid::Uuid::new_v4().into_bytes(),
            vault: Mutex::new(HashMap::new()),
        }
    }
}

impl Sealer for MemorySealer {
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, SecureError> {
        if !self.enabled {
            return Err(SecureError::Unsupported);
        }
        let handle = uuid::Uuid::new_v4().into_bytes();
        self.vault
            .lock()
            .insert(handle, Zeroizing::new(plaintext.to_vec()));
        let mut ciphertext = Vec::with_capacity(MEMORY_CIPHERTEXT_LEN);
        ciphertext.extend_from_slice(MEMORY_MAGIC);
        ciphertext.extend_from_slice(&self.id);
        ciphertext.extend_from_slice(&handle);
        Ok(ciphertext)
    }

    fn open(&self, ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, SecureError> {
        if !self.enabled {
            return Err(SecureError::Unsupported);
        }
        if ciphertext.is_empty() {
            return Err(SecureError::Empty);
        }
        let Some(rest) = ciphertext.strip_prefix(MEMORY_MAGIC.as_slice()) else {
            return Err(SecureError::rejected("not a memory-sealer ciphertext"));
        };
        if rest.len() != 2 * ID_LEN {
            return Err(SecureError::rejected(
                "memory-sealer ciphertext has the wrong length",
            ));
        }
        let (sealer_id, handle) = rest.split_at(ID_LEN);
        if sealer_id != self.id.as_slice() {
            return Err(SecureError::rejected("sealed by another sealer or process"));
        }
        let Ok(handle) = <[u8; ID_LEN]>::try_from(handle) else {
            return Err(SecureError::rejected(
                "memory-sealer ciphertext has the wrong length",
            ));
        };
        self.vault
            .lock()
            .get(&handle)
            .map(|plaintext| Zeroizing::new(plaintext.to_vec()))
            .ok_or_else(|| SecureError::rejected("no secret is held for this handle"))
    }

    fn available(&self) -> bool {
        self.enabled
    }
}
