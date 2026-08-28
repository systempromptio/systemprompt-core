//! Task Scheduler registration for the sync job.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{InstallError, ScheduleRemoval};
use crate::schedule::{self, Os};

pub(super) fn register(
    os: Os,
    rendered: &str,
    _binary: &Path,
) -> Result<(PathBuf, Vec<String>), InstallError> {
    let task = schedule::schedule_label(os);
    let path = std::env::temp_dir().join(schedule::template_filename(os));
    fs::write(&path, to_utf16le_bom(rendered)).map_err(|e| InstallError::Schedule {
        path: path.display().to_string(),
        source: e,
    })?;

    // Why: without `/F` schtasks duplicates rather than replaces a task of the
    // same name.
    let status = crate::winproc::no_window(&mut Command::new("schtasks"))
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

fn to_utf16le_bom(rendered: &str) -> Vec<u8> {
    // Why: Task Scheduler refuses XML that is not UTF-16LE with a BOM.
    let mut bytes = vec![0xFF, 0xFE];
    for unit in rendered.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

pub(super) fn register_autostart(rendered: &str) -> Result<Vec<String>, InstallError> {
    let task = schedule::autostart_label(Os::Windows);
    let path = std::env::temp_dir().join(format!("{task}.xml"));
    fs::write(&path, to_utf16le_bom(rendered)).map_err(|e| InstallError::Schedule {
        path: path.display().to_string(),
        source: e,
    })?;
    let status = crate::winproc::no_window(&mut Command::new("schtasks"))
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
    Ok(vec![format!("logon task: {task}")])
}

pub(super) fn remove_autostart() -> ScheduleRemoval {
    let task = schedule::autostart_label(Os::Windows);
    match crate::winproc::no_window(&mut Command::new("schtasks"))
        .args(["/Delete", "/TN", task, "/F"])
        .status()
    {
        Ok(s) if s.success() => ScheduleRemoval::Removed(task.to_owned()),
        Ok(_) => ScheduleRemoval::NotInstalled(task.to_owned()),
        Err(e) => ScheduleRemoval::Failed(format!("schtasks /Delete: {e}")),
    }
}

pub(super) fn autostart_status() -> super::ScheduleStatus {
    let task = schedule::autostart_label(Os::Windows);
    match crate::winproc::no_window(&mut Command::new("schtasks"))
        .args(["/Query", "/TN", task])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(s) if s.success() => super::ScheduleStatus::Installed,
        Ok(_) => super::ScheduleStatus::NotInstalled,
        // Why: schtasks failing to launch says nothing about whether the task
        // exists, and an unchecked box the user cannot tick is worse than one
        // that admits it does not know.
        Err(_) => super::ScheduleStatus::Unknown,
    }
}

pub(super) fn schedule_registered() -> super::ScheduleStatus {
    let task = schedule::schedule_label(Os::Windows);
    match crate::winproc::no_window(&mut Command::new("schtasks"))
        .args(["/Query", "/TN", task])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(s) if s.success() => super::ScheduleStatus::Installed,
        Ok(_) => super::ScheduleStatus::NotInstalled,
        // Why: schtasks itself failing to launch says nothing about whether the
        // task exists, so reporting "not installed" would be a guess.
        Err(_) => super::ScheduleStatus::Unknown,
    }
}

pub(super) fn remove_current() -> ScheduleRemoval {
    let task = schedule::schedule_label(Os::Windows);
    match crate::winproc::no_window(&mut Command::new("schtasks"))
        .args(["/Delete", "/TN", task, "/F"])
        .status()
    {
        Ok(s) if s.success() => ScheduleRemoval::Removed(task.to_owned()),
        Ok(_) => ScheduleRemoval::NotInstalled(task.to_owned()),
        Err(e) => ScheduleRemoval::Failed(format!("schtasks /Delete: {e}")),
    }
}
