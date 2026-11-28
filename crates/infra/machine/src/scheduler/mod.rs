//! Scheduler entries: one typed description of a program the system starts
//! for the current user, registered with Task Scheduler on Windows, as a
//! launchd agent on macOS and as systemd user units on Linux.
//!
//! [`create`] replaces an entry of the same id, [`query`] says whether one is
//! registered and [`remove`] retires it. The documents each system is handed
//! are rendered by [`task_xml`], [`launchd`] and [`systemd`], which hold no
//! platform condition, so the bytes for every system are checked on each.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod entry;
mod error;
pub mod launchd;
pub mod systemd;
pub mod task_xml;

pub use entry::{Created, EntryId, EntryState, Interval, Removal, SchedulerEntry, Trigger};
pub use error::{EntryError, SchedulerError};

use crate::platform::{Current, System};

pub fn create(entry: &SchedulerEntry) -> Result<Created, SchedulerError> {
    Current::create_entry(entry)
}

pub fn query(id: &EntryId) -> Result<EntryState, SchedulerError> {
    Current::query_entry(id)
}

pub fn remove(id: &EntryId) -> Result<Removal, SchedulerError> {
    Current::remove_entry(id)
}
