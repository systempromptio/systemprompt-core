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
pub use crate::schedule::status::ScheduleStatus;
use crate::schedule::status::ScheduleStatusCache;
use crate::schedule::{self, Os};
#[cfg(not(target_os = "windows"))]
use std::fs;
use std::path::Path;
#[cfg(not(target_os = "windows"))]
use std::path::PathBuf;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as platform;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod xdg;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use xdg as platform;

pub fn apply_schedule(
    cache: &ScheduleStatusCache,
    os: Os,
    binary: &Path,
) -> Result<ScheduleApplied, InstallError> {
    if !same_os(os, Os::current()) {
        return Err(InstallError::ScheduleOsMismatch);
    }
    let rendered = schedule::template(os, binary);
    let (path, lines) = platform::register(os, &rendered, binary)?;
    cache.set_schedule(ScheduleStatus::Installed);
    Ok(ScheduleApplied {
        os,
        label: schedule::schedule_label(os).to_owned(),
        path,
        lines,
    })
}

pub fn remove_schedule(cache: &ScheduleStatusCache) -> ScheduleRemoval {
    let removal = platform::remove_current();
    if !matches!(removal, ScheduleRemoval::Failed(_)) {
        cache.set_schedule(ScheduleStatus::NotInstalled);
    }
    removal
}

// Why: not a convenience. The bridge's value is a loopback proxy that governs
// agent traffic, so a session where nobody remembered to open the app is a
// session where agents ran ungoverned.
pub fn apply_gui_autostart(
    cache: &ScheduleStatusCache,
    binary: &Path,
) -> Result<Vec<String>, InstallError> {
    let rendered = schedule::autostart_template(Os::current(), binary);
    let lines = platform::register_autostart(&rendered)?;
    cache.set_autostart(ScheduleStatus::Installed);
    Ok(lines)
}

pub fn remove_gui_autostart(cache: &ScheduleStatusCache) -> ScheduleRemoval {
    let removal = platform::remove_autostart();
    if !matches!(removal, ScheduleRemoval::Failed(_)) {
        cache.set_autostart(ScheduleStatus::NotInstalled);
    }
    removal
}

#[must_use]
pub fn gui_autostart_status(cache: &ScheduleStatusCache) -> ScheduleStatus {
    cache.autostart(platform::autostart_status)
}

#[must_use]
pub fn schedule_status(cache: &ScheduleStatusCache) -> ScheduleStatus {
    cache.schedule(platform::schedule_registered)
}

#[must_use]
pub fn schedule_label() -> &'static str {
    schedule::schedule_label(Os::current())
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
