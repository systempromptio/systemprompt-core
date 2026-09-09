//! DB-backed tests for the worker assignment reader: frozen inputs are handed
//! out only against a live, owned lease, and every revision is re-verified
//! against its stored digest before it leaves the server.

use crate::repository_workers::Harness;
use systemprompt_evaluation::experiments::resources::ResourceContent;
use systemprompt_evaluation::repository::experiments::{
    AssignmentRepository, ExecutionLease, RevisionRepository,
};
use systemprompt_identifiers::EvalWorkerId;

#[tokio::test]
async fn assignment_hands_a_live_lease_its_frozen_inputs() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;

    let assignment = AssignmentRepository::new(harness.pg.clone())
        .get(&harness.worker, &lease)
        .await
        .expect("assignment");

    assert_eq!(assignment.execution.id, execution.id);
    assert_eq!(assignment.execution.case_revision_id, harness.case_revision);
    assert_eq!(assignment.spec.rubric, harness.rubric_revision);
    assert_eq!(assignment.spec.cases, vec![harness.case_revision.clone()]);
    let ResourceContent::Case(case) = &assignment.case else {
        panic!("case revision must deserialize as case content");
    };
    assert_eq!(case.prompt, "Write a specification");
    assert!(matches!(assignment.rubric, ResourceContent::Rubric(_)));
    assert_eq!(
        assignment.skill_bundle.digest().expect("bundle digest"),
        harness.bundle_digest
    );
    assert_eq!(
        assignment
            .configuration
            .digest()
            .expect("configuration digest"),
        harness.configuration_digest
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn assignment_requires_a_live_owned_worker_lease() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let assignments = AssignmentRepository::new(harness.pg.clone());

    let stale = ExecutionLease::builder(execution.id.clone(), harness.worker.id.clone())
        .fencing_token(lease.fencing_token + 1)
        .build()
        .expect("stale lease");
    assert!(assignments.get(&harness.worker, &stale).await.is_err());

    let foreign = ExecutionLease::builder(execution.id.clone(), EvalWorkerId::generate())
        .fencing_token(lease.fencing_token)
        .build()
        .expect("foreign lease");
    assert!(assignments.get(&harness.worker, &foreign).await.is_err());

    let mut other_environment = harness.worker.clone();
    other_environment.environment = "somewhere-else".to_owned();
    assert!(
        assignments.get(&other_environment, &lease).await.is_err(),
        "a worker record from another environment cannot collect inputs"
    );

    harness
        .workers()
        .revoke(&harness.owner, &harness.worker.id)
        .await
        .expect("revoke");
    assert!(
        assignments.get(&harness.worker, &lease).await.is_err(),
        "a revoked worker cannot collect inputs"
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn assignment_stops_when_the_lease_expires() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let assignments = AssignmentRepository::new(harness.pg.clone());

    assignments
        .get(&harness.worker, &lease)
        .await
        .expect("assignment while live");
    harness.set_lease_expiry(&execution.id, -60.0).await;
    assert!(assignments.get(&harness.worker, &lease).await.is_err());

    harness.cleanup().await;
}

#[tokio::test]
async fn a_tampered_revision_digest_fails_the_assignment() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (_, lease) = harness.claimed_lease().await;
    let assignments = AssignmentRepository::new(harness.pg.clone());

    sqlx::query("UPDATE eval_resource_revisions SET digest = $2 WHERE id = $1")
        .bind(harness.case_revision.as_str())
        .bind("f".repeat(64))
        .execute(&harness.pg)
        .await
        .expect("tamper digest");

    assert!(
        assignments.get(&harness.worker, &lease).await.is_err(),
        "a revision whose stored digest no longer covers its content is not served"
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn a_case_slot_holding_a_rubric_revision_fails_the_assignment() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let assignments = AssignmentRepository::new(harness.pg.clone());

    let decoy = RevisionRepository::new(harness.pg.clone())
        .create(
            &harness.owner,
            "decoy-rubric",
            &ResourceContent::Rubric(crate::repository_workers::rubric_content()),
        )
        .await
        .expect("decoy rubric");
    sqlx::query("UPDATE eval_executions SET case_revision_id = $2 WHERE id = $1")
        .bind(execution.id.as_str())
        .bind(decoy.as_str())
        .execute(&harness.pg)
        .await
        .expect("repoint case revision");

    assert!(
        assignments.get(&harness.worker, &lease).await.is_err(),
        "a case slot must hold a case revision that the spec actually lists"
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn an_out_of_range_variant_index_fails_the_assignment() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let assignments = AssignmentRepository::new(harness.pg.clone());

    sqlx::query("UPDATE eval_executions SET variant_index = 7 WHERE id = $1")
        .bind(execution.id.as_str())
        .execute(&harness.pg)
        .await
        .expect("repoint variant");

    assert!(
        assignments.get(&harness.worker, &lease).await.is_err(),
        "an execution pointing outside the frozen variant matrix is not served"
    );

    harness.cleanup().await;
}
