pub struct DeviceCert {
    pub fingerprint: String,
}

pub trait DeviceCertSource {
    fn load(&self) -> Result<DeviceCert, String>;
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
