//! Stateful approval decisions retain exact operations, preconditions and
//! ownership.
use super::repository_workers::Harness;
use systemprompt_evaluation::repository::experiments::{
    ApprovalAuthorization, ApprovalDecision, ApprovalVerdict, EvaluationLifecycleRepository,
};

fn lifecycle(harness: &Harness) -> EvaluationLifecycleRepository {
    crate::seams::lifecycle(&harness.pg, crate::fixture_admission::fixture_admission())
}

#[tokio::test]
async fn pending_operation_retries_share_one_approval_and_changed_preconditions_do_not_resume() {
    let h = Harness::start()
        .await
        .expect("approval acceptance requires PostgreSQL");
    let (_, lease) = h.claimed_lease().await;
    let repository = lifecycle(&h);
    let operation =
        serde_json::json!({"operation":"write","path":"fixture.txt","value":"reviewed"});
    let digest = "a".repeat(64);
    let before = h.budget().await;
    let ApprovalAuthorization::Pending(id) = repository
        .authorize_operation(&h.owner, &lease.execution_id, &operation, &digest)
        .await
        .unwrap()
    else {
        panic!("new operation must require approval")
    };
    assert_eq!(
        h.execution_status(&lease.execution_id).await,
        "awaiting_approval"
    );
    for _ in 0..2 {
        assert!(
            matches!(repository.authorize_operation(&h.owner, &lease.execution_id, &operation, &digest).await.unwrap(), ApprovalAuthorization::Pending(repeated) if repeated == id)
        );
    }
    assert!(
        repository
            .authorize_operation(&h.owner, &lease.execution_id, &operation, &"b".repeat(64))
            .await
            .is_err()
    );
    assert!(
        repository
            .authorize_operation(
                &h.owner,
                &lease.execution_id,
                &serde_json::json!({"operation":"delete"}),
                &digest
            )
            .await
            .is_err()
    );
    assert!(
        repository
            .decide_approval(
                &h.owner,
                &ApprovalVerdict {
                    actor: &h.owner,
                    approval: &id,
                    decision: ApprovalDecision::Approve,
                    observed_precondition: &"b".repeat(64)
                }
            )
            .await
            .is_err()
    );
    assert_eq!(
        h.execution_status(&lease.execution_id).await,
        "awaiting_approval"
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM eval_execution_approvals WHERE owner_id=$1")
            .bind(h.owner.as_str())
            .fetch_one(&h.pg)
            .await
            .unwrap();
    assert_eq!(count, 1);
    repository
        .decide_approval(
            &h.owner,
            &ApprovalVerdict {
                actor: &h.owner,
                approval: &id,
                decision: ApprovalDecision::Approve,
                observed_precondition: &digest,
            },
        )
        .await
        .unwrap();
    assert_eq!(h.execution_status(&lease.execution_id).await, "queued");
    assert!(
        matches!(repository.authorize_operation(&h.owner, &lease.execution_id, &operation, &digest).await.unwrap(), ApprovalAuthorization::Authorized(approved) if approved == id)
    );
    let replay = repository
        .authorize_operation(&h.owner, &lease.execution_id, &operation, &digest)
        .await;
    assert!(
        replay.is_err(),
        "an approval authorises exactly one operation; the second authorisation used to succeed: {replay:?}"
    );
    assert_eq!(h.budget().await, before);
    h.cleanup().await;
}

#[tokio::test]
async fn denial_cannot_be_reversed_by_retry_or_used_to_authorize_execution() {
    let h = Harness::start()
        .await
        .expect("approval acceptance requires PostgreSQL");
    let (_, lease) = h.claimed_lease().await;
    let repository = lifecycle(&h);
    let operation = serde_json::json!({"operation":"fixture-write"});
    let digest = "a".repeat(64);
    let approval = repository
        .request_approval(&h.owner, &lease, operation.clone(), &digest)
        .await
        .unwrap();
    repository
        .decide_approval(
            &h.owner,
            &ApprovalVerdict {
                actor: &h.owner,
                approval: &approval.id,
                decision: ApprovalDecision::Deny,
                observed_precondition: &digest,
            },
        )
        .await
        .unwrap();
    assert!(
        repository
            .decide_approval(
                &h.owner,
                &ApprovalVerdict {
                    actor: &h.owner,
                    approval: &approval.id,
                    decision: ApprovalDecision::Approve,
                    observed_precondition: &digest
                }
            )
            .await
            .is_err()
    );
    assert!(
        repository
            .authorize_operation(&h.owner, &lease.execution_id, &operation, &digest)
            .await
            .is_err()
    );
    let retained: (String, Option<String>) =
        sqlx::query_as("SELECT status,decided_by FROM eval_execution_approvals WHERE id=$1")
            .bind(approval.id.as_str())
            .fetch_one(&h.pg)
            .await
            .unwrap();
    assert_eq!(retained, ("denied".to_owned(), Some(h.owner.to_string())));
    assert_eq!(
        h.execution_status(&lease.execution_id).await,
        "awaiting_approval"
    );
    assert_eq!(h.budget().await, (0, 0));
    h.cleanup().await;
}

#[tokio::test]
async fn concurrent_approve_and_deny_have_one_durable_winner() {
    let h = Harness::start()
        .await
        .expect("approval acceptance requires PostgreSQL");
    let (_, lease) = h.claimed_lease().await;
    let repository = lifecycle(&h);
    let digest = "a".repeat(64);
    let approval = repository
        .request_approval(
            &h.owner,
            &lease,
            serde_json::json!({"operation":"fixture-write"}),
            &digest,
        )
        .await
        .unwrap();
    let approve = ApprovalVerdict {
        actor: &h.owner,
        approval: &approval.id,
        decision: ApprovalDecision::Approve,
        observed_precondition: &digest,
    };
    let deny = ApprovalVerdict {
        decision: ApprovalDecision::Deny,
        ..approve
    };
    let (approved, denied) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(
            repository.decide_approval(&h.owner, &approve),
            repository.decide_approval(&h.owner, &deny)
        )
    })
    .await
    .expect("bounded concurrent decisions");
    assert_ne!(approved.is_ok(), denied.is_ok());
    let status: String =
        sqlx::query_scalar("SELECT status FROM eval_execution_approvals WHERE id=$1")
            .bind(approval.id.as_str())
            .fetch_one(&h.pg)
            .await
            .unwrap();
    assert_eq!(
        status,
        if approved.is_ok() {
            "approved"
        } else {
            "denied"
        }
    );
    assert_eq!(
        h.execution_status(&lease.execution_id).await,
        if approved.is_ok() {
            "queued"
        } else {
            "awaiting_approval"
        }
    );
    assert_eq!(h.budget().await, (0, 0));
    h.cleanup().await;
}

#[tokio::test]
async fn cancelled_approval_wait_never_regains_execution_credentials_or_requeues_on_restart() {
    let h = Harness::start()
        .await
        .expect("approval acceptance requires PostgreSQL");
    let (_, lease) = h.claimed_lease().await;
    let repository = lifecycle(&h);
    let digest = "a".repeat(64);
    let operation = serde_json::json!({"operation":"fixture-write"});
    let approval = repository
        .request_approval(&h.owner, &lease, operation.clone(), &digest)
        .await
        .unwrap();
    h.experiments()
        .cancel(&h.owner, &h.experiment)
        .await
        .unwrap();
    assert_eq!(h.execution_status(&lease.execution_id).await, "cancelled");
    assert!(
        repository
            .request_approval(&h.owner, &lease, operation, &digest)
            .await
            .is_err()
    );
    assert!(
        crate::seams::capabilities(&h.pg)
            .issue(&h.owner, &lease)
            .await
            .is_err()
    );
    for _ in 0..2 {
        repository.reconcile_restart(&h.owner).await.unwrap();
        assert!(
            h.experiments()
                .claim(&h.owner, &h.worker.id)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(h.execution_status(&lease.execution_id).await, "cancelled");
    }
    let retained: String =
        sqlx::query_scalar("SELECT status FROM eval_execution_approvals WHERE id=$1")
            .bind(approval.id.as_str())
            .fetch_one(&h.pg)
            .await
            .unwrap();
    assert_eq!(
        retained, "pending",
        "cancellation must not invent an approval decision"
    );
    assert_eq!(h.budget().await, (0, 0));
    h.cleanup().await;
}
