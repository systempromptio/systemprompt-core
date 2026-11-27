//! A sealed secret as base64 text, the form the accounts store keeps on disk.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::error::SecureError;
use crate::sealer::Sealer;

/// Base64 (standard alphabet, padded) of a [`Sealer`]'s ciphertext.
///
/// Serialises as a plain string; deserialising checks that it is non-empty
/// base64. It is opened only through a [`Sealer`], and `Debug` shows its
/// length, never its text.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SealedString(String);

impl SealedString {
    pub fn seal(sealer: &dyn Sealer, plaintext: &str) -> Result<Self, SecureError> {
        Self::seal_bytes(sealer, plaintext.as_bytes())
    }

    pub fn seal_bytes(sealer: &dyn Sealer, plaintext: &[u8]) -> Result<Self, SecureError> {
        let ciphertext = sealer.seal(plaintext)?;
        Ok(Self(STANDARD.encode(ciphertext)))
    }

    pub fn open(&self, sealer: &dyn Sealer) -> Result<Zeroizing<String>, SecureError> {
        let mut bytes = self.open_bytes(sealer)?;
        match String::from_utf8(std::mem::take(&mut *bytes)) {
            Ok(text) => Ok(Zeroizing::new(text)),
            Err(e) => {
                e.into_bytes().zeroize();
                Err(SecureError::NotUtf8)
            },
        }
    }

    pub fn open_bytes(&self, sealer: &dyn Sealer) -> Result<Zeroizing<Vec<u8>>, SecureError> {
        let ciphertext = decode(&self.0)?;
        sealer.open(&ciphertext)
    }

    pub fn from_base64(text: String) -> Result<Self, SecureError> {
        decode(&text)?;
        Ok(Self(text))
    }

    #[must_use]
    pub fn as_base64(&self) -> &str {
        &self.0
    }
}

fn decode(text: &str) -> Result<Vec<u8>, SecureError> {
    if text.is_empty() {
        return Err(SecureError::Empty);
    }
    STANDARD
        .decode(text)
        .map_err(|source| SecureError::Encoding { source })
}

impl TryFrom<String> for SealedString {
    type Error = SecureError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::from_base64(text)
    }
}

impl From<SealedString> for String {
    fn from(sealed: SealedString) -> Self {
        sealed.0
    }
}

impl fmt::Debug for SealedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SealedString(<{} base64 chars>)", self.0.len())
    }
}
