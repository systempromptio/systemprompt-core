//! DB-backed tests for `ReleaseStampRepository`: work runs once per release,
//! an older binary never reruns a newer release's work, a failed run leaves
//! no stamp, and a held stamp lock reports contention.

use std::cell::Cell;
use std::time::Duration;

use systemprompt_database::{
    ReleaseGate, ReleaseRun, ReleaseStampError, ReleaseStampRepository, StampOutcome, release_key,
};
use systemprompt_identifiers::{AdvisoryLockKey, JobName};
use systemprompt_traits::AdvisoryLocks;

use crate::services::db_helper::test_pool;

fn name() -> JobName {
    JobName::new(format!("stamp_{}", uuid::Uuid::new_v4().simple()))
}

fn run<'a>(name: &'a JobName, release: &'a str, gate: ReleaseGate) -> ReleaseRun<'a> {
    ReleaseRun {
        name,
        release,
        gate,
        wait: Duration::from_secs(5),
    }
}

async fn counted(
    repo: &ReleaseStampRepository,
    run: ReleaseRun<'_>,
    calls: &Cell<u32>,
) -> StampOutcome<u32> {
    repo.run_once(run, || async {
        calls.set(calls.get() + 1);
        Ok::<_, std::io::Error>(calls.get())
    })
    .await
    .expect("stamped run")
}

#[tokio::test]
async fn work_runs_once_per_release_under_if_behind() {
    let db = test_pool().await;
    let repo = ReleaseStampRepository::new(&db);
    let name = name();
    let calls = Cell::new(0);

    assert_eq!(
        counted(&repo, run(&name, "0.70.0", ReleaseGate::IfBehind), &calls).await,
        StampOutcome::Ran(1)
    );
    assert_eq!(
        repo.recorded(&name).await.expect("read").as_deref(),
        Some("0.70.0")
    );
    assert_eq!(
        counted(&repo, run(&name, "0.70.0", ReleaseGate::IfBehind), &calls).await,
        StampOutcome::Current
    );
    assert_eq!(
        counted(
            &repo,
            run(&name, "0.70.0", ReleaseGate::UnlessNewer),
            &calls
        )
        .await,
        StampOutcome::Ran(2),
        "UnlessNewer reruns the current release"
    );
    assert_eq!(
        counted(&repo, run(&name, "0.71.0", ReleaseGate::IfBehind), &calls).await,
        StampOutcome::Ran(3)
    );
}

#[tokio::test]
async fn an_older_binary_never_reruns_a_newer_release() {
    let db = test_pool().await;
    let repo = ReleaseStampRepository::new(&db);
    let name = name();
    let calls = Cell::new(0);

    counted(&repo, run(&name, "0.71.0", ReleaseGate::IfBehind), &calls).await;
    let outcome = counted(
        &repo,
        run(&name, "0.70.9", ReleaseGate::UnlessNewer),
        &calls,
    )
    .await;
    assert_eq!(
        outcome,
        StampOutcome::NewerRecorded {
            recorded: "0.71.0".to_owned()
        }
    );
    assert_eq!(calls.get(), 1);
}

#[tokio::test]
async fn a_failed_run_records_no_stamp() {
    let db = test_pool().await;
    let repo = ReleaseStampRepository::new(&db);
    let name = name();

    let error = repo
        .run_once(run(&name, "0.70.0", ReleaseGate::IfBehind), || async {
            Err::<(), _>(std::io::Error::other("ingest failed"))
        })
        .await
        .unwrap_err();
    assert!(matches!(error, ReleaseStampError::Run(_)), "{error}");
    assert!(repo.recorded(&name).await.expect("read").is_none());
}

#[tokio::test]
async fn a_held_stamp_lock_reports_contention() {
    let db = test_pool().await;
    let repo = ReleaseStampRepository::new(&db);
    let name = name();
    let locks = systemprompt_database::SessionAdvisoryLocks::new(&db);
    let guard = locks
        .try_acquire(&AdvisoryLockKey::new(format!(
            "release_stamp:{}",
            name.as_str()
        )))
        .await
        .expect("lock")
        .expect("granted");

    let outcome = repo
        .run_once(
            ReleaseRun {
                wait: Duration::from_millis(300),
                ..run(&name, "0.70.0", ReleaseGate::IfBehind)
            },
            || async { Ok::<_, std::io::Error>(()) },
        )
        .await
        .expect("run");
    assert_eq!(outcome, StampOutcome::Contended);
    guard.release().await;
}

#[test]
fn release_keys_compare_numerically_and_ignore_pre_release_tags() {
    assert!(release_key("0.10.0") > release_key("0.9.9"));
    assert_eq!(release_key("0.70.0-rc.1"), release_key("0.70.0"));
    assert_eq!(release_key("1.2.3+build.7"), vec![1, 2, 3]);
}
