//! Per-instant run claims, missed-run policies, start-time requirements and
//! member-scoped sealed reads. Two replicas with skewed clocks claim every
//! instant of a hundred exactly once; a node-scoped job claims once per
//! instance; a claim's completion is fenced on its token; each missed-run
//! policy runs the instants it promises after a restart; a job whose
//! `requires` is unmet refuses the scheduler start; a job reads the sealed
//! value its own member provided and never another member's.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use systemprompt_extension::{
    ContributionError, Contributions, Extension, ExtensionMetadata, ProvideError, Resolved,
};
use systemprompt_identifiers::{InstanceId, JobName};
use systemprompt_manifest::services::UnlistedJobs;
use systemprompt_scheduler::{
    CronInstants, JobConfig, RunKey, SchedulerClock, SchedulerConfig, SchedulerError,
    SchedulerRepository, SchedulerService,
};
use systemprompt_test_fixtures::{
    core_members, fixture_admitted_of, test_database_url, test_db_pool, unique_name,
};
use systemprompt_traits::{Job, JobContext, JobResult, MissedRunPolicy, StateRequirement};

const EVERY_SECOND: &str = "* * * * * *";
const EVERY_MINUTE: &str = "0 * * * * *";
const MEMBER: &str = "s5a_runs_owner";
const OTHER: &str = "s5a_runs_other";

const SKIP_JOB: &str = "s5a_missed_skip";
const ONCE_JOB: &str = "s5a_missed_once";
const CATCH_JOB: &str = "s5a_missed_catch_up";
const UNMET_JOB: &str = "s5a_unmet_requirement";
const SEALED_JOB: &str = "s5a_sealed_reader";

static OWN_SEALED_SEEN: AtomicU64 = AtomicU64::new(0);
static OTHER_SEALED_SEEN: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct OwnSealed;
#[derive(Debug)]
struct OtherSealed;
#[derive(Debug)]
struct NeverProvided;

#[derive(Debug)]
struct FixedClock(DateTime<Utc>);

impl SchedulerClock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

#[derive(Clone, Copy)]
struct PolicyJob {
    name: &'static str,
    policy: MissedRunPolicy,
}

#[async_trait]
impl Job for PolicyJob {
    fn name(&self) -> &'static str {
        self.name
    }
    fn schedule(&self) -> &'static str {
        EVERY_MINUTE
    }
    fn enabled(&self) -> bool {
        false
    }
    fn missed_runs(&self) -> MissedRunPolicy {
        self.policy
    }
    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        Ok(JobResult::success().with_stats(1, 0))
    }
}

#[derive(Clone, Copy)]
struct UnmetJob;

#[async_trait]
impl Job for UnmetJob {
    fn name(&self) -> &'static str {
        UNMET_JOB
    }
    fn schedule(&self) -> &'static str {
        EVERY_MINUTE
    }
    fn enabled(&self) -> bool {
        false
    }
    fn requires(&self) -> Vec<StateRequirement> {
        vec![StateRequirement::of::<Arc<NeverProvided>>()]
    }
    async fn execute(
        &self,
        _ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        Ok(JobResult::success())
    }
}

#[derive(Clone, Copy)]
struct SealedReader;

#[async_trait]
impl Job for SealedReader {
    fn name(&self) -> &'static str {
        SEALED_JOB
    }
    fn schedule(&self) -> &'static str {
        ""
    }
    fn enabled(&self) -> bool {
        false
    }
    async fn execute(
        &self,
        ctx: &JobContext,
    ) -> systemprompt_provider_contracts::ProviderResult<JobResult> {
        if ctx.find::<Arc<OwnSealed>>().is_some() {
            OWN_SEALED_SEEN.fetch_add(1, Ordering::SeqCst);
        }
        if ctx.find::<Arc<OtherSealed>>().is_some() {
            OTHER_SEALED_SEEN.fetch_add(1, Ordering::SeqCst);
        }
        Ok(JobResult::success())
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct OwnerMember;

impl Extension for OwnerMember {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: MEMBER,
            name: "s5a runs owner",
            version: "0.0.1",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.job(PolicyJob {
            name: SKIP_JOB,
            policy: MissedRunPolicy::Skip,
        })
        .job(PolicyJob {
            name: ONCE_JOB,
            policy: MissedRunPolicy::RunOnce,
        })
        .job(PolicyJob {
            name: CATCH_JOB,
            policy: MissedRunPolicy::CatchUp { max: 3 },
        })
        .job(UnmetJob)
        .job(SealedReader)
        .provide_sealed(|_: &Resolved<'_>| -> Result<Arc<OwnSealed>, ProvideError> {
            Ok(Arc::new(OwnSealed))
        });
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct OtherMember;

impl Extension for OtherMember {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: OTHER,
            name: "s5a runs other",
            version: "0.0.1",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.provide_sealed(
            |_: &Resolved<'_>| -> Result<Arc<OtherSealed>, ProvideError> {
                Ok(Arc::new(OtherSealed))
            },
        );
        Ok(())
    }
}

async fn app() -> (
    Arc<systemprompt_runtime::AppContext>,
    Arc<systemprompt_extension::AdmittedSchedule>,
) {
    let mut members = core_members();
    members.push(Box::new(OwnerMember));
    members.push(Box::new(OtherMember));
    crate::test_extension::app_over(&test_database_url(), fixture_admitted_of(members)).await
}

fn config(jobs: &[&str]) -> SchedulerConfig {
    SchedulerConfig {
        enabled: true,
        jobs: jobs
            .iter()
            .map(|name| JobConfig::new(JobName::new(*name)))
            .collect(),
        bootstrap_jobs: Vec::new(),
        distributed_lock: true,
        unlisted_jobs: UnlistedJobs::Warn,
    }
}

fn base_instant() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2031, 1, 1, 0, 0, 0)
        .single()
        .expect("valid instant")
}

mod claims_db {
    use super::*;

    #[tokio::test]
    async fn two_replicas_with_skewed_clocks_run_each_of_a_hundred_instants_once() {
        let pool = test_db_pool().await;
        let runs = SchedulerRepository::new(&pool).runs().clone();
        let job = JobName::new(unique_name("s5a_skew"));
        let cron = CronInstants::parse(EVERY_SECOND).expect("cron");
        let replica_a = InstanceId::new(unique_name("node-a"));
        let replica_b = InstanceId::new(unique_name("node-b"));
        let skews = [
            chrono::Duration::milliseconds(2000),
            chrono::Duration::milliseconds(-2000),
        ];
        let base = base_instant();

        let mut won = 0;
        for i in 0..100 {
            let instant = base + chrono::Duration::seconds(i * 10);
            for (replica, skew) in [(&replica_a, skews[0]), (&replica_b, skews[1])] {
                let real = instant - skew;
                let local = real + skew + chrono::Duration::milliseconds(i % 7 * 40);
                let scheduled = cron.at_or_before(local).expect("instant");
                assert_eq!(
                    scheduled, instant,
                    "each replica keys the tick by the instant"
                );
                if runs
                    .claim(&job, scheduled, &RunKey::Cluster, replica)
                    .await
                    .expect("claim")
                    .is_some()
                {
                    won += 1;
                }
            }
        }

        assert_eq!(won, 100, "one claim wins per instant");
        assert_eq!(runs.count_runs(&job).await.expect("count"), 100);
    }

    #[tokio::test]
    async fn a_node_scoped_instant_is_claimed_once_per_instance() {
        let pool = test_db_pool().await;
        let runs = SchedulerRepository::new(&pool).runs().clone();
        let job = JobName::new(unique_name("s5a_node"));
        let a = InstanceId::new(unique_name("node-a"));
        let b = InstanceId::new(unique_name("node-b"));
        let at = base_instant();

        let first = runs
            .claim(&job, at, &RunKey::Node(a.clone()), &a)
            .await
            .expect("claim");
        let again = runs
            .claim(&job, at, &RunKey::Node(a.clone()), &a)
            .await
            .expect("claim");
        let other = runs
            .claim(&job, at, &RunKey::Node(b.clone()), &b)
            .await
            .expect("claim");

        assert!(first.is_some(), "the first claim on an instance wins");
        assert!(
            again.is_none(),
            "the same instance never runs an instant twice"
        );
        assert!(other.is_some(), "another instance runs the same instant");
    }

    #[tokio::test]
    async fn a_run_is_closed_once_and_only_under_its_own_token() {
        let pool = test_db_pool().await;
        let runs = SchedulerRepository::new(&pool).runs().clone();
        let job = JobName::new(unique_name("s5a_fenced"));
        let node = InstanceId::new(unique_name("node"));
        let claim = runs
            .claim(&job, base_instant(), &RunKey::Cluster, &node)
            .await
            .expect("claim")
            .expect("won");

        let closed = runs
            .finish(&claim, systemprompt_scheduler::JobStatus::Success)
            .await
            .expect("finish");
        let twice = runs
            .finish(&claim, systemprompt_scheduler::JobStatus::Failed)
            .await
            .expect("finish");

        assert!(closed, "the claim closes its own run row");
        assert!(!twice, "a closed run is never rewritten");
    }

    #[test]
    fn missed_instants_are_bounded_and_oldest_first() {
        let cron = CronInstants::parse(EVERY_MINUTE).expect("cron");
        let last = base_instant();
        let now = last + chrono::Duration::minutes(10) + chrono::Duration::seconds(5);

        let once = cron.missed(last, now, 1);
        let three = cron.missed(last, now, 3);
        let none = cron.missed(now, now, 5);

        assert_eq!(once, vec![last + chrono::Duration::minutes(10)]);
        assert_eq!(
            three,
            vec![
                last + chrono::Duration::minutes(8),
                last + chrono::Duration::minutes(9),
                last + chrono::Duration::minutes(10),
            ]
        );
        assert!(
            none.is_empty(),
            "nothing after the latest instant is missed"
        );
    }
}

mod missed_runs_db {
    use super::*;

    async fn seed_last(pool: &systemprompt_database::DbPool, job: &str, at: DateTime<Utc>) {
        sqlx::query!("DELETE FROM scheduled_job_runs WHERE job_name = $1", job)
            .execute(&*pool.write_pool())
            .await
            .expect("clear runs");
        SchedulerRepository::new(pool)
            .runs()
            .claim(
                &JobName::new(job),
                at,
                &RunKey::Cluster,
                &InstanceId::new(unique_name("s5a-seed")),
            )
            .await
            .expect("seed last run")
            .expect("seeded");
    }

    #[tokio::test]
    async fn restart_after_a_gap_honours_each_policy() {
        let pool = test_db_pool().await;
        let last = base_instant();
        for job in [SKIP_JOB, ONCE_JOB, CATCH_JOB] {
            seed_last(&pool, job, last).await;
        }
        let now = last + chrono::Duration::minutes(10) + chrono::Duration::seconds(5);
        let (app, schedule) = app().await;

        let startup = SchedulerService::new(
            config(&[SKIP_JOB, ONCE_JOB, CATCH_JOB]),
            &pool,
            app,
            schedule,
        )
        .with_clock(Arc::new(FixedClock(now)))
        .start()
        .await
        .expect("scheduler starts");
        tokio::time::sleep(Duration::from_secs(5)).await;
        if let Some(handle) = startup.handle {
            handle.shutdown().await.expect("shutdown");
        }

        let runs = SchedulerRepository::new(&pool).runs().clone();
        let count = |job: &'static str| {
            let runs = runs.clone();
            async move { runs.count_runs(&JobName::new(job)).await.expect("count") }
        };
        assert_eq!(count(SKIP_JOB).await, 1, "Skip runs no missed instant");
        assert_eq!(
            count(ONCE_JOB).await,
            2,
            "RunOnce runs the latest missed instant"
        );
        assert_eq!(count(CATCH_JOB).await, 4, "CatchUp {{ max: 3 }} runs three");
    }
}

mod requirements_db {
    use super::*;

    #[tokio::test]
    async fn a_job_with_an_unmet_requirement_refuses_the_start() {
        let pool = test_db_pool().await;
        let (app, schedule) = app().await;

        let refused = SchedulerService::new(config(&[UNMET_JOB]), &pool, app, schedule)
            .start()
            .await;

        match refused {
            Err(SchedulerError::UnmetRequirement { job, ty }) => {
                assert_eq!(job.as_str(), UNMET_JOB);
                assert!(
                    ty.contains("NeverProvided"),
                    "the refusal names the type: {ty}"
                );
            },
            other => panic!("expected UnmetRequirement, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_job_reads_its_own_members_sealed_value_and_never_another_members() {
        let (app, schedule) = app().await;
        let own_before = OWN_SEALED_SEEN.load(Ordering::SeqCst);
        let other_before = OTHER_SEALED_SEEN.load(Ordering::SeqCst);

        let report = crate::test_extension::job_service(
            app,
            schedule,
            vec![JobConfig::new(JobName::new(SEALED_JOB))],
        )
        .run_job(&JobName::new(SEALED_JOB), &HashMap::new())
        .await;

        assert!(report.success, "the job runs: {report:?}");
        assert_eq!(
            OWN_SEALED_SEEN.load(Ordering::SeqCst) - own_before,
            1,
            "the owning member's sealed value is in its job's context"
        );
        assert_eq!(
            OTHER_SEALED_SEEN.load(Ordering::SeqCst),
            other_before,
            "another member's sealed value is withheld"
        );
    }
}
