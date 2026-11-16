//! The scheduler runs a job only while its owning member is admitted:
//! leaving a member out of the composition removes its jobs from the
//! `AdmittedSchedule`, and leaves every other member's jobs in place.

use systemprompt_identifiers::ExtensionId;
use systemprompt_test_fixtures::fixture_admitted_without;

use crate::test_extension::{core_schedule, schedule_of};

#[test]
fn with_nothing_disabled_every_core_owner_contributes_its_jobs() {
    let schedule = core_schedule();
    let (owner, _) = schedule.job("database_cleanup").expect("database_cleanup is admitted");
    assert_eq!(owner.as_str(), "scheduler");
    assert!(schedule.contains_job("mcp_session_cleanup"));
}

#[test]
fn a_disabled_member_contributes_no_jobs() {
    let mcp = ExtensionId::new("mcp");
    let full = core_schedule();
    let owned: Vec<&str> = full
        .jobs()
        .iter()
        .filter(|(owner, _)| *owner == mcp)
        .map(|(_, job)| job.name())
        .collect();
    assert!(!owned.is_empty(), "the mcp member owns a job");

    let without = schedule_of(&fixture_admitted_without(std::slice::from_ref(&mcp)));
    for name in &owned {
        assert!(
            !without.contains_job(name),
            "{name} belongs to the disabled member mcp"
        );
    }
    assert!(without.jobs().iter().all(|(owner, _)| *owner != mcp));
    assert!(without.contains_job("database_cleanup"));
}

#[test]
fn an_owner_that_is_not_admitted_has_no_jobs() {
    let schedule = core_schedule();
    assert!(
        schedule
            .jobs()
            .iter()
            .all(|(owner, _)| owner.as_str() != "no-such-extension")
    );
}
