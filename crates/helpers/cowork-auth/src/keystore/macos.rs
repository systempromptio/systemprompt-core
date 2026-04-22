use super::{DeviceCert, DeviceCertSource};

pub struct MacOsKeystore;

impl DeviceCertSource for MacOsKeystore {
    fn load(&self) -> Result<DeviceCert, String> {
        Err("macOS keystore integration not yet implemented; see plan phase 2".to_string())
    }
}

pub fn platform_source() -> Box<dyn DeviceCertSource> {
    Box::new(MacOsKeystore)
}
