//! Contract of the fenced evaluator supervisor scheduler job.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_traits::{Job, JobScope};

/// The job as the scheduler sees it: looked up through `inventory`, never
/// constructed, so the test also proves the `submit_job!` registration
/// survives linking. A consumer binary once carried an explicit anchor
/// against this being dropped; this is the proof that made it unnecessary.
fn registered_supervisor() -> &'static dyn Job {
    inventory::iter::<&'static dyn Job>
        .into_iter()
        .copied()
        .find(|job| job.name() == "evaluation_supervisor")
        .expect("evaluation_supervisor is registered in inventory")
}

#[test]
fn supervisor_is_a_five_second_node_local_job() {
    let job = registered_supervisor();

    assert_eq!(job.name(), "evaluation_supervisor");
    assert_eq!(job.schedule(), "*/5 * * * * *");
    assert_eq!(job.scope(), JobScope::Node);
    assert!(job.description().contains("fenced leases"));
    assert!(job.description().contains("isolated Docker networks"));
}
