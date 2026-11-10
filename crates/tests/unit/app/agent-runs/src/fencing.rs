//! DB-backed: every write to a claimed run is fenced by the claim's lease.
//! A claim that was reclaimed, or released and claimed again, changes
//! nothing and reports `LeaseLost`; events under one lease are numbered
//! gaplessly; a release re-queues the run with its resume session.

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use systemprompt_agent_runs::repository::{Fenced, Released, RunStarted};
use systemprompt_agent_runs::{
    ErrorClass, RunEvent, RunEventPayload, RunOutcome, RunRequest, RunStatus, RunnerLevel,
};
use systemprompt_identifiers::{AgentRunId, HostSessionId};
use systemprompt_models::managed::AssetDigest;

use crate::repository::{Fixture, claim, expire_lease, fixture, queue};

fn runner_event(message: &str) -> RunEvent {
    let payload = RunEventPayload::Runner {
        message: message.to_owned(),
        level: RunnerLevel::Info,
    };
    RunEvent {
        seq: 0,
        kind: payload.kind(),
        tool_name: None,
        tool_use_id: None,
        payload,
        created_at: Utc::now(),
    }
}

fn outcome(status: RunStatus) -> RunOutcome {
    RunOutcome {
        status,
        error_class: ErrorClass::Ok,
        session_id: None,
        cost_microdollars: 0,
        input_tokens: 0,
        output_tokens: 0,
        turns: 0,
        duration_ms: 0,
        structured_output: None,
        exit_code: Some(0),
        error: None,
        final_text: None,
    }
}

fn started(dir: &Path) -> RunStarted<'_> {
    RunStarted {
        run_dir: dir,
        pid: Some(7),
        gateway_session_id: None,
        api_key_id: None,
    }
}

async fn reclaimed_by_b(f: &Fixture, id: &AgentRunId) {
    expire_lease(f, id).await;
    assert_eq!(
        f.repo
            .reclaim_stale(&f.origin_kind, 3)
            .await
            .expect("reclaim"),
        vec![id.clone()]
    );
}

#[tokio::test]
async fn a_reclaimed_claim_cannot_write_to_the_run() {
    let f = fixture().await;
    let id = queue(&f).await;
    let a = claim(&f, "a").await;
    reclaimed_by_b(&f, &id).await;
    let b = claim(&f, "b").await;
    let b_session = HostSessionId::try_new("session-b").expect("session");
    assert_eq!(
        f.repo
            .update_started(&id, &b, started(Path::new("/tmp/b")))
            .await
            .expect("b started"),
        Fenced::Applied
    );
    assert_eq!(
        f.repo
            .update_host_session(&id, &b, &b_session)
            .await
            .expect("b session"),
        Fenced::Applied
    );
    assert_eq!(
        f.repo
            .insert_events_batch(&id, &b, &[runner_event("b")])
            .await
            .expect("b events"),
        Fenced::Applied
    );

    let a_session = HostSessionId::try_new("session-a").expect("session");
    let digest = AssetDigest::of(b"a-digest");
    let stale = [
        f.repo
            .update_started(&id, &a, started(Path::new("/tmp/a")))
            .await
            .expect("a started"),
        f.repo
            .update_host_session(&id, &a, &a_session)
            .await
            .expect("a session"),
        f.repo
            .insert_events_batch(&id, &a, &[runner_event("a")])
            .await
            .expect("a events"),
        f.repo
            .record_harness_digest(&id, &a, &digest)
            .await
            .expect("a digest"),
        f.repo
            .finish(&id, &a, &outcome(RunStatus::Succeeded), None)
            .await
            .expect("a finish"),
        f.repo
            .renew_lease(&id, &a, std::time::Duration::from_secs(90))
            .await
            .expect("a renew"),
    ];
    assert_eq!(stale, [Fenced::LeaseLost; 6]);
    assert_eq!(
        f.repo.release(&id, &a, 3).await.expect("a release"),
        Released::LeaseLost
    );

    let row = f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.status, "running");
    assert_eq!(row.claimed_by.as_deref(), Some("b"));
    assert_eq!(row.attempt, 2);
    assert_eq!(row.run_dir.as_deref(), Some("/tmp/b"));
    assert_eq!(row.host_session_id, Some(b_session));
    assert_eq!(row.harness_digest, None);
    assert!(row.finished_at.is_none());
    assert!(row.lease_until.is_some());
    let events = f.repo.list_events_after(&id, -1, 10).await.expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload, runner_event("b").payload);
}

#[tokio::test]
async fn a_released_claim_is_fenced_when_the_same_claimer_takes_the_run_again() {
    let f = fixture().await;
    let id = queue(&f).await;
    let first = claim(&f, "same").await;
    assert_eq!(
        f.repo.release(&id, &first, 3).await.expect("release"),
        Released::Requeued
    );
    let second = claim(&f, "same").await;
    assert_eq!(second.attempt(), first.attempt());
    assert_ne!(second, first);

    assert_eq!(
        f.repo
            .finish(&id, &first, &outcome(RunStatus::Failed), None)
            .await
            .expect("stale finish"),
        Fenced::LeaseLost
    );
    assert_eq!(
        f.repo
            .finish(&id, &second, &outcome(RunStatus::Succeeded), None)
            .await
            .expect("finish"),
        Fenced::Applied
    );
    let row = f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.status, "succeeded");
}

#[tokio::test]
async fn release_requeues_with_the_resume_session_and_the_same_attempt() {
    let f = fixture().await;
    let id = queue(&f).await;
    let lease = claim(&f, "a").await;
    let session = HostSessionId::try_new("session-release").expect("session");
    assert_eq!(
        f.repo
            .update_host_session(&id, &lease, &session)
            .await
            .expect("session"),
        Fenced::Applied
    );
    assert_eq!(
        f.repo.release(&id, &lease, 3).await.expect("release"),
        Released::Requeued
    );
    assert_eq!(
        f.repo
            .release(&id, &lease, 3)
            .await
            .expect("second release"),
        Released::LeaseLost
    );

    let row = f.repo.find_run(&id).await.expect("find").expect("row");
    assert_eq!(row.status, "queued");
    assert_eq!(row.attempt, 1);
    assert!(row.claimed_by.is_none());
    assert!(row.lease_until.is_none());
    assert!(row.finished_at.is_none());
    let request: RunRequest = serde_json::from_value(row.request).expect("request");
    assert_eq!(request.resume_session_id, Some(session.clone()));

    let next = f
        .repo
        .claim_queued(&f.origin_kind, "b", std::time::Duration::from_secs(90))
        .await
        .expect("claim")
        .expect("released run is claimable");
    assert_eq!(next.row.id, id);
    let request: RunRequest = serde_json::from_value(next.row.request).expect("request");
    assert_eq!(request.resume_session_id, Some(session));
}

#[tokio::test]
async fn concurrent_event_batches_under_one_lease_get_gapless_unique_seqs() {
    let f = Arc::new(fixture().await);
    let id = queue(&f).await;
    let lease = claim(&f, "a").await;
    let mut handles = Vec::new();
    for writer in 0..8 {
        let (f, id, lease) = (Arc::clone(&f), id.clone(), lease.clone());
        handles.push(tokio::spawn(async move {
            let batch: Vec<RunEvent> = (0..5)
                .map(|n| runner_event(&format!("w{writer}-{n}")))
                .collect();
            f.repo
                .insert_events_batch(&id, &lease, &batch)
                .await
                .expect("insert")
        }));
    }
    for handle in handles {
        assert_eq!(handle.await.expect("join"), Fenced::Applied);
    }
    let events = f
        .repo
        .list_events_after(&id, -1, 100)
        .await
        .expect("events");
    let seqs: Vec<i64> = events.iter().map(|e| e.seq).collect();
    assert_eq!(seqs, (0..40).collect::<Vec<i64>>());
    for writer in 0..8 {
        let mine: Vec<String> = events
            .iter()
            .filter_map(|e| match &e.payload {
                RunEventPayload::Runner { message, .. }
                    if message.starts_with(&format!("w{writer}-")) =>
                {
                    Some(message.clone())
                },
                _ => None,
            })
            .collect();
        let expected: Vec<String> = (0..5).map(|n| format!("w{writer}-{n}")).collect();
        assert_eq!(mine, expected, "writer {writer} batch kept its order");
    }
}
