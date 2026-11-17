//! The files member contributes the document actor job run scope: a job body
//! run inside it observes the system document actor named after the job.
//! Outside the scope the actor falls back to the capped unscoped agent, and
//! an admitted schedule applies the scope only when the files member is
//! admitted.

use std::sync::Arc;

use systemprompt_extension::{AdmittedSchedule, StandingTag};
use systemprompt_files::FilesExtension;
use systemprompt_files::documents::agents::{DocumentActor, current_actor, unscoped_actor};
use systemprompt_identifiers::ExtensionId;
use systemprompt_test_fixtures::admitted_member;
use systemprompt_traits::{JobResult, JobRunScope};

fn contributed_scopes() -> Vec<Arc<dyn JobRunScope>> {
    admitted_member(&FilesExtension, StandingTag::Core)
        .expect("the files member admits")
        .sink
        .job_run_scopes()
        .to_vec()
}

fn document_actor_scope() -> Arc<dyn JobRunScope> {
    contributed_scopes()
        .into_iter()
        .find(|scope| scope.name() == "document_actor")
        .expect("the files member contributes the document actor scope")
}

#[tokio::test]
async fn a_job_run_acts_as_the_system_actor_named_after_the_job() {
    let observed = std::sync::Mutex::new(None);
    let result = document_actor_scope()
        .scope(
            "scope_probe_job",
            Box::pin(async {
                *observed.lock().expect("observed lock") = Some(current_actor());
                Ok(JobResult::success())
            }),
        )
        .await
        .expect("scoped run");
    assert!(result.success);
    assert_eq!(
        observed.into_inner().expect("observed lock"),
        Some(DocumentActor::system("scope_probe_job"))
    );
}

#[tokio::test]
async fn outside_a_job_run_the_actor_is_the_unscoped_agent() {
    assert_eq!(current_actor(), unscoped_actor());
}

#[test]
fn the_files_member_contributes_one_job_run_scope() {
    let names: Vec<&str> = contributed_scopes()
        .iter()
        .map(|scope| scope.name())
        .collect();
    assert_eq!(names, vec!["document_actor"]);
}

#[tokio::test]
async fn the_admitted_schedule_scopes_a_run_only_when_files_is_admitted() {
    let admitted = systemprompt_test_fixtures::fixture_admitted();
    let with_files = AdmittedSchedule::of(admitted.built());
    let observed = with_files
        .run_in_scopes("scope_probe_job", async {
            assert_eq!(current_actor(), DocumentActor::system("scope_probe_job"));
            Ok(JobResult::success())
        })
        .await
        .expect("scoped run");
    assert!(observed.success);

    let without =
        systemprompt_test_fixtures::fixture_admitted_without(&[ExtensionId::new("files")]);
    let without_files = AdmittedSchedule::of(without.built());
    let unscoped = without_files
        .run_in_scopes("scope_probe_job", async {
            assert_eq!(current_actor(), unscoped_actor());
            Ok(JobResult::success())
        })
        .await
        .expect("unscoped run");
    assert!(unscoped.success);
}
