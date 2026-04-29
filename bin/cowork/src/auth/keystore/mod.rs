use crate::ids::{CertFingerprint, IdValidationError};
use sha2::{Digest, Sha256};
use std::fmt::Write;

#[derive(Debug, thiserror::Error)]
pub enum KeystoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid fingerprint: {0}")]
    Fingerprint(#[from] IdValidationError),
    #[error("device certificate not configured: {0}")]
    NotConfigured(&'static str),
    #[error("device certificate not found: {0}")]
    NotFound(String),
    #[error("keystore: {0}")]
    Other(String),
}

pub struct DeviceCert {
    pub fingerprint: CertFingerprint,
}

pub trait DeviceCertSource {
    fn load(&self) -> Result<DeviceCert, KeystoreError>;
}

pub fn sha256_der(der: &[u8]) -> Result<CertFingerprint, KeystoreError> {
    let digest = Sha256::digest(der);
    let mut out = String::with_capacity(64);
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    Ok(CertFingerprint::try_new(out)?)
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
