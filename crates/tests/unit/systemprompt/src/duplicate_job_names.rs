//! A job name has exactly one owner, and a job's kernel standing is its
//! admitted owner's stamped standing, never its name: an install job named
//! like a scheduler job is refused at composition, a forged handoff carrying
//! two owners of one name is refused when its schedule is built, and every
//! admitted job keeps the standing the facade stamped on its owner.

use async_trait::async_trait;
use systemprompt::CompositionError;
use systemprompt_extension::admitted::{AdmittedMember, AdmittedParts};
use systemprompt_extension::contribute::{ContributionSink, Contributions};
use systemprompt_extension::{
    Admitted, AdmittedSchedule, DuplicateJobOwner, ExtensionMetadata, StandingTag,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::{Job, JobContext, JobResult, ProviderResult};

use crate::probe::{Probe, build_with};

const SCHEDULER_JOB: &str = "database_cleanup";

struct NamedJob(&'static str);

#[async_trait]
impl Job for NamedJob {
    fn name(&self) -> &'static str {
        self.0
    }

    fn schedule(&self) -> &'static str {
        ""
    }

    async fn execute(&self, _ctx: &JobContext) -> ProviderResult<JobResult> {
        Ok(JobResult::success())
    }
}

fn forged_member(id: &'static str, standing: StandingTag, job: &'static str) -> AdmittedMember {
    let owner = ExtensionId::new(id);
    let mut sink = ContributionSink::default();
    Contributions::__new(owner.clone(), &mut sink).job(NamedJob(job));
    AdmittedMember {
        secret_grants: Vec::new(),
        owner,
        metadata: ExtensionMetadata {
            id,
            name: id,
            version: "0.0.1",
        },
        standing,
        sink,
    }
}

#[test]
fn an_install_job_named_like_a_scheduler_job_is_refused_naming_both_owners() {
    let error = build_with(Probe::new("probe-install", |c| {
        c.job(NamedJob(SCHEDULER_JOB));
    }))
    .expect_err("an install cannot contribute a job name the scheduler owns");
    match error {
        CompositionError::DuplicateJob(DuplicateJobOwner { job, first, second }) => {
            assert_eq!(job, SCHEDULER_JOB);
            assert_eq!(first.as_str(), "scheduler");
            assert_eq!(second.as_str(), "probe-install");
        },
        other => panic!("expected a duplicate job refusal, got {other:?}"),
    }
}

#[test]
fn a_forged_handoff_with_two_owners_of_one_job_name_builds_no_schedule() {
    let admitted = Admitted::__from_facade(AdmittedParts {
        withheld: Vec::new(),
        members: vec![
            forged_member("scheduler", StandingTag::Core, SCHEDULER_JOB),
            forged_member("probe-install", StandingTag::Install, SCHEDULER_JOB),
        ],
        ..AdmittedParts::default()
    });
    let error = AdmittedSchedule::of(admitted.built())
        .expect_err("one job name, two owners: no schedule is built");
    assert_eq!(
        error,
        DuplicateJobOwner {
            job: SCHEDULER_JOB,
            first: ExtensionId::new("scheduler"),
            second: ExtensionId::new("probe-install"),
        }
    );
}

#[test]
fn every_admitted_job_keeps_its_owners_stamped_standing() {
    let admitted = build_with(Probe::new("probe-install", |c| {
        c.job(NamedJob("probe_install_job"));
    }))
    .expect("a uniquely named install job is admitted");
    let schedule = AdmittedSchedule::of(admitted.built()).expect("one owner per job name");

    let owner = |name: &str| {
        schedule
            .owner_of(name)
            .map(|(owner, standing)| (owner.as_str().to_owned(), standing))
    };
    assert_eq!(
        owner(SCHEDULER_JOB),
        Some(("scheduler".to_owned(), StandingTag::Core))
    );
    assert_eq!(
        owner("probe_install_job"),
        Some(("probe-install".to_owned(), StandingTag::Install))
    );
}

#[test]
fn a_member_stamped_install_never_carries_core_standing_for_its_jobs() {
    let admitted = Admitted::__from_facade(AdmittedParts {
        withheld: Vec::new(),
        members: vec![forged_member(
            "scheduler",
            StandingTag::Install,
            "forged_kernel_job",
        )],
        ..AdmittedParts::default()
    });
    let schedule = AdmittedSchedule::of(admitted.built()).expect("one owner");
    assert_eq!(
        schedule
            .owner_of("forged_kernel_job")
            .map(|(_, standing)| standing),
        Some(StandingTag::Install),
        "kernel standing follows the stamped standing, not the owner's id"
    );
}
