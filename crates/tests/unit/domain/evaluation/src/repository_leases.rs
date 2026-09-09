//! DB-backed tests for fenced execution leases: heartbeat renewal, terminal
//! completion, idempotent replay and the rejection of stale or foreign leases.

use crate::repository_workers::Harness;
use systemprompt_evaluation::repository::experiments::{
    ExecutionCompletion, ExecutionLease, TerminalOutcome,
};
use systemprompt_identifiers::{EvalExecutionId, EvalWorkerId};

fn completion(outcome: TerminalOutcome, summary: &str) -> ExecutionCompletion {
    ExecutionCompletion {
        outcome,
        summary: summary.to_owned(),
    }
}

#[tokio::test]
async fn heartbeat_renews_a_live_lease_up_to_its_deadline() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;

    harness.set_lease_expiry(&execution.id, 5.0).await;
    let before = harness.lease_expiry(&execution.id).await;
    harness
        .experiments()
        .heartbeat(&harness.owner, &lease)
        .await
        .expect("heartbeat");
    let after = harness.lease_expiry(&execution.id).await;

    assert!(after > before, "heartbeat must push the lease forward");
    assert!(
        after - before > chrono::Duration::seconds(30),
        "renewal grants a fresh 60-second window, not a nudge"
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn heartbeat_rejects_stale_foreign_and_expired_leases() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let experiments = harness.experiments();

    let stale = ExecutionLease::builder(execution.id.clone(), harness.worker.id.clone())
        .fencing_token(lease.fencing_token + 1)
        .build()
        .expect("stale lease");
    assert!(experiments.heartbeat(&harness.owner, &stale).await.is_err());

    let foreign = ExecutionLease::builder(execution.id.clone(), EvalWorkerId::generate())
        .fencing_token(lease.fencing_token)
        .build()
        .expect("foreign lease");
    assert!(
        experiments
            .heartbeat(&harness.owner, &foreign)
            .await
            .is_err()
    );

    let unknown = ExecutionLease::builder(EvalExecutionId::generate(), harness.worker.id.clone())
        .fencing_token(lease.fencing_token)
        .build()
        .expect("unknown lease");
    assert!(
        experiments
            .heartbeat(&harness.owner, &unknown)
            .await
            .is_err()
    );

    harness.set_lease_expiry(&execution.id, -60.0).await;
    assert!(
        experiments.heartbeat(&harness.owner, &lease).await.is_err(),
        "an expired lease cannot be renewed"
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn completion_records_its_outcome_and_finishes_the_experiment() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;

    harness
        .experiments()
        .complete(
            &harness.owner,
            &lease,
            &completion(TerminalOutcome::Completed, "specification published"),
        )
        .await
        .expect("complete");

    assert_eq!(harness.execution_status(&execution.id).await, "completed");
    assert_eq!(
        harness.experiment_status().await,
        "completed",
        "the last terminal execution closes its experiment"
    );

    let detail = harness
        .experiments()
        .get(&harness.owner, &harness.experiment)
        .await
        .expect("detail");
    let stored = detail.executions[0].result.as_ref().expect("result");
    assert_eq!(stored.outcome, TerminalOutcome::Completed);
    assert_eq!(stored.summary, "specification published");

    harness.cleanup().await;
}

#[tokio::test]
async fn completion_replay_is_idempotent_only_for_identical_results() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (_, lease) = harness.claimed_lease().await;
    let experiments = harness.experiments();
    let recorded = completion(TerminalOutcome::Error, "worker crashed");

    experiments
        .complete(&harness.owner, &lease, &recorded)
        .await
        .expect("complete");
    experiments
        .complete(&harness.owner, &lease, &recorded)
        .await
        .expect("identical replay is accepted");

    let divergent = completion(TerminalOutcome::Blocked, "worker crashed");
    assert!(
        experiments
            .complete(&harness.owner, &lease, &divergent)
            .await
            .is_err(),
        "a finished execution cannot be rewritten with another outcome"
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn completion_requires_a_bounded_summary_and_a_live_lease() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let experiments = harness.experiments();

    for summary in ["   ".to_owned(), "s".repeat(16_001)] {
        assert!(
            experiments
                .complete(
                    &harness.owner,
                    &lease,
                    &completion(TerminalOutcome::Cancelled, &summary)
                )
                .await
                .is_err(),
            "summary of {} bytes must be rejected",
            summary.len()
        );
    }

    let foreign = ExecutionLease::builder(execution.id.clone(), EvalWorkerId::generate())
        .fencing_token(lease.fencing_token)
        .build()
        .expect("foreign lease");
    assert!(
        experiments
            .complete(
                &harness.owner,
                &foreign,
                &completion(TerminalOutcome::BudgetExhausted, "spend ceiling reached")
            )
            .await
            .is_err()
    );

    harness.set_lease_expiry(&execution.id, -60.0).await;
    assert!(
        experiments
            .complete(
                &harness.owner,
                &lease,
                &completion(TerminalOutcome::Completed, "too late")
            )
            .await
            .is_err(),
        "an expired lease cannot report a terminal outcome"
    );
    assert_eq!(harness.execution_status(&execution.id).await, "running");

    harness.cleanup().await;
}

#[test]
fn execution_lease_builder_requires_a_positive_fencing_token() {
    let execution = EvalExecutionId::generate();
    let worker = EvalWorkerId::generate();

    let lease = ExecutionLease::builder(execution.clone(), worker.clone())
        .fencing_token(3)
        .build()
        .expect("lease");
    assert_eq!(lease.execution_id, execution);
    assert_eq!(lease.worker_id, worker);
    assert_eq!(lease.fencing_token, 3);

    assert!(
        ExecutionLease::builder(execution.clone(), worker.clone())
            .build()
            .is_err()
    );
    assert!(
        ExecutionLease::builder(execution, worker)
            .fencing_token(0)
            .build()
            .is_err()
    );
}
