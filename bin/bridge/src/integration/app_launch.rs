//! Cross-platform "is the desktop app installed / open it" helpers shared by
//! the host integrations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::PathBuf;
use std::process::Command;

use crate::integration::host_app::AppInstallState;

/// Everything the platform detectors need to find one host application.
///
/// Grouped into a struct rather than passed positionally: the Windows path
/// needs four of these at once and stringly-typed positional arguments made the
/// call sites unreadable.
#[derive(Debug, Clone, Copy)]
#[expect(
    dead_code,
    reason = "each target_os arm reads a different subset of these fields"
)]
pub(crate) struct AppLocator<'a> {
    /// macOS `.app` bundle name, without the extension.
    pub macos_name: &'a str,
    /// Windows Start-menu display name.
    pub windows_name: &'a str,
    /// Windows install paths to test directly, in priority order.
    pub windows_candidates: &'a [PathBuf],
    /// Linux executable name to look for on `PATH`.
    pub linux_bin: &'a str,
    /// Windows MSIX package family name, when the host ships as a package.
    pub msix_family: Option<&'a str>,
    /// Application ID within `msix_family`.
    pub msix_app_id: &'a str,
}

#[cfg(target_os = "macos")]
pub(crate) fn open_app(loc: &AppLocator<'_>) -> io::Result<()> {
    run(
        Command::new("/usr/bin/open").args(["-a", loc.macos_name]),
        loc.macos_name,
    )
}

#[cfg(target_os = "windows")]
pub(crate) fn open_app(loc: &AppLocator<'_>) -> io::Result<()> {
    // AppsFolder activation first: it is the only launch path that works for an
    // MSIX package (the exe under %ProgramFiles%\WindowsApps is not directly
    // executable by us) and it costs no PowerShell spawn.
    if let Some(family) = loc.msix_family
        && msix_launch(family, loc.msix_app_id).is_ok()
    {
        return Ok(());
    }
    if start_menu_launch(loc.windows_name).is_ok() {
        return Ok(());
    }
    if let Some(path) = loc.windows_candidates.iter().find(|p| p.exists()) {
        return run(
            Command::new("cmd").args(["/C", "start", "", &path.to_string_lossy()]),
            loc.windows_name,
        );
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "{} is not installed (no MSIX package, Start-menu entry or known install path found)",
            loc.windows_name
        ),
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn open_app(loc: &AppLocator<'_>) -> io::Result<()> {
    Command::new(loc.linux_bin).spawn().map(|_| ())
}

#[cfg(target_os = "macos")]
pub(crate) fn is_installed(loc: &AppLocator<'_>) -> AppInstallState {
    if macos_bundles(loc.macos_name).iter().any(|p| p.exists()) {
        AppInstallState::Installed
    } else {
        AppInstallState::NotInstalled
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn is_installed(loc: &AppLocator<'_>) -> AppInstallState {
    if loc.windows_candidates.iter().any(|p| p.exists()) {
        return AppInstallState::Installed;
    }
    // MSIX packages install under the ACL-locked %ProgramFiles%\WindowsApps, so
    // the path check above can never see them. The per-user AppModel repository
    // records them and is readable unelevated — and unlike the Start-menu probe
    // it spawns no process, so it stays cheap enough for a 30s probe tick.
    if let Some(family) = loc.msix_family
        && msix_package_present(family)
    {
        return AppInstallState::Installed;
    }
    match start_menu_present_cached(loc.windows_name) {
        Some(true) => AppInstallState::Installed,
        Some(false) => AppInstallState::NotInstalled,
        // The probe timed out or could not be spawned. We genuinely do not know;
        // reporting "not installed" here is what made a running, fully
        // configured host render as a red error.
        None => AppInstallState::Unknown,
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn is_installed(loc: &AppLocator<'_>) -> AppInstallState {
    let found = std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| dir.join(loc.linux_bin).exists())
    });
    if found {
        AppInstallState::Installed
    } else {
        AppInstallState::NotInstalled
    }
}

#[cfg(target_os = "macos")]
fn macos_bundles(name: &str) -> Vec<PathBuf> {
    let mut out = vec![PathBuf::from(format!("/Applications/{name}.app"))];
    if let Some(home) = dirs::home_dir() {
        out.push(home.join("Applications").join(format!("{name}.app")));
    }
    out
}

/// Registry-only MSIX presence check.
///
/// Subkeys of the per-user package repository are named
/// `<family-stem>_<version>_<arch>__<publisher-id>`, e.g.
/// `Claude_1.22209.3.0_x64__pzs8sxrjxfjjc` for family `Claude_pzs8sxrjxfjjc`.
#[cfg(target_os = "windows")]
fn msix_package_present(family: &str) -> bool {
    const REPOSITORY: &str = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";

    // Split from the right: the publisher ID is always the final `_` segment,
    // while the name part may itself contain underscores.
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

/// Minimal HKCU subkey enumeration.
///
/// Kept local rather than added to `config::store`, which is deliberately
/// scoped to the managed-policy subtree and exposes value reads only.
#[cfg(target_os = "windows")]
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

    /// `None` means the key could not be opened (absent or unreadable), which
    /// is distinct from an empty key.
    pub(super) fn enumerate_subkeys(subkey: &str) -> Option<Vec<String>> {
        // Registry key names are capped at 255 chars; +1 for the NUL.
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

/// `None` when the probe was inconclusive (timed out or could not be spawned).
#[cfg(target_os = "windows")]
fn start_menu_present_cached(display_name: &str) -> Option<bool> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    // Get-StartApps cold-starts powershell (seconds per call); cache per app so
    // probes spawn it at most once per TTL.
    /// Cached probe verdict (`None` = inconclusive) and when it was taken.
    type Verdict = (Option<bool>, Instant);

    static CACHE: OnceLock<Mutex<HashMap<String, Verdict>>> = OnceLock::new();
    /// A completed Get-StartApps run answered the question; hold it.
    const CONCLUSIVE_TTL: Duration = Duration::from_secs(300);
    /// A timeout says nothing. Expire it inside one probe interval so the next
    /// tick retries instead of pinning the host to a wrong badge for minutes.
    const INCONCLUSIVE_TTL: Duration = Duration::from_secs(15);

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock()
        && let Some((present, at)) = map.get(display_name)
    {
        let ttl = if present.is_some() {
            CONCLUSIVE_TTL
        } else {
            INCONCLUSIVE_TTL
        };
        if at.elapsed() < ttl {
            return *present;
        }
    }
    let present = start_menu_present(display_name);
    if let Ok(mut map) = cache.lock() {
        map.insert(display_name.to_owned(), (present, Instant::now()));
    }
    present
}

/// `None` when the probe was inconclusive (timed out or could not be spawned).
#[cfg(target_os = "windows")]
fn start_menu_present(display_name: &str) -> Option<bool> {
    use std::os::windows::process::CommandExt;
    use std::time::{Duration, Instant};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // Bounded so a probe never blocks the UI. This is now only a fallback for
    // hosts with no MSIX family and no known install path, so the budget is
    // generous rather than tight — a kill here costs an `Unknown` badge.
    const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
    let script = format!(
        "if (Get-StartApps | Where-Object {{ $_.Name -eq '{name}' }}) {{ exit 0 }} else {{ exit 2 }}",
        name = ps_single_quote(display_name),
    );
    let Ok(mut child) = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    else {
        return None;
    };
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            // Exit 0 = present, exit 2 = absent, anything else = the script
            // itself failed and tells us nothing.
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

#[cfg(target_os = "windows")]
fn msix_launch(family: &str, app_id: &str) -> io::Result<()> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    run(
        Command::new("cmd")
            .args([
                "/C",
                "start",
                "",
                &format!(r"shell:AppsFolder\{family}!{app_id}"),
            ])
            .creation_flags(CREATE_NO_WINDOW),
        family,
    )
}

#[cfg(target_os = "windows")]
fn start_menu_launch(display_name: &str) -> io::Result<()> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         $a = Get-StartApps | Where-Object {{ $_.Name -eq '{name}' }} | Select-Object -First 1; \
         if (-not $a) {{ exit 2 }}; \
         Start-Process ('shell:AppsFolder\\' + $a.AppID); exit 0",
        name = ps_single_quote(display_name),
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
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

#[cfg(target_os = "windows")]
fn ps_single_quote(s: &str) -> String {
    s.replace('\'', "''")
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn run(cmd: &mut Command, what: &str) -> io::Result<()> {
    let status = cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "failed to open {what} (exit {})",
            status.code().unwrap_or(-1)
        )))
    }
}
