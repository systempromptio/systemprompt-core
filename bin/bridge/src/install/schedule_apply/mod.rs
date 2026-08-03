//! Registers (and deregisters) the periodic sync job with the host scheduler.
//!
//! Emitting a template only tells the user what to run; this module runs it.
//! Every path is idempotent — re-running `install --apply-schedule` replaces
//! the existing registration rather than adding a second one — and every
//! identifier is brand-scoped via [`crate::schedule::schedule_label`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{InstallError, ScheduleApplied, ScheduleRemoval};
use crate::schedule::{self, Os};
use std::fs;
use std::path::{Path, PathBuf};

/// Registers the sync job for the running OS. `os` is the caller's declared
/// target and must match [`Os::current`] — a Windows task cannot be created
/// from a Mac.
pub fn apply_schedule(os: Os, binary: &Path) -> Result<ScheduleApplied, InstallError> {
    if !same_os(os, Os::current()) {
        return Err(InstallError::ScheduleOsMismatch);
    }
    let rendered = schedule::template(os, binary);
    let (path, lines) = register(os, &rendered, binary)?;
    Ok(ScheduleApplied {
        os,
        label: schedule::schedule_label(os).to_owned(),
        path,
        lines,
    })
}

const fn same_os(a: Os, b: Os) -> bool {
    matches!(
        (a, b),
        (Os::Mac, Os::Mac) | (Os::Windows, Os::Windows) | (Os::Linux, Os::Linux)
    )
}

#[cfg(not(target_os = "windows"))]
fn write(path: &Path, contents: &str) -> Result<(), InstallError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| InstallError::Schedule {
            path: parent.display().to_string(),
            source: e,
        })?;
    }
    fs::write(path, contents).map_err(|e| InstallError::Schedule {
        path: path.display().to_string(),
        source: e,
    })
}

#[cfg(not(target_os = "windows"))]
fn home() -> Result<PathBuf, InstallError> {
    crate::basedirs::home_dir().ok_or_else(|| {
        InstallError::ScheduleApply("cannot resolve the user's home directory".into())
    })
}

// Why: launchd addresses the per-user domain as `gui/<uid>`.
#[cfg(target_os = "macos")]
fn gui_domain() -> String {
    #![allow(unsafe_code, reason = "libc::getuid is the only way to read the uid")]
    format!("gui/{}", unsafe { libc::getuid() })
}

#[cfg(target_os = "macos")]
fn register(
    os: Os,
    rendered: &str,
    _binary: &Path,
) -> Result<(PathBuf, Vec<String>), InstallError> {
    use std::process::Command;

    let label = schedule::schedule_label(os);
    let path = home()?
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{label}.plist"));
    write(&path, rendered)?;

    let domain = gui_domain();
    // Why: launchctl fails bootout with "not loaded" on a first install, which
    // is expected and not fatal.
    _ = Command::new("launchctl")
        .args(["bootout", &domain])
        .arg(&path)
        .status();
    let status = Command::new("launchctl")
        .args(["bootstrap", &domain])
        .arg(&path)
        .status()
        .map_err(|e| InstallError::ScheduleApply(format!("launchctl bootstrap: {e}")))?;
    if !status.success() {
        return Err(InstallError::ScheduleApply(format!(
            "launchctl bootstrap exited with {}",
            status.code().unwrap_or(-1)
        )));
    }
    Ok((
        path.clone(),
        vec![
            format!("wrote: {}", path.display()),
            format!("launchd agent: {label} (loaded in {domain})"),
        ],
    ))
}

#[cfg(target_os = "windows")]
fn register(
    os: Os,
    rendered: &str,
    _binary: &Path,
) -> Result<(PathBuf, Vec<String>), InstallError> {
    use std::process::Command;

    let task = schedule::schedule_label(os);
    let path = std::env::temp_dir().join(schedule::template_filename(os));
    // Why: Task Scheduler requires UTF-16LE with a BOM for the XML it imports.
    let mut bytes = vec![0xFF, 0xFE];
    for unit in rendered.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    fs::write(&path, &bytes).map_err(|e| InstallError::Schedule {
        path: path.display().to_string(),
        source: e,
    })?;

    // Why: without `/F` schtasks duplicates rather than replaces a task of the
    // same name.
    let status = Command::new("schtasks")
        .args(["/Create", "/TN", task, "/XML"])
        .arg(&path)
        .arg("/F")
        .status()
        .map_err(|e| InstallError::ScheduleApply(format!("schtasks /Create: {e}")))?;
    _ = fs::remove_file(&path);
    if !status.success() {
        return Err(InstallError::ScheduleApply(format!(
            "schtasks /Create exited with {}",
            status.code().unwrap_or(-1)
        )));
    }
    Ok((
        path,
        vec![format!("scheduled task: {task} (logon + every 30m)")],
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn register(os: Os, rendered: &str, binary: &Path) -> Result<(PathBuf, Vec<String>), InstallError> {
    let unit = schedule::schedule_label(os);
    let (service, timer) = schedule::split_systemd_unit(rendered).ok_or_else(|| {
        InstallError::ScheduleApply("systemd template has no .timer section".into())
    })?;
    let dir = home()?.join(".config").join("systemd").join("user");
    let service_path = dir.join(format!("{unit}.service"));
    let timer_path = dir.join(format!("{unit}.timer"));
    write(&service_path, &service)?;
    write(&timer_path, &timer)?;

    let proxy_unit = schedule::proxy_unit_name();
    let proxy_path = dir.join(format!("{proxy_unit}.service"));
    write(&proxy_path, &schedule::proxy_template(binary))?;

    let mut lines = vec![
        format!("wrote: {}", service_path.display()),
        format!("wrote: {}", timer_path.display()),
        format!("wrote: {}", proxy_path.display()),
    ];

    // Why: activation needs a systemd user bus, which containers and
    // systemd-less WSL distros lack; the written units still stand.
    if let Err(e) = activate(unit, &proxy_unit) {
        crate::obs::output::diag(&format!(
            "warning: units written but not activated: {e}. Activate them yourself with: \
             systemctl --user daemon-reload && systemctl --user enable --now {unit}.timer \
             {proxy_unit}.service"
        ));
        lines.push(format!("not activated: {e}"));
        return Ok((timer_path, lines));
    }

    lines.push(format!(
        "systemd user timer: {unit}.timer (enabled, every 30m)"
    ));
    lines.push(format!(
        "systemd user service: {proxy_unit}.service (enabled, restarts on failure)"
    ));
    Ok((timer_path, lines))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn activate(unit: &str, proxy_unit: &str) -> Result<(), InstallError> {
    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", "--now", &format!("{unit}.timer")])?;
    systemctl(&["enable", "--now", &format!("{proxy_unit}.service")])
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn systemctl(args: &[&str]) -> Result<(), InstallError> {
    let status = std::process::Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()
        .map_err(|e| InstallError::ScheduleApply(format!("systemctl --user {}: {e}", args[0])))?;
    if status.success() {
        return Ok(());
    }
    Err(InstallError::ScheduleApply(format!(
        "systemctl --user {} exited with {}",
        args.join(" "),
        status.code().unwrap_or(-1)
    )))
}

/// Removes the registration written by [`apply_schedule`]. Reports rather than
/// fails: an uninstall must not abort because no job was ever registered.
pub fn remove_schedule() -> ScheduleRemoval {
    remove_current()
}

#[cfg(target_os = "macos")]
fn remove_current() -> ScheduleRemoval {
    use std::process::Command;

    let label = schedule::schedule_label(Os::Mac);
    let Ok(home) = home() else {
        return ScheduleRemoval::Failed("cannot resolve the user's home directory".into());
    };
    let path = home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{label}.plist"));
    if !path.exists() {
        return ScheduleRemoval::NotInstalled(label.to_owned());
    }
    _ = Command::new("launchctl")
        .args(["bootout", &gui_domain()])
        .arg(&path)
        .status();
    match fs::remove_file(&path) {
        Ok(()) => ScheduleRemoval::Removed(label.to_owned()),
        Err(e) => ScheduleRemoval::Failed(format!("remove {}: {e}", path.display())),
    }
}

#[cfg(target_os = "windows")]
fn remove_current() -> ScheduleRemoval {
    use std::process::Command;

    let task = schedule::schedule_label(Os::Windows);
    match Command::new("schtasks")
        .args(["/Delete", "/TN", task, "/F"])
        .status()
    {
        Ok(s) if s.success() => ScheduleRemoval::Removed(task.to_owned()),
        Ok(_) => ScheduleRemoval::NotInstalled(task.to_owned()),
        Err(e) => ScheduleRemoval::Failed(format!("schtasks /Delete: {e}")),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn remove_current() -> ScheduleRemoval {
    let unit = schedule::schedule_label(Os::Linux);
    let proxy_unit = schedule::proxy_unit_name();
    let Ok(home) = home() else {
        return ScheduleRemoval::Failed("cannot resolve the user's home directory".into());
    };
    let dir = home.join(".config").join("systemd").join("user");
    let timer_path = dir.join(format!("{unit}.timer"));
    let proxy_path = dir.join(format!("{proxy_unit}.service"));
    if !timer_path.exists() && !proxy_path.exists() {
        return ScheduleRemoval::NotInstalled(unit.to_owned());
    }
    _ = systemctl(&["disable", "--now", &format!("{unit}.timer")]);
    _ = systemctl(&["disable", "--now", &format!("{proxy_unit}.service")]);
    let removed = remove_if_present(&timer_path)
        .and_then(|()| remove_if_present(&dir.join(format!("{unit}.service"))))
        .and_then(|()| remove_if_present(&proxy_path));
    match removed {
        Ok(()) => {
            _ = systemctl(&["daemon-reload"]);
            ScheduleRemoval::Removed(format!("{unit} + {proxy_unit}"))
        },
        Err(e) => ScheduleRemoval::Failed(format!("remove under {}: {e}", dir.display())),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}
