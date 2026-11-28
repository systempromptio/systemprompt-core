//! Windows: Task Scheduler through `schtasks`, the UAC consent dialog, and
//! the machine policy key with a file under `ProgramData`.
//!
//! `unsafe` is confined to the three modules that call Win32; every block
//! states why it is sound.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod elevation;
mod ffi;
mod policy;
mod scheduler;

use super::{Current, System};
use crate::elevation::{ElevatedCommand, Elevation, ElevationError, ElevationOutcome};
use crate::policy::{PolicyError, PolicyIdentity, PolicyLocation, PolicyRead};
use crate::scheduler::{Created, EntryId, EntryState, Removal, SchedulerEntry, SchedulerError};

impl System for Current {
    fn elevation() -> Result<Elevation, ElevationError> {
        elevation::current().map_err(|source| ElevationError::Detect { source })
    }

    fn elevate(command: &ElevatedCommand) -> Result<ElevationOutcome, ElevationError> {
        elevation::elevate(command)
    }

    fn create_entry(entry: &SchedulerEntry) -> Result<Created, SchedulerError> {
        scheduler::create(entry)
    }

    fn query_entry(id: &EntryId) -> Result<EntryState, SchedulerError> {
        scheduler::query(id)
    }

    fn remove_entry(id: &EntryId) -> Result<Removal, SchedulerError> {
        scheduler::remove(id)
    }

    fn policy_locations(identity: &PolicyIdentity) -> Result<Vec<PolicyLocation>, PolicyError> {
        policy::locations(identity)
    }

    fn read_policy(identity: &PolicyIdentity) -> Result<Vec<PolicyRead>, PolicyError> {
        policy::read(identity)
    }
}
