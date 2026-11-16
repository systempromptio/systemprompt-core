//! The member that contributes this crate's probe jobs. It is admitted beside
//! the core members, so the probe jobs reach the scheduler through the same
//! `AdmittedSchedule` production jobs do.

use std::sync::Arc;

use systemprompt_extension::{
    Admitted, AdmittedSchedule, ContributionError, Contributions, Extension, ExtensionMetadata,
};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    core_members, fixture_admitted_of, fixture_app_context_with_extensions, fixture_config,
    test_database_url,
};

use crate::distributed_lock::{LockedProbeJob, UnlockedProbeJob};

pub(crate) const TEST_JOBS_OWNER: &str = "scheduler_test_jobs";

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct SchedulerTestJobsExtension;

impl Extension for SchedulerTestJobsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: TEST_JOBS_OWNER,
            name: "Scheduler test jobs",
            version: "0.0.1",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.job(LockedProbeJob).job(UnlockedProbeJob);
        Ok(())
    }
}

pub(crate) fn admitted_with_test_jobs() -> Arc<Admitted> {
    let mut members = core_members();
    members.push(Box::new(SchedulerTestJobsExtension));
    fixture_admitted_of(members)
}

pub(crate) async fn app_with_test_jobs() -> (Arc<AppContext>, Arc<AdmittedSchedule>) {
    let admitted = admitted_with_test_jobs();
    let schedule = Arc::new(AdmittedSchedule::of(admitted.built()));
    let config = fixture_config(&test_database_url());
    let app = fixture_app_context_with_extensions(config, admitted)
        .await
        .expect("the app context over the core members and the probe jobs builds");
    (app, schedule)
}


