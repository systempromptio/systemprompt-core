//! Sealing secrets for the current user: DPAPI on Windows, the login keychain
//! on macOS (`KeychainSealer`), a process-memory store for tests.
//! Elsewhere the platform sealer is a disabled memory sealer that reports
//! itself unavailable, so no secret is ever kept there outside tests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use parking_lot::Mutex;
use zeroize::Zeroizing;

use crate::error::SecureError;

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
#[cfg_attr(
    not(any(windows, target_os = "macos")),
    expect(
        unused_variables,
        reason = "the app id scopes DPAPI and the keychain, which exist only on Windows and macOS"
    )
)]
pub fn platform_sealer(app_id: &str) -> Arc<dyn Sealer> {
    #[cfg(windows)]
    {
        Arc::new(DpapiSealer::new(app_id.as_bytes()))
    }
    #[cfg(target_os = "macos")]
    {
        Arc::new(crate::mac::KeychainSealer::new(app_id))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Arc::new(MemorySealer::disabled())
    }
}

/// DPAPI in the current-user scope (`CryptProtectData` / `CryptUnprotectData`),
/// bound to an app-specific entropy blob and never prompting.
#[cfg(windows)]
#[derive(Debug, Clone)]
pub struct DpapiSealer {
    entropy: Vec<u8>,
}

#[cfg(windows)]
impl DpapiSealer {
    const PROBE: &'static [u8] = b"sp-secure availability probe";

    #[must_use]
    pub fn new(entropy: &[u8]) -> Self {
        Self {
            entropy: entropy.to_vec(),
        }
    }
}

#[cfg(windows)]
impl Sealer for DpapiSealer {
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, SecureError> {
        crate::win::dpapi_protect(plaintext, &self.entropy)
            .map_err(|source| SecureError::Unavailable { source })
    }

    fn open(&self, ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, SecureError> {
        if ciphertext.is_empty() {
            return Err(SecureError::Empty);
        }
        crate::win::dpapi_unprotect(ciphertext, &self.entropy).map_err(|source| {
            if crate::win::is_foreign_ciphertext(&source) {
                SecureError::Rejected { source }
            } else {
                SecureError::Unavailable { source }
            }
        })
    }

    fn available(&self) -> bool {
        let sealed = match self.seal(Self::PROBE) {
            Ok(sealed) => sealed,
            Err(e) => {
                tracing::warn!(error = %e, "DPAPI cannot seal for this user");
                return false;
            },
        };
        match self.open(&sealed) {
            Ok(opened) => opened.as_slice() == Self::PROBE,
            Err(e) => {
                tracing::warn!(error = %e, "DPAPI cannot open what it sealed for this user");
                false
            },
        }
    }
}

const MEMORY_MAGIC: &[u8; 8] = b"spmem01\0";
const ID_LEN: usize = 16;
const MEMORY_CIPHERTEXT_LEN: usize = MEMORY_MAGIC.len() + 2 * ID_LEN;

/// TEST AND DEVELOPMENT ONLY. Keeps each plaintext in this process's memory and
/// hands out an opaque handle as its "ciphertext"; nothing survives the process
/// and nothing touches disk.
///
/// Only [`MemorySealer::for_tests`] reports `available() == true`.
/// [`MemorySealer::disabled`] is the platform sealer where neither DPAPI nor
/// the keychain exists: it refuses to seal or open anything.
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
