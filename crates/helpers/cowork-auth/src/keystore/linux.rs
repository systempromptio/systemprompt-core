use super::{DeviceCert, DeviceCertSource};
use std::{env, fs};

pub struct LinuxKeystore;

impl DeviceCertSource for LinuxKeystore {
    fn load(&self) -> Result<DeviceCert, String> {
        let path = env::var("SP_COWORK_DEVICE_CERT")
            .map_err(|_| "SP_COWORK_DEVICE_CERT unset; no device cert on Linux".to_string())?;
        let bytes = fs::read(&path).map_err(|e| format!("read {path}: {e}"))?;
        let fingerprint = fingerprint_hex(&bytes);
        Ok(DeviceCert { fingerprint })
    }
}

pub fn platform_source() -> Box<dyn DeviceCertSource> {
    Box::new(LinuxKeystore)
}

fn fingerprint_hex(bytes: &[u8]) -> String {
    let mut hasher: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hasher ^= u64::from(*b);
        hasher = hasher.wrapping_mul(0x100_0000_01b3);
    }
    format!("{hasher:016x}")
}
