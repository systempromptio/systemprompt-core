use super::{DeviceCert, DeviceCertSource, sha256_der};
use std::{env, fs};

pub struct LinuxKeystore;

impl DeviceCertSource for LinuxKeystore {
    fn load(&self) -> Result<DeviceCert, String> {
        let path = env::var("SP_COWORK_DEVICE_CERT")
            .map_err(|_| "SP_COWORK_DEVICE_CERT unset; no device cert on Linux".to_string())?;
        let bytes = fs::read(&path).map_err(|e| format!("read {path}: {e}"))?;
        let der = pem_to_der(&bytes).unwrap_or(bytes);
        Ok(DeviceCert {
            fingerprint: sha256_der(&der),
        })
    }
}

pub fn platform_source() -> Box<dyn DeviceCertSource> {
    Box::new(LinuxKeystore)
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
    const ALPHA: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [0xFFu8; 256];
    for (i, b) in ALPHA.iter().enumerate() {
        lookup[*b as usize] = i as u8;
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
