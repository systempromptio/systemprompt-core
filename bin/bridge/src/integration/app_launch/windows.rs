//! Windows app discovery and launch: MSIX packages and Start-menu entries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::process::Command;

use super::run;

pub(super) fn msix_package_present(family: &str) -> bool {
    const REPOSITORY: &str = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";

    let Some((stem, publisher)) = family.rsplit_once('_') else {
        return false;
    };
    let name_prefix = format!("{}_", stem.to_ascii_lowercase());
    let publisher_suffix = format!("__{}", publisher.to_ascii_lowercase());
    winreg::enumerate_subkeys(REPOSITORY).is_some_and(|keys| {
        keys.iter()
            .map(|k| k.to_ascii_lowercase())
            .any(|k| k.starts_with(&name_prefix) && k.ends_with(&publisher_suffix))
    })
}

mod winreg {
    #![allow(
        unsafe_code,
        reason = "Win32 registry FFI to enumerate the AppModel package repository"
    )]

    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegEnumKeyExW, RegOpenKeyExW,
    };

    struct OwnedKey(HKEY);

    impl Drop for OwnedKey {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: `self.0` is a non-null key this `OwnedKey` exclusively owns.
                unsafe { RegCloseKey(self.0) };
            }
        }
    }

    // Why: `None` means the key could not be opened, which is distinct from an
    // empty key; registry key names are capped at 255 chars, +1 for the NUL.
    pub(super) fn enumerate_subkeys(subkey: &str) -> Option<Vec<String>> {
        const MAX_KEY_NAME: usize = 256;

        let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
        let mut handle: HKEY = std::ptr::null_mut();
        // SAFETY: `HKEY_CURRENT_USER` is a predefined hive, `subkey_w` is a
        // NUL-terminated UTF-16 buffer, and `handle` is a live out-param.
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey_w.as_ptr(),
                0,
                KEY_READ,
                &raw mut handle,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let key = OwnedKey(handle);

        let mut out = Vec::new();
        let mut buf = [0u16; MAX_KEY_NAME];
        for index in 0.. {
            let mut len = u32::try_from(buf.len()).unwrap_or(0);
            // SAFETY: `key.0` is a live open key; `buf`/`len` are a matched
            // buffer and capacity out-param, and the remaining out-params are
            // documented as optional and passed as null.
            let status = unsafe {
                RegEnumKeyExW(
                    key.0,
                    index,
                    buf.as_mut_ptr(),
                    &raw mut len,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            if status != ERROR_SUCCESS {
                break;
            }
            let len = usize::try_from(len).unwrap_or(0).min(buf.len());
            out.push(String::from_utf16_lossy(&buf[..len]));
        }
        Some(out)
    }
}

pub(super) fn start_menu_present_cached(
    cache: &crate::probe_cache::StartMenuCache,
    display_name: &str,
) -> Option<bool> {
    if let Some(presence) = cache.lookup(display_name) {
        return presence.as_probe();
    }
    let present = start_menu_present(display_name);
    cache.record(
        display_name,
        crate::probe_cache::StartMenuPresence::from_probe(present),
    );
    present
}

fn start_menu_present(display_name: &str) -> Option<bool> {
    use std::time::{Duration, Instant};

    const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
    let script = format!(
        "if (Get-StartApps | Where-Object {{ $_.Name -eq '{name}' }}) {{ exit 0 }} else {{ exit 2 }}",
        name = ps_single_quote(display_name),
    );
    let Ok(mut child) = crate::winproc::no_window(&mut Command::new("powershell"))
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .spawn()
    else {
        return None;
    };
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return match status.code() {
                    Some(0) => Some(true),
                    Some(2) => Some(false),
                    _ => None,
                };
            },
            Ok(None) => {
                if Instant::now() >= deadline {
                    drop(child.kill());
                    drop(child.wait());
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            },
            Err(_) => return None,
        }
    }
}

pub(super) fn msix_launch(family: &str, app_id: &str) -> io::Result<()> {
    run(
        crate::winproc::no_window(&mut Command::new("cmd")).args([
            "/C",
            "start",
            "",
            &format!(r"shell:AppsFolder\{family}!{app_id}"),
        ]),
        family,
    )
}

pub(super) fn start_menu_launch(display_name: &str) -> io::Result<()> {
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         $a = Get-StartApps | Where-Object {{ $_.Name -eq '{name}' }} | Select-Object -First 1; \
         if (-not $a) {{ exit 2 }}; \
         Start-Process ('shell:AppsFolder\\' + $a.AppID); exit 0",
        name = ps_single_quote(display_name),
    );
    let status = crate::winproc::no_window(&mut Command::new("powershell"))
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no Start-menu app named {display_name}"),
        ))
    }
}

fn ps_single_quote(s: &str) -> String {
    s.replace('\'', "''")
}
