//! Scheduler-job identifiers.
//!
//! `ScheduledJobId` is the `scheduled_jobs.id` the scheduler mints as a UUID;
//! `JobName` is a job's registered name (the `submit_job!` name and the
//! `scheduler.jobs` key), a checked non-empty string.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(ScheduledJobId, uuid);
crate::define_id!(JobName, checked, |value| {
    crate::macros::validate_non_empty("JobName", value)
});
