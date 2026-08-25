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

pub(super) fn remove_current() -> ScheduleRemoval {
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
