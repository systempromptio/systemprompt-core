//! With the files crate linked, every job run goes through
//! `run_in_job_scopes`, and the job body observes the system document actor
//! named after the job. Outside the scope the actor falls back to the capped
//! unscoped agent.

use systemprompt_files::documents::agents::{DocumentActor, current_actor, unscoped_actor};
use systemprompt_traits::{JobResult, registered_job_run_scopes, run_in_job_scopes};

#[tokio::test]
async fn a_job_run_acts_as_the_system_actor_named_after_the_job() {
    let observed = std::sync::Mutex::new(None);
    let result = run_in_job_scopes("scope_probe_job", async {
        *observed.lock().expect("observed lock") = Some(current_actor());
        Ok(JobResult::success())
    })
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
fn the_files_crate_registers_its_job_run_scope() {
    assert!(
        registered_job_run_scopes()
            .any(|registration| registration.owner == "files"
                && registration.scope.name() == "document_actor"),
        "linking systemprompt-files must register the document actor scope"
    );
}
