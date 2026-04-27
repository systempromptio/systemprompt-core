#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::PlatformWindow;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::PlatformWindow;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod stub;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub use stub::PlatformWindow;

pub fn open_path(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        windows::open_path(path);
    }
    #[cfg(target_os = "macos")]
    {
        macos::open_path(path);
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = path;
    }
}
