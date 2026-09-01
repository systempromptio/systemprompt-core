//! Whether the scheduler jobs are registered, and the per-process cache of
//! that answer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::RwLock;

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

/// The last conclusive answer the host scheduler gave, per registration.
///
/// Asking the scheduler spawns a subprocess, and the tray redraw asks on every
/// 30-second probe — uncached, that stalls the event loop and on Windows
/// flashes a console window. Registration only changes through
/// `install::schedule_apply`, which writes through here. `Unknown` is never
/// cached: a transient failure to reach the scheduler must not become the
/// permanent answer.
#[derive(Debug, Default)]
pub struct ScheduleStatusCache {
    autostart: RwLock<Option<ScheduleStatus>>,
    schedule: RwLock<Option<ScheduleStatus>>,
}

impl ScheduleStatusCache {
    pub fn autostart(&self, probe: impl FnOnce() -> ScheduleStatus) -> ScheduleStatus {
        cached(&self.autostart, probe)
    }

    pub fn schedule(&self, probe: impl FnOnce() -> ScheduleStatus) -> ScheduleStatus {
        cached(&self.schedule, probe)
    }

    pub fn set_autostart(&self, status: ScheduleStatus) {
        store(&self.autostart, status);
    }

    pub fn set_schedule(&self, status: ScheduleStatus) {
        store(&self.schedule, status);
    }
}

fn cached(
    cell: &RwLock<Option<ScheduleStatus>>,
    probe: impl FnOnce() -> ScheduleStatus,
) -> ScheduleStatus {
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
