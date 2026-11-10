//! DB-backed: a release re-queues a run without spending an attempt and
//! counts itself in `request.releases`; once a run has been released
//! `max_releases` times the next release fails it as `release_cap_exceeded` and
//! fails its A2A task. Through `ClaimLoop`, a shutdown past the bound ends
//! the run failed instead of queued.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_agent_runs::repository::Released;
use systemprompt_agent_runs::service::ClaimLoop;
use tokio_util::sync::CancellationToken;

use crate::repository::{claim, fixture, queue};
use crate::run_interrupts::{queue_in, wait_for};
use crate::runner_handler::{handler, rig};
use crate::task_link::{insert_task, queue_for, task_state};

#[tokio::test]
async fn releases_up_to_the_bound_requeue_and_the_next_fails_the_run() {
    let f = fixture().await;
    let id = queue(&f).await;
    for round in 1..=2 {
        let lease = claim(&f, "worker").await;
        assert_eq!(lease.attempt(), 1);
        assert_eq!(
            f.repo.release(&id, &lease, 2).await.expect("release"),
            Released::Requeued
        );
        let row = f.repo.find_run(&id).await.expect("find").expect("row");
        assert_eq!(row.status, "queued");
        assert_eq!(row.attempt, 1);
        assert_eq!(row.request["releases"], round);
    }

    let lease = claim(&f, "worker").await;
    assert_eq!(
        f.repo.release(&id, &lease, 2).await.expect("release"),
        Released::Exhausted
    );
    let row = f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.status, "failed");
    assert_eq!(row.error_class.as_deref(), Some("release_cap_exceeded"));
    assert!(row.finished_at.is_some());
    assert!(row.claimed_by.is_none());
    assert!(row.lease_until.is_none());
    assert_eq!(row.request["releases"], 2);
    assert!(
        f.repo
            .claim_queued(&f.origin_kind, "worker", Duration::from_secs(90))
            .await
            .expect("claim")
            .is_none()
    );
    assert_eq!(
        f.repo.release(&id, &lease, 2).await.expect("stale release"),
        Released::LeaseLost
    );
}

#[tokio::test]
async fn an_exhausted_release_fails_the_linked_task() {
    let f = fixture().await;
    let (task_id, context_id) = insert_task(&f).await;
    let id = queue_for(&f, &task_id, &context_id).await;
    let lease = claim(&f, "worker").await;
    assert_eq!(
        f.repo.release(&id, &lease, 0).await.expect("release"),
        Released::Exhausted
    );
    assert_eq!(task_state(&f, &task_id).await, "TASK_STATE_FAILED");
    let row = f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.error_class.as_deref(), Some("release_cap_exceeded"));
}

#[tokio::test]
async fn a_shutdown_past_the_release_bound_fails_the_run() {
    let r = rig().await;
    let id = queue_in(&r).await;
    let shutdown = CancellationToken::new();
    let host = handler(&r, "echo INIT session-bound; sleep 30", None).await;
    let mut claim = ClaimLoop::new(r.f.repo.clone(), &r.f.origin_kind, "bound".to_owned());
    claim.poll_every = Duration::from_millis(50);
    claim.max_releases = 0;
    let looping = tokio::spawn(claim.run(shutdown.clone(), Arc::new(host)));
    wait_for(&r, &id, |row| row.host_session_id.is_some()).await;

    shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(20), looping)
        .await
        .expect("loop stopped")
        .expect("loop joined");

    let row = r.f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.status, "failed");
    assert_eq!(row.error_class.as_deref(), Some("release_cap_exceeded"));
    assert!(row.finished_at.is_some());
    assert_eq!(row.attempt, 1);
}
