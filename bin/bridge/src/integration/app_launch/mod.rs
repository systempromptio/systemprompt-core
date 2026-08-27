//! Cross-platform "is the desktop app installed / open it" helpers shared by
//! the host integrations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::PathBuf;
use std::process::Command;

use crate::integration::host_app::AppInstallState;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
use windows::{msix_launch, msix_package_present, start_menu_launch, start_menu_present_cached};
#[derive(Debug, Clone, Copy)]
#[expect(
    dead_code,
    reason = "each target_os arm reads a different subset of these fields"
)]
pub(crate) struct AppLocator<'a> {
    pub macos_name: &'a str,
    pub windows_name: &'a str,
    pub windows_candidates: &'a [PathBuf],
    pub linux_bin: &'a str,
    pub msix_family: Option<&'a str>,
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
    // Why: AppsFolder activation is the only launch path that works for an MSIX
    // package — its exe under %ProgramFiles%\WindowsApps is not executable by us.
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
    // Why: MSIX packages live under the ACL-locked %ProgramFiles%\WindowsApps, so
    // the path check cannot see them; the AppModel repository is readable
    // unelevated.
    if let Some(family) = loc.msix_family
        && msix_package_present(family)
    {
        return AppInstallState::Installed;
    }
    match start_menu_present_cached(loc.windows_name) {
        Some(true) => AppInstallState::Installed,
        Some(false) => AppInstallState::NotInstalled,
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
    if let Some(home) = crate::basedirs::home_dir() {
        out.push(home.join("Applications").join(format!("{name}.app")));
    }
    out
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
