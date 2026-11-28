//! Sealing for the current user on Windows: DPAPI bound to the app id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use zeroize::Zeroizing;

use super::dpapi::{dpapi_protect, dpapi_unprotect, is_foreign_ciphertext};
use crate::secure::{Sealer, SecureError};

/// DPAPI in the current-user scope (`CryptProtectData` / `CryptUnprotectData`),
/// bound to an app-specific entropy blob and never prompting.
#[derive(Debug, Clone)]
pub struct DpapiSealer {
    entropy: Vec<u8>,
}

impl DpapiSealer {
    const PROBE: &'static [u8] = b"sp-secure availability probe";

    #[must_use]
    pub fn new(entropy: &[u8]) -> Self {
        Self {
            entropy: entropy.to_vec(),
        }
    }
}

impl Sealer for DpapiSealer {
    fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, SecureError> {
        dpapi_protect(plaintext, &self.entropy)
            .map_err(|source| SecureError::Unavailable { source })
    }

    fn open(&self, ciphertext: &[u8]) -> Result<Zeroizing<Vec<u8>>, SecureError> {
        if ciphertext.is_empty() {
            return Err(SecureError::Empty);
        }
        dpapi_unprotect(ciphertext, &self.entropy).map_err(|source| {
            if is_foreign_ciphertext(&source) {
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
