//! The one place the crate names a target system.
//!
//! [`System`] is the contract for every concern and [`Current`] is the system
//! the crate is built for. Each of the Windows, macOS and Linux modules
//! implements the contract for it; on any other target nothing does, and the
//! crate does not compile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
mod process;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use crate::elevation::{ElevatedCommand, Elevation, ElevationError, ElevationOutcome};
use crate::policy::{PolicyError, PolicyIdentity, PolicyLocation, PolicyRead};
use crate::scheduler::{Created, EntryId, EntryState, Removal, SchedulerEntry, SchedulerError};

pub(crate) trait System {
    fn elevation() -> Result<Elevation, ElevationError>;
    fn elevate(command: &ElevatedCommand) -> Result<ElevationOutcome, ElevationError>;
    fn create_entry(entry: &SchedulerEntry) -> Result<Created, SchedulerError>;
    fn query_entry(id: &EntryId) -> Result<EntryState, SchedulerError>;
    fn remove_entry(id: &EntryId) -> Result<Removal, SchedulerError>;
    fn policy_locations(identity: &PolicyIdentity) -> Result<Vec<PolicyLocation>, PolicyError>;
    fn read_policy(identity: &PolicyIdentity) -> Result<Vec<PolicyRead>, PolicyError>;
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Current {}
