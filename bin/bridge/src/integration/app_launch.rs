//! Cross-platform "open the installed desktop app" helper shared by the host
//! integrations.

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
    if windows_candidates.iter().any(|p| p.exists()) {
        return true;
    }
    start_menu_present_cached(windows_name)
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
fn start_menu_present_cached(display_name: &str) -> bool {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    // Get-StartApps cold-starts powershell (seconds per call); cache per app so
    // probes spawn it at most once per TTL.
    static CACHE: OnceLock<Mutex<HashMap<String, (bool, Instant)>>> = OnceLock::new();
    const TTL: Duration = Duration::from_secs(300);

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock()
        && let Some((present, at)) = map.get(display_name)
        && at.elapsed() < TTL
    {
        return *present;
    }
    let present = start_menu_present(display_name);
    if let Ok(mut map) = cache.lock() {
        map.insert(display_name.to_owned(), (present, Instant::now()));
    }
    present
}

#[cfg(target_os = "windows")]
fn start_menu_present(display_name: &str) -> bool {
    use std::os::windows::process::CommandExt;
    use std::time::{Duration, Instant};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // Bounded so a probe never blocks the UI; app_installed is best-effort.
    const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
    let script = format!(
        "if (Get-StartApps | Where-Object {{ $_.Name -eq '{name}' }}) {{ exit 0 }} else {{ exit 2 }}",
        name = ps_single_quote(display_name),
    );
    let Ok(mut child) = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    else {
        return false;
    };
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => {
                if Instant::now() >= deadline {
                    drop(child.kill());
                    drop(child.wait());
                    return false;
                }
                std::thread::sleep(Duration::from_millis(50));
            },
            Err(_) => return false,
        }
    }
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
