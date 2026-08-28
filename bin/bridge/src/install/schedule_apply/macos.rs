//! launchd registration for the sync job.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{InstallError, ScheduleRemoval, home, write};
use crate::schedule::{self, Os};

// Why: launchd addresses the per-user domain as `gui/<uid>`.
fn gui_domain() -> String {
    #![allow(unsafe_code, reason = "libc::getuid is the only way to read the uid")]
    // SAFETY: getuid() is always safe; it reads the caller's real uid and cannot
    // fail.
    format!("gui/{}", unsafe { libc::getuid() })
}

fn agents_dir() -> Result<PathBuf, InstallError> {
    Ok(home()?.join("Library").join("LaunchAgents"))
}

pub(super) fn register(
    os: Os,
    rendered: &str,
    _binary: &Path,
) -> Result<(PathBuf, Vec<String>), InstallError> {
    let label = schedule::schedule_label(os);
    let path = agents_dir()?.join(format!("{label}.plist"));
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

pub(super) fn register_autostart(rendered: &str) -> Result<Vec<String>, InstallError> {
    let label = schedule::autostart_label(Os::Mac);
    let path = agents_dir()?.join(format!("{label}.plist"));
    write(&path, rendered)?;
    let domain = gui_domain();
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
    Ok(vec![format!("login agent: {label} (loaded in {domain})")])
}

pub(super) fn remove_autostart() -> ScheduleRemoval {
    let label = schedule::autostart_label(Os::Mac);
    let Ok(dir) = agents_dir() else {
        return ScheduleRemoval::Failed("cannot resolve the user's home directory".into());
    };
    let path = dir.join(format!("{label}.plist"));
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

pub(super) fn autostart_status() -> super::ScheduleStatus {
    let Ok(dir) = agents_dir() else {
        return super::ScheduleStatus::Unknown;
    };
    let path = dir.join(format!("{}.plist", schedule::autostart_label(Os::Mac)));
    match path.try_exists() {
        Ok(true) => super::ScheduleStatus::Installed,
        Ok(false) => super::ScheduleStatus::NotInstalled,
        Err(_) => super::ScheduleStatus::Unknown,
    }
}

pub(super) fn schedule_registered() -> super::ScheduleStatus {
    let label = schedule::schedule_label(Os::Mac);
    match agents_dir() {
        Ok(dir) => {
            if dir.join(format!("{label}.plist")).exists() {
                super::ScheduleStatus::Installed
            } else {
                super::ScheduleStatus::NotInstalled
            }
        },
        Err(_) => super::ScheduleStatus::Unknown,
    }
}

pub(super) fn remove_current() -> ScheduleRemoval {
    let label = schedule::schedule_label(Os::Mac);
    let Ok(dir) = agents_dir() else {
        return ScheduleRemoval::Failed("cannot resolve the user's home directory".into());
    };
    let path = dir.join(format!("{label}.plist"));
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
