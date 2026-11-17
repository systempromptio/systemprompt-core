//! The member that contributes this crate's test jobs. It is admitted beside
//! the core members, so the test jobs reach the scheduler through the same
//! `AdmittedSchedule` production jobs do; assertions about production jobs
//! use [`core_schedule`], which does not admit it.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::{
    Admitted, AdmittedSchedule, ContributionError, Contributions, Extension, ExtensionMetadata,
};
use systemprompt_manifest::Config;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    core_members, fixture_admitted, fixture_admitted_of, fixture_app_context_with_extensions,
    fixture_config,
};
use systemprompt_traits::Dependencies;

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

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        crate::test_jobs::contribute(c);
        crate::job_rows_db::contribute(c);
        crate::registration_db::contribute(c);
        Ok(())
    }
}

pub fn admitted_with_test_jobs() -> Arc<Admitted> {
    let mut members = core_members();
    members.push(Box::new(SchedulerTestJobsExtension));
    fixture_admitted_of(members)
}

pub fn schedule_of(admitted: &Admitted) -> Arc<AdmittedSchedule> {
    Arc::new(AdmittedSchedule::of(admitted.built()))
}

pub fn core_schedule() -> Arc<AdmittedSchedule> {
    schedule_of(&fixture_admitted())
}

pub async fn app_over_config(
    config: Config,
    admitted: Arc<Admitted>,
) -> (Arc<AppContext>, Arc<AdmittedSchedule>) {
    let schedule = schedule_of(&admitted);
    let app = fixture_app_context_with_extensions(config, admitted)
        .await
        .expect("the app context over the admitted members builds");
    (app, schedule)
}

pub async fn app_over(
    database_url: &str,
    admitted: Arc<Admitted>,
) -> (Arc<AppContext>, Arc<AdmittedSchedule>) {
    app_over_config(fixture_config(database_url), admitted).await
}

pub async fn scheduled_app(database_url: &str) -> (Arc<AppContext>, Arc<AdmittedSchedule>) {
    app_over(database_url, admitted_with_test_jobs()).await
}

pub async fn job_dependencies(pool: &DbPool, database_url: &str) -> Dependencies {
    let app = systemprompt_test_fixtures::test_app_context(database_url).await;
    app.kernel()
        .state()
        .clone()
        .with(Arc::clone(app.app_paths_arc()))
        .with(Arc::clone(pool))
        .with(app)
}
