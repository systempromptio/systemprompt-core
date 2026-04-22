use sha2::{Digest, Sha256};
use std::fmt::Write;

pub struct DeviceCert {
    pub fingerprint: String,
}

pub trait DeviceCertSource {
    fn load(&self) -> Result<DeviceCert, String>;
}

pub fn sha256_der(der: &[u8]) -> String {
    let digest = Sha256::digest(der);
    let mut out = String::with_capacity(64);
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    out
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
