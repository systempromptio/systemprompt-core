//! Cross-platform "open the installed desktop app" helper shared by the host
//! integrations.
//!
//! macOS resolves the app by name through `LaunchServices` (`open -a`). Windows
//! resolves the Start-menu entry (its `AppUserModelID`) by display name via
//! `Get-StartApps` and launches `shell:AppsFolder\<AUMID>` — this covers both
//! classic desktop apps and Microsoft Store / MSIX packages (e.g. the Codex
//! app), and an exact name match launches "Claude", never "Claude Code". It
//! falls back to known install paths, then a clear not-installed error.
//!
//! Launching a bare command name is deliberately avoided: it fails when the
//! tool is not on `PATH` and can resolve to the wrong target.

use std::io;
use std::path::PathBuf;
use std::process::Command;

#[cfg(target_os = "macos")]
pub(crate) fn open_app(
    macos_name: &str,
    _windows_name: &str,
    _windows_candidates: &[PathBuf],
    _linux_bin: &str,
) -> io::Result<()> {
    run(
        Command::new("/usr/bin/open").args(["-a", macos_name]),
        macos_name,
    )
}

#[cfg(target_os = "windows")]
pub(crate) fn open_app(
    _macos_name: &str,
    windows_name: &str,
    windows_candidates: &[PathBuf],
    _linux_bin: &str,
) -> io::Result<()> {
    if start_menu_launch(windows_name).is_ok() {
        return Ok(());
    }
    if let Some(path) = windows_candidates.iter().find(|p| p.exists()) {
        return run(
            Command::new("cmd").args(["/C", "start", "", &path.to_string_lossy()]),
            windows_name,
        );
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "{windows_name} is not installed (no Start-menu entry or known install path found)"
        ),
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn open_app(
    _macos_name: &str,
    _windows_name: &str,
    _windows_candidates: &[PathBuf],
    linux_bin: &str,
) -> io::Result<()> {
    Command::new(linux_bin).spawn().map(|_| ())
}

#[cfg(target_os = "macos")]
pub(crate) fn is_installed(
    macos_name: &str,
    _windows_name: &str,
    _windows_candidates: &[PathBuf],
    _linux_bin: &str,
) -> bool {
    macos_bundles(macos_name).iter().any(|p| p.exists())
}

#[cfg(target_os = "windows")]
pub(crate) fn is_installed(
    _macos_name: &str,
    windows_name: &str,
    windows_candidates: &[PathBuf],
    _linux_bin: &str,
) -> bool {
    windows_candidates.iter().any(|p| p.exists()) || start_menu_present(windows_name)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn is_installed(
    _macos_name: &str,
    _windows_name: &str,
    _windows_candidates: &[PathBuf],
    linux_bin: &str,
) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(linux_bin).exists()))
}

#[cfg(target_os = "macos")]
fn macos_bundles(name: &str) -> Vec<PathBuf> {
    let mut out = vec![PathBuf::from(format!("/Applications/{name}.app"))];
    if let Some(home) = dirs::home_dir() {
        out.push(home.join("Applications").join(format!("{name}.app")));
    }
    out
}

#[cfg(target_os = "windows")]
fn start_menu_present(display_name: &str) -> bool {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = format!(
        "if (Get-StartApps | Where-Object {{ $_.Name -eq '{name}' }}) {{ exit 0 }} else {{ exit 2 }}",
        name = ps_single_quote(display_name),
    );
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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
