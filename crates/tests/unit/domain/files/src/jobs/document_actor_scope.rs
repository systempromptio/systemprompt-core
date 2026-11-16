//! The files member contributes the document actor job run scope: a job body
//! run inside it observes the system document actor named after the job.
//! Outside the scope the actor falls back to the capped unscoped agent.

use std::sync::Arc;

use systemprompt_extension::StandingTag;
use systemprompt_files::FilesExtension;
use systemprompt_files::documents::agents::{DocumentActor, current_actor, unscoped_actor};
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
    let names: Vec<&str> = contributed_scopes().iter().map(|scope| scope.name()).collect();
    assert_eq!(names, vec!["document_actor"]);
}
