//! The extension that owns this crate's test jobs, registered through the
//! normal `register_extension!` path so the jobs are attributed to a linked
//! extension that is not a production one. Assertions about production jobs
//! exclude [`TEST_JOBS_OWNER`].

use systemprompt_extension::prelude::*;

pub const TEST_JOBS_OWNER: &str = "scheduler_test_jobs";

#[derive(Debug, Default, Clone, Copy)]
pub struct SchedulerTestJobsExtension;

impl Extension for SchedulerTestJobsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: TEST_JOBS_OWNER,
            name: "Scheduler test jobs",
            version: "0.0.1",
        }
    }
}

register_extension!(SchedulerTestJobsExtension);
