//! Per-OS device-certificate keystore access behind `DeviceCertSource`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::ids::{CertFingerprint, IdValidationError};
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum KeystoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid fingerprint: {0}")]
    Fingerprint(#[from] IdValidationError),
    #[error("device certificate not configured: {0}")]
    NotConfigured(String),
    #[error("device certificate not found: {0}")]
    NotFound(String),
    #[error("keystore: {0}")]
    Other(String),
}

#[derive(Debug)]
pub struct DeviceCert {
    pub fingerprint: CertFingerprint,
}

pub trait DeviceCertSource {
    fn load(&self) -> Result<DeviceCert, KeystoreError>;
}

/// The configured `mtls.cert_keystore_ref`, if any.
///
/// Only the Linux source reads it, where it names a path to the device
/// certificate; the macOS Keychain and Windows certificate-store sources
/// address certificates by label and thumbprint respectively and ignore it.
pub type CertRef<'a> = Option<&'a str>;

pub fn sha256_der(der: &[u8]) -> Result<CertFingerprint, KeystoreError> {
    let hex = crate::hash::hex_encode(&Sha256::digest(der));
    Ok(CertFingerprint::try_new(hex)?)
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::platform_source;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::platform_source;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod linux;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub use linux::platform_source;
