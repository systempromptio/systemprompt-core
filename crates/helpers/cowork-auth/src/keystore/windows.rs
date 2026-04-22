use super::{DeviceCert, DeviceCertSource};

pub struct WindowsKeystore;

impl DeviceCertSource for WindowsKeystore {
    fn load(&self) -> Result<DeviceCert, String> {
        Err("Windows keystore integration not yet implemented; see plan phase 2".to_string())
    }
}

pub fn platform_source() -> Box<dyn DeviceCertSource> {
    Box::new(WindowsKeystore)
}
