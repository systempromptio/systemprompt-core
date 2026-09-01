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
#[cfg(not(target_os = "windows"))]
use std::fs;
use std::path::Path;
#[cfg(not(target_os = "windows"))]
use std::path::PathBuf;
use std::sync::RwLock;

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

// Why: asking the host scheduler spawns a subprocess, and the tray redraw
// asks on every 30-second probe — uncached, that stalls the event loop and on
// Windows flashes a console window. Registration only changes through this
// module, so the answer is cached here and written through by the calls that
// change it. `Unknown` is never cached: a transient failure to reach the
// scheduler must not become the permanent answer.
static AUTOSTART_STATUS: RwLock<Option<ScheduleStatus>> = RwLock::new(None);
static SCHEDULE_STATUS: RwLock<Option<ScheduleStatus>> = RwLock::new(None);

fn cached(cell: &RwLock<Option<ScheduleStatus>>, probe: fn() -> ScheduleStatus) -> ScheduleStatus {
    if let Ok(guard) = cell.read()
        && let Some(status) = *guard
    {
        return status;
    }
    let status = probe();
    if status != ScheduleStatus::Unknown
        && let Ok(mut guard) = cell.write()
    {
        *guard = Some(status);
    }
    status
}

fn store(cell: &RwLock<Option<ScheduleStatus>>, status: ScheduleStatus) {
    if let Ok(mut guard) = cell.write() {
        *guard = Some(status);
    }
}

pub fn apply_schedule(os: Os, binary: &Path) -> Result<ScheduleApplied, InstallError> {
    if !same_os(os, Os::current()) {
        return Err(InstallError::ScheduleOsMismatch);
    }
    let rendered = schedule::template(os, binary);
    let (path, lines) = platform::register(os, &rendered, binary)?;
    store(&SCHEDULE_STATUS, ScheduleStatus::Installed);
    Ok(ScheduleApplied {
        os,
        label: schedule::schedule_label(os).to_owned(),
        path,
        lines,
    })
}

pub fn remove_schedule() -> ScheduleRemoval {
    let removal = platform::remove_current();
    if !matches!(removal, ScheduleRemoval::Failed(_)) {
        store(&SCHEDULE_STATUS, ScheduleStatus::NotInstalled);
    }
    removal
}

// Why: not a convenience. The bridge's value is a loopback proxy that governs
// agent traffic, so a session where nobody remembered to open the app is a
// session where agents ran ungoverned.
pub fn apply_gui_autostart(binary: &Path) -> Result<Vec<String>, InstallError> {
    let rendered = schedule::autostart_template(Os::current(), binary);
    let lines = platform::register_autostart(&rendered)?;
    store(&AUTOSTART_STATUS, ScheduleStatus::Installed);
    Ok(lines)
}

pub fn remove_gui_autostart() -> ScheduleRemoval {
    let removal = platform::remove_autostart();
    if !matches!(removal, ScheduleRemoval::Failed(_)) {
        store(&AUTOSTART_STATUS, ScheduleStatus::NotInstalled);
    }
    removal
}

#[must_use]
pub fn gui_autostart_status() -> ScheduleStatus {
    cached(&AUTOSTART_STATUS, platform::autostart_status)
}

/// Whether the periodic sync job is registered with the host scheduler.
///
/// `Unknown` is a real answer, not a failure: the Settings pane previously
/// hardcoded "manual", which became a lie the moment a schedule was installed,
/// and guessing is how it got there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScheduleStatus {
    Installed,
    NotInstalled,
    Unknown,
}

impl ScheduleStatus {
    #[must_use]
    pub const fn tone(self) -> crate::verdict::Tone {
        match self {
            Self::Installed => crate::verdict::Tone::Ok,
            Self::NotInstalled => crate::verdict::Tone::Warn,
            Self::Unknown => crate::verdict::Tone::Unknown,
        }
    }

    #[must_use]
    pub const fn verdict(self) -> crate::verdict::Verdict<Self> {
        crate::verdict::Verdict::new(self.tone(), self)
    }
}

#[must_use]
pub fn schedule_status() -> ScheduleStatus {
    cached(&SCHEDULE_STATUS, platform::schedule_registered)
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
