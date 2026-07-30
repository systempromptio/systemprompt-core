//! Linux file-based device-certificate source with PEM/DER handling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{CertRef, DeviceCert, DeviceCertSource, KeystoreError, sha256_der};
use std::{env, fs};

pub(super) struct LinuxKeystore {
    configured_path: Option<String>,
}

impl LinuxKeystore {
    fn new(cert_ref: CertRef<'_>) -> Self {
        Self {
            configured_path: cert_ref
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(crate::auth::expand_home),
        }
    }

    /// The env var wins over `mtls.cert_keystore_ref` so setups that predate
    /// the config key keep working unchanged.
    fn resolve(&self) -> Result<String, KeystoreError> {
        let cert_env = crate::brand::brand().env("DEVICE_CERT");
        if let Ok(from_env) = env::var(&cert_env)
            && !from_env.trim().is_empty()
        {
            return Ok(from_env);
        }
        self.configured_path.clone().ok_or_else(|| {
            KeystoreError::NotConfigured(format!(
                "{cert_env} unset and mtls.cert_keystore_ref absent; no device cert on Linux"
            ))
        })
    }
}

impl DeviceCertSource for LinuxKeystore {
    fn load(&self) -> Result<DeviceCert, KeystoreError> {
        let path = self.resolve()?;
        let bytes = fs::read(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                KeystoreError::NotFound(format!("{path}: {e}"))
            } else {
                KeystoreError::Io(e)
            }
        })?;
        let der = pem_to_der(&bytes).unwrap_or(bytes);
        Ok(DeviceCert {
            fingerprint: sha256_der(&der)?,
        })
    }
}

#[must_use]
pub fn platform_source(cert_ref: CertRef<'_>) -> Box<dyn DeviceCertSource> {
    Box::new(LinuxKeystore::new(cert_ref))
}

fn pem_to_der(input: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(input).ok()?;
    let begin = text.find("-----BEGIN CERTIFICATE-----")?;
    let end = text.find("-----END CERTIFICATE-----")?;
    let body = &text[begin + "-----BEGIN CERTIFICATE-----".len()..end];
    let cleaned: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    base64_decode(&cleaned)
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [0xFFu8; 256];
    for (i, b) in ALPHA.iter().enumerate() {
        lookup[*b as usize] = u8::try_from(i).unwrap_or(0xFF);
    }
    let trimmed = input.trim_end_matches('=');
    let mut out = Vec::with_capacity(trimmed.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u8 = 0;
    for c in trimmed.bytes() {
        let v = lookup[c as usize];
        if v == 0xFF {
            return None;
        }
        buf = (buf << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}
