use std::path::Path;

pub fn open_path(path: &Path) {
    #[cfg(target_os = "windows")]
    open_windows(path);
    #[cfg(target_os = "macos")]
    open_macos(path);
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let _ = path;
}

#[cfg(target_os = "windows")]
fn open_windows(path: &Path) {
    use std::ptr;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let target: Vec<u16> = path
        .display()
        .to_string()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}

#[cfg(target_os = "macos")]
fn open_macos(path: &Path) {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSString, NSURL};

    unsafe {
        let url_str = NSString::from_str(&path.display().to_string());
        let url = NSURL::fileURLWithPath(&url_str);
        let workspace = NSWorkspace::sharedWorkspace();
        workspace.openURL(&url);
    }
}
