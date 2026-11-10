//! DB-backed, through `ClaimLoop` and `A2aRunHandler` with the `/bin/sh`
//! fake host: a run in flight at shutdown is released to `queued` with its
//! resume session and finished by the next loop; a cancel request still ends
//! the run `cancelled`; a handler whose lease was taken stops without
//! writing to the run.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_agent_runs::repository::{AgentRunRow, NewAgentRun};
use systemprompt_agent_runs::service::{ClaimLoop, RunHandler};
use systemprompt_agent_runs::{AgentRunsError, RunRequest};
use systemprompt_identifiers::{AgentRunId, HostSessionId};
use tokio_util::sync::CancellationToken;

use crate::repository::{expire_lease, request};
use crate::runner_handler::{Rig, claim, handler, rig};

async fn queue_in(r: &Rig) -> AgentRunId {
    let id = AgentRunId::generate();
    let request = RunRequest {
        workspace: r.workspace.path().to_path_buf(),
        ..request()
    };
    r.f.repo
        .insert_run(NewAgentRun {
            id: &id,
            origin_kind: &r.f.origin_kind,
            origin_id: "origin-interrupt",
            user_id: &r.f.user_id,
            request: &request,
            not_before: None,
            task_id: None,
            context_id: None,
        })
        .await
        .expect("insert run");
    id
}

async fn wait_for(r: &Rig, id: &AgentRunId, done: impl Fn(&AgentRunRow) -> bool) -> AgentRunRow {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let row = r.f.repo.find_run(id).await.expect("find").expect("row");
            if done(&row) {
                return row;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("run reached the expected state")
}

fn claim_loop(r: &Rig, claimer: &str) -> ClaimLoop {
    let mut claim = ClaimLoop::new(r.f.repo.clone(), &r.f.origin_kind, claimer.to_owned());
    claim.poll_every = Duration::from_millis(50);
    claim
}

#[tokio::test]
async fn shutdown_releases_an_in_flight_run_and_the_next_loop_resumes_it() {
    let r = rig().await;
    let id = queue_in(&r).await;
    let shutdown = CancellationToken::new();
    let first = handler(&r, "echo INIT session-interrupted; sleep 30", None).await;
    let looping = tokio::spawn(claim_loop(&r, "worker-1").run(shutdown.clone(), Arc::new(first)));
    wait_for(&r, &id, |row| row.host_session_id.is_some()).await;

    shutdown.cancel();
    tokio::time::timeout(Duration::from_secs(20), looping)
        .await
        .expect("loop stopped")
        .expect("loop joined");

    let session = HostSessionId::try_new("session-interrupted").expect("session");
    let row = r.f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.status, "queued");
    assert_eq!(row.attempt, 1);
    assert!(row.claimed_by.is_none());
    assert!(row.finished_at.is_none());
    assert_eq!(row.error_class, None);
    let request: RunRequest = serde_json::from_value(row.request).expect("request");
    assert_eq!(request.resume_session_id, Some(session.clone()));

    let shutdown = CancellationToken::new();
    let second = handler(&r, "echo resumed", None).await;
    let looping = tokio::spawn(claim_loop(&r, "worker-2").run(shutdown.clone(), Arc::new(second)));
    let row = wait_for(&r, &id, |row| row.finished_at.is_some()).await;
    shutdown.cancel();
    looping.await.expect("loop joined");
    assert_eq!(row.status, "succeeded");
    assert_eq!(row.claimed_by.as_deref(), Some("worker-2"));
    let request: RunRequest = serde_json::from_value(row.request).expect("request");
    assert_eq!(request.resume_session_id, Some(session));
}

#[tokio::test]
async fn a_cancel_request_during_a_run_finishes_it_cancelled_even_at_shutdown() {
    let r = rig().await;
    let mut claimed = claim(&r, None).await;
    let id = claimed.row.id.clone();
    let shutdown = CancellationToken::new();
    claimed.run_token = shutdown.child_token();
    claimed.shutdown = shutdown.clone();
    let running = handler(&r, "sleep 30", None).await;
    let task = tokio::spawn(async move { running.run(claimed).await });
    wait_for(&r, &id, |row| row.status == "running").await;

    r.f.repo.update_cancel_requested(&id).await.expect("cancel");
    shutdown.cancel();
    task.await.expect("join").expect("run");

    let row = r.f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.status, "cancelled");
    assert_eq!(row.error_class.as_deref(), Some("ok"));
    assert!(row.finished_at.is_some());
}

#[tokio::test]
async fn a_cancel_request_alone_stops_the_host_and_finishes_cancelled() {
    let r = rig().await;
    let claimed = claim(&r, None).await;
    let id = claimed.row.id.clone();
    let running = handler(&r, "sleep 30", None).await;
    let task = tokio::spawn(async move { running.run(claimed).await });
    wait_for(&r, &id, |row| row.status == "running").await;

    r.f.repo.update_cancel_requested(&id).await.expect("cancel");
    tokio::time::timeout(Duration::from_secs(20), task)
        .await
        .expect("cancel stopped the run")
        .expect("join")
        .expect("run");

    let row = r.f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.status, "cancelled");
    assert!(row.finished_at.is_some());
}

#[tokio::test]
async fn a_handler_whose_lease_was_taken_stops_without_writing() {
    let r = rig().await;
    let claimed = claim(&r, None).await;
    let id = claimed.row.id.clone();
    let running = handler(&r, "sleep 1; echo late; exit 3", None).await;
    let task = tokio::spawn(async move { running.run(claimed).await });
    wait_for(&r, &id, |row| row.status == "running").await;

    expire_lease(&r.f, &id).await;
    r.f.repo
        .reclaim_stale(&r.f.origin_kind, 3)
        .await
        .expect("reclaim");
    let taken =
        r.f.repo
            .claim_queued(&r.f.origin_kind, "worker-b", Duration::from_secs(90))
            .await
            .expect("claim")
            .expect("reclaimed run");
    assert_eq!(taken.row.id, id);

    let result = task.await.expect("join");
    assert!(
        matches!(result, Err(AgentRunsError::LeaseLost { ref run_id }) if *run_id == id),
        "{result:?}"
    );
    let row = r.f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.status, "preparing");
    assert_eq!(row.claimed_by.as_deref(), Some("worker-b"));
    assert_eq!(row.attempt, 2);
    assert!(row.finished_at.is_none());
    assert_eq!(row.error_class, None);
    assert_eq!(r.f.repo.count_events(&id).await.expect("events"), 0);
}
