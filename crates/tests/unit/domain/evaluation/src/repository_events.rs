//! DB-backed tests for the append-only execution event log: in-order
//! delivery, idempotent redelivery, content conflicts and lease gating.

use crate::repository_workers::Harness;
use systemprompt_evaluation::repository::experiments::{
    ExecutionEvent, ExecutionEventRepository, ExecutionLease, ExecutionStage,
};
use systemprompt_identifiers::EvalWorkerId;

fn event(sequence: i64, stage: ExecutionStage, summary: &str) -> ExecutionEvent {
    ExecutionEvent::builder(sequence, stage)
        .summary(summary.to_owned())
        .build()
        .expect("event")
}

async fn stored_sequences(harness: &Harness, execution: &str) -> Vec<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT sequence FROM eval_execution_events WHERE execution_id = $1 ORDER BY sequence",
    )
    .bind(execution)
    .fetch_all(&harness.pg)
    .await
    .expect("read events")
}

#[tokio::test]
async fn events_append_in_sequence_and_redelivery_is_idempotent() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let events = ExecutionEventRepository::new(harness.pg.clone());

    let provisioning = event(0, ExecutionStage::Provisioning, "sandbox ready");
    events
        .append(&harness.worker, &lease, &provisioning)
        .await
        .expect("first event");
    events
        .append(
            &harness.worker,
            &lease,
            &event(1, ExecutionStage::Context, "case fixtures written"),
        )
        .await
        .expect("second event");
    events
        .append(&harness.worker, &lease, &provisioning)
        .await
        .expect("identical redelivery is accepted");

    assert_eq!(
        stored_sequences(&harness, execution.id.as_str()).await,
        vec![0, 1]
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn out_of_order_and_divergent_events_are_rejected() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let events = ExecutionEventRepository::new(harness.pg.clone());

    events
        .append(
            &harness.worker,
            &lease,
            &event(0, ExecutionStage::Provisioning, "sandbox ready"),
        )
        .await
        .expect("first event");

    assert!(
        events
            .append(
                &harness.worker,
                &lease,
                &event(0, ExecutionStage::Cleanup, "different content")
            )
            .await
            .is_err(),
        "a sequence cannot be rewritten with other content"
    );
    assert!(
        events
            .append(
                &harness.worker,
                &lease,
                &event(3, ExecutionStage::Verification, "skipped ahead")
            )
            .await
            .is_err(),
        "events must arrive without gaps"
    );

    assert_eq!(
        stored_sequences(&harness, execution.id.as_str()).await,
        vec![0]
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn appending_requires_a_live_owned_worker_lease() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let events = ExecutionEventRepository::new(harness.pg.clone());
    let progress = event(0, ExecutionStage::Specification, "draft written");

    let stale = ExecutionLease::builder(execution.id.clone(), harness.worker.id.clone())
        .fencing_token(lease.fencing_token + 1)
        .build()
        .expect("stale lease");
    assert!(
        events
            .append(&harness.worker, &stale, &progress)
            .await
            .is_err()
    );

    let foreign = ExecutionLease::builder(execution.id.clone(), EvalWorkerId::generate())
        .fencing_token(lease.fencing_token)
        .build()
        .expect("foreign lease");
    assert!(
        events
            .append(&harness.worker, &foreign, &progress)
            .await
            .is_err()
    );

    let mut other_environment = harness.worker.clone();
    other_environment.environment = "somewhere-else".to_owned();
    assert!(
        events
            .append(&other_environment, &lease, &progress)
            .await
            .is_err(),
        "a worker record from another environment cannot report progress"
    );

    harness
        .workers()
        .revoke(&harness.owner, &harness.worker.id)
        .await
        .expect("revoke");
    assert!(
        events
            .append(&harness.worker, &lease, &progress)
            .await
            .is_err(),
        "a revoked worker cannot report progress"
    );

    assert!(
        stored_sequences(&harness, execution.id.as_str())
            .await
            .is_empty()
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn events_stop_once_the_execution_lease_expires() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let events = ExecutionEventRepository::new(harness.pg.clone());

    harness.set_lease_expiry(&execution.id, -60.0).await;
    assert!(
        events
            .append(
                &harness.worker,
                &lease,
                &event(0, ExecutionStage::Publication, "published")
            )
            .await
            .is_err()
    );

    harness.cleanup().await;
}

#[test]
fn event_validation_bounds_sequence_and_summary() {
    assert!(
        ExecutionEvent::builder(0, ExecutionStage::Cleanup)
            .build()
            .is_err(),
        "a summary is required"
    );

    for (sequence, summary) in [
        (-1_i64, "negative".to_owned()),
        (1000, "out of range".to_owned()),
        (0, "   ".to_owned()),
        (0, "s".repeat(8193)),
    ] {
        assert!(
            ExecutionEvent::builder(sequence, ExecutionStage::Cleanup)
                .summary(summary.clone())
                .build()
                .is_err(),
            "sequence {sequence} with a {}-byte summary must be rejected",
            summary.len()
        );
    }

    let valid = ExecutionEvent {
        sequence: 999,
        stage: ExecutionStage::Verification,
        summary: "verified".to_owned(),
    };
    assert!(valid.validate().is_ok());
}
