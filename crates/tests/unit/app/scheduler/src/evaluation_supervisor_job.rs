//! Contract of the fenced evaluator supervisor scheduler job.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_scheduler::jobs::EvaluationSupervisorJob;
use systemprompt_traits::{Job, JobScope};

#[test]
fn supervisor_is_a_five_second_node_local_job() {
    let job = EvaluationSupervisorJob;

    assert_eq!(job.name(), "evaluation_supervisor");
    assert_eq!(job.schedule(), "*/5 * * * * *");
    assert_eq!(job.scope(), JobScope::Node);
    assert!(job.description().contains("fenced leases"));
    assert!(job.description().contains("isolated Docker networks"));
}
