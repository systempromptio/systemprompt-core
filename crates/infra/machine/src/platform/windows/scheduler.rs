//! Task Scheduler registration through `schtasks`, found in the system
//! directory. The definition is staged in the user's temporary directory for
//! the import and removed after it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use super::ffi;
use crate::platform::process::{self, Finished};
use crate::scheduler::{
    Created, EntryId, EntryState, Removal, SchedulerEntry, SchedulerError, task_xml,
};

const SCHTASKS: &str = "schtasks.exe";

fn schtasks(verb: &str, arguments: &[&OsStr]) -> Result<Finished, SchedulerError> {
    let program = ffi::system_directory()
        .map_err(|source| SchedulerError::io("resolve", Path::new(SCHTASKS), source))?
        .join(SCHTASKS);
    process::run(
        program.as_os_str(),
        std::iter::once(OsStr::new(verb)).chain(arguments.iter().copied()),
    )
    .map_err(|source| SchedulerError::Spawn {
        command: format!("schtasks {verb}"),
        source,
    })
}

fn exited(verb: &str, finished: Finished) -> SchedulerError {
    SchedulerError::Exited {
        command: format!("schtasks {verb}"),
        code: finished.code,
        stderr: finished.stderr,
    }
}

fn stage(id: &EntryId, definition: &[u8]) -> Result<PathBuf, SchedulerError> {
    let path = std::env::temp_dir().join(format!("{id}.{}.task.xml", std::process::id()));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut file| file.write_all(definition))
        .map_err(|source| SchedulerError::io("stage", &path, source))?;
    Ok(path)
}

pub(super) fn create(entry: &SchedulerEntry) -> Result<Created, SchedulerError> {
    let name = OsStr::new(entry.id().as_str());
    let staged = stage(entry.id(), &task_xml::encode(&task_xml::render(entry)))?;
    let created = schtasks(
        "/Create",
        &[
            OsStr::new("/TN"),
            name,
            OsStr::new("/XML"),
            staged.as_os_str(),
            OsStr::new("/F"),
        ],
    );
    if let Err(error) = fs::remove_file(&staged) {
        tracing::debug!(
            path = %staged.display(),
            error = %error,
            "staged task definition left behind"
        );
    }
    let finished = created?;
    if finished.succeeded() {
        Ok(Created::Active)
    } else {
        Err(exited("/Create", finished))
    }
}

// Why: schtasks exits 1 both for a task that does not exist and for a query it
// could not run; a task it cannot show is reported as not installed.
pub(super) fn query(id: &EntryId) -> Result<EntryState, SchedulerError> {
    let finished = schtasks("/Query", &[OsStr::new("/TN"), OsStr::new(id.as_str())])?;
    Ok(if finished.succeeded() {
        EntryState::Installed
    } else {
        EntryState::NotInstalled
    })
}

pub(super) fn remove(id: &EntryId) -> Result<Removal, SchedulerError> {
    if query(id)? == EntryState::NotInstalled {
        return Ok(Removal::NotInstalled);
    }
    let finished = schtasks(
        "/Delete",
        &[OsStr::new("/TN"), OsStr::new(id.as_str()), OsStr::new("/F")],
    )?;
    if finished.succeeded() {
        Ok(Removal::Removed)
    } else {
        Err(exited("/Delete", finished))
    }
}
