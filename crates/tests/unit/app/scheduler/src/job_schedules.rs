//! Every registered job's cron expression, checked against the parser that
//! actually schedules it.
//!
//! `Job::new_async` parses the expression at registration. A typo there does
//! not crash anything and does not fail a build — the job is simply never
//! registered, and never runs. Nothing downstream reports a job that was
//! supposed to fire and did not, so this is asserted against
//! `tokio_cron_scheduler` itself rather than against a hand-rolled regex that
//! could accept expressions the real parser rejects.

use systemprompt_scheduler::jobs::{
    BackfillSessionGeoJob, BehavioralAnalysisJob, CleanupEmptyContextsJob,
    CleanupInactiveSessionsJob, DatabaseCleanupJob, EvaluationLoopJob, GhostSessionCleanupJob,
    MaliciousIpBlacklistJob, NoJsCleanupJob,
};
use systemprompt_traits::Job;
use tokio_cron_scheduler::Job as CronJob;

fn all_jobs() -> Vec<Box<dyn Job>> {
    vec![
        Box::new(BackfillSessionGeoJob),
        Box::new(BehavioralAnalysisJob),
        Box::new(CleanupEmptyContextsJob),
        Box::new(CleanupInactiveSessionsJob),
        Box::new(DatabaseCleanupJob),
        Box::new(EvaluationLoopJob),
        Box::new(GhostSessionCleanupJob),
        Box::new(MaliciousIpBlacklistJob),
        Box::new(NoJsCleanupJob),
    ]
}

// Why: this is the check the scheduler performs at registration. A job whose
// expression it rejects is dropped silently, so the failure looks like "the
// job never ran" long after the change that caused it.
//
// Scoped to jobs that declare themselves schedulable. `backfill_session_geo`
// is manual-only and carries an empty schedule deliberately, so requiring one
// of every job would assert the opposite of what that job intends.
#[test]
fn every_schedulable_job_has_a_schedule_the_scheduler_accepts() {
    let mut checked = 0;
    for job in all_jobs().iter().filter(|job| job.schedulable()) {
        let schedule = job.schedule();
        assert!(
            CronJob::new_async(schedule, |_uuid, _lock| Box::pin(async {})).is_ok(),
            "{} has a schedule the scheduler will not accept: {schedule}",
            job.name()
        );
        checked += 1;
    }

    assert!(
        checked > 0,
        "no job declared itself schedulable, so this asserted nothing"
    );
}

// Why: the pairing is the invariant. A job that is not schedulable has no
// schedule to honour, and one that is must not be left with an empty
// expression — which parses as nothing and registers as nothing.
#[test]
fn a_job_is_schedulable_exactly_when_it_carries_a_schedule() {
    for job in all_jobs() {
        assert_eq!(
            job.schedulable(),
            !job.schedule().trim().is_empty(),
            "{} declares schedulable={} but its schedule is {:?}",
            job.name(),
            job.schedulable(),
            job.schedule()
        );
    }
}

// Why: jobs are looked up and configured by name. Two sharing one would make
// a configuration entry ambiguous, and one of them unreachable.
#[test]
fn no_two_jobs_share_a_name() {
    let mut names: Vec<&str> = all_jobs().iter().map(|job| job.name()).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();

    assert_eq!(before, names.len(), "two jobs share a name: {names:?}");
}

// Why: an empty name or description reaches an operator listing jobs, and an
// unnamed job cannot be configured at all.
#[test]
fn every_job_carries_a_name_and_a_description() {
    for job in all_jobs() {
        assert!(
            !job.name().is_empty(),
            "a job with no name cannot be configured"
        );
        assert!(
            !job.description().trim().is_empty(),
            "{} has no description",
            job.name()
        );
    }
}
