//! DB-backed failure scenarios for approvals, restart recovery and cleanup
//! evidence.

use super::repository_workers::{Harness, MODEL, PROVIDER};
use systemprompt_evaluation::EvaluationError;
use systemprompt_evaluation::repository::experiments::{
    AdmissionRequest, ApprovalDecision, ApprovalVerdict, CleanupReport, RequestAdmission,
};
use systemprompt_identifiers::{ModelId, ProviderId, UserId};

#[tokio::test]
async fn expired_and_foreign_approval_decisions_fail_closed() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (_, lease) = harness.claimed_lease().await;
    let lifecycle =
        crate::seams::lifecycle(&harness.pg, crate::fixture_admission::fixture_admission());
    let digest = "a".repeat(64);
    let approval = lifecycle
        .request_approval(
            &harness.owner,
            &lease,
            serde_json::json!({"operation":"fixture-write"}),
            &digest,
        )
        .await
        .expect("approval wait");
    sqlx::query!(
        "UPDATE eval_execution_approvals SET expires_at=NOW()-INTERVAL '1 second' WHERE id=$1",
        approval.id.as_str()
    )
    .execute(&harness.pg)
    .await
    .expect("expire approval");

    let foreign = UserId::new(format!("foreign-{}", uuid::Uuid::new_v4()));
    assert!(matches!(
        lifecycle
            .decide_approval(
                &foreign,
                &ApprovalVerdict {
                    actor: &foreign,
                    approval: &approval.id,
                    decision: ApprovalDecision::Approve,
                    observed_precondition: &digest,
                },
            )
            .await,
        Err(EvaluationError::ExperimentConflict(_))
    ));
    assert!(matches!(
        lifecycle
            .decide_approval(
                &harness.owner,
                &ApprovalVerdict {
                    actor: &harness.owner,
                    approval: &approval.id,
                    decision: ApprovalDecision::Approve,
                    observed_precondition: &digest,
                },
            )
            .await,
        Err(EvaluationError::ExperimentConflict(_))
    ));
    let status = sqlx::query_scalar!(
        "SELECT status FROM eval_execution_approvals WHERE id=$1",
        approval.id.as_str()
    )
    .fetch_one(&harness.pg)
    .await
    .expect("approval status");
    assert_eq!(status, "expired");
    harness.cleanup().await;
}

#[tokio::test]
async fn restart_marks_expired_work_uncertain_and_never_requeues_it() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    harness.set_lease_expiry(&execution.id, -1.0).await;
    let lifecycle =
        crate::seams::lifecycle(&harness.pg, crate::fixture_admission::fixture_admission());
    assert_eq!(
        lifecycle
            .reconcile_restart(&harness.owner)
            .await
            .expect("reconcile"),
        1
    );
    assert_eq!(harness.execution_status(&execution.id).await, "error");
    let row = sqlx::query!(
        "SELECT status,attempts FROM eval_execution_cleanup WHERE execution_id=$1",
        execution.id.as_str()
    )
    .fetch_one(&harness.pg)
    .await
    .expect("cleanup reconciliation");
    assert_eq!(row.status, "retrying");
    assert_eq!(row.attempts, 1);
    let summary = sqlx::query_scalar!(
        "SELECT result->>'summary' FROM eval_executions WHERE id=$1",
        execution.id.as_str()
    )
    .fetch_one(&harness.pg)
    .await
    .expect("restart summary");
    assert!(
        summary
            .as_deref()
            .is_some_and(|value| value.contains("uncertain writes require review"))
    );
    assert!(
        !lifecycle
            .execution_is_live(&harness.owner, &lease.execution_id)
            .await
            .expect("live state")
    );
    assert!(
        harness
            .experiments()
            .claim(&harness.owner, &harness.worker.id)
            .await
            .expect("claim after restart")
            .is_some(),
        "only other queued matrix work may proceed"
    );
    harness.cleanup().await;
}

#[tokio::test]
async fn failed_cleanup_is_durable_and_owner_fenced() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (_, lease) = harness.claimed_lease().await;
    let lifecycle =
        crate::seams::lifecycle(&harness.pg, crate::fixture_admission::fixture_admission());
    lifecycle
        .record_cleanup(
            &harness.owner,
            &lease,
            &CleanupReport {
                container_id: Some("eval-client"),
                network_id: Some("eval-network"),
                succeeded: false,
                error: Some("injected cleanup refusal"),
            },
        )
        .await
        .expect("record failed cleanup");
    let row = sqlx::query!(
        "SELECT status,last_error FROM eval_execution_cleanup WHERE execution_id=$1",
        lease.execution_id.as_str()
    )
    .fetch_one(&harness.pg)
    .await
    .expect("cleanup evidence");
    assert_eq!(row.status, "failed");
    assert_eq!(row.last_error.as_deref(), Some("injected cleanup refusal"));
    let foreign = UserId::new(format!("foreign-{}", uuid::Uuid::new_v4()));
    assert!(matches!(
        lifecycle
            .record_cleanup(
                &foreign,
                &lease,
                &CleanupReport {
                    container_id: None,
                    network_id: None,
                    succeeded: true,
                    error: None,
                },
            )
            .await,
        Err(EvaluationError::ExperimentConflict(_))
    ));
    harness.cleanup().await;
}

#[tokio::test]
async fn an_execution_awaiting_approval_keeps_its_reservations_held() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (_, lease) = harness.claimed_lease().await;
    let gateway = crate::seams::gateway(&harness.pg, crate::fixture_admission::fixture_admission());
    let access = crate::seams::capabilities(&harness.pg)
        .issue(&harness.owner, &lease)
        .await
        .expect("issue");
    let request = harness
        .seed_pending_request(access.session_id.as_str())
        .await;
    let admitted = gateway
        .admit(
            &AdmissionRequest::builder(&harness.owner, &access.session_id)
                .request(&request)
                .model(&ModelId::new(MODEL))
                .provider(&ProviderId::new(PROVIDER))
                .bound_microdollars(50_000)
                .build()
                .expect("admission request"),
        )
        .await
        .expect("admit");
    assert!(matches!(admitted, RequestAdmission::Reserved(_)));
    assert_eq!(harness.budget().await, (50_000, 0));

    let lifecycle =
        crate::seams::lifecycle(&harness.pg, crate::fixture_admission::fixture_admission());
    lifecycle
        .request_approval(
            &harness.owner,
            &lease,
            serde_json::json!({"operation":"pause"}),
            &"c".repeat(64),
        )
        .await
        .expect("approval wait");

    let budgets = crate::seams::budgets(&harness.pg);
    assert_eq!(
        budgets
            .retain_orphaned(&harness.owner)
            .await
            .expect("retain"),
        0,
        "a paused execution is live: its pending reservation is not an orphan"
    );
    assert_eq!(harness.budget().await, (50_000, 0));

    harness.complete_request(&request, 12_500).await;
    assert!(
        gateway
            .settle_recorded(&harness.owner, &request)
            .await
            .expect("settle")
    );
    assert_eq!(harness.budget().await, (0, 12_500));
    harness.cleanup().await;
}
