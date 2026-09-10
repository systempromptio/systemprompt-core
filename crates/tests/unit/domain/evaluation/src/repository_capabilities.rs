//! DB-backed tests for execution capability tokens: issuance bound to a live
//! lease, session reuse across reissue, prior-token revocation, and the
//! conditions under which a token stops authenticating.

use crate::repository_workers::Harness;
use systemprompt_evaluation::repository::experiments::{
    EXECUTION_TOKEN_PREFIX, ExecutionCapabilityRepository, ExecutionCompletion, ExecutionIdentity,
    ExecutionLease, TerminalOutcome,
};
use systemprompt_identifiers::{EvalExecutionId, EvalWorkerId, UserId};
use uuid::Uuid;

#[tokio::test]
async fn issued_capability_authenticates_as_a_scoped_execution_principal() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let capabilities = ExecutionCapabilityRepository::new(harness.pg.clone());

    let access = capabilities
        .issue(&harness.owner, &lease)
        .await
        .expect("issue");
    assert!(access.expose_token().starts_with(EXECUTION_TOKEN_PREFIX));

    let principal = capabilities
        .authenticate(access.expose_token(), &harness.environment)
        .await
        .expect("authenticate");
    assert_eq!(principal.session_id, access.session_id);
    assert_eq!(principal.identity.owner_id, harness.owner);
    assert_eq!(principal.identity.execution_id, execution.id);
    assert_eq!(
        principal.identity.roles,
        vec!["user".to_owned()],
        "an execution principal never inherits its owner's roles"
    );

    assert!(
        capabilities
            .authenticate(access.expose_token(), "another-environment")
            .await
            .is_err(),
        "a capability is bound to the worker environment that issued it"
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn reissue_reuses_the_session_and_revokes_the_prior_token() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (_, lease) = harness.claimed_lease().await;
    let capabilities = ExecutionCapabilityRepository::new(harness.pg.clone());

    let first = capabilities
        .issue(&harness.owner, &lease)
        .await
        .expect("first issue");
    let second = capabilities
        .issue(&harness.owner, &lease)
        .await
        .expect("second issue");

    assert_eq!(
        first.session_id, second.session_id,
        "the fenced execution keeps one gateway session"
    );
    assert_ne!(first.expose_token(), second.expose_token());
    assert!(
        capabilities
            .authenticate(first.expose_token(), &harness.environment)
            .await
            .is_err(),
        "reissue revokes the token it replaces"
    );
    assert!(
        capabilities
            .authenticate(second.expose_token(), &harness.environment)
            .await
            .is_ok()
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn issuance_requires_an_eligible_running_execution() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let capabilities = ExecutionCapabilityRepository::new(harness.pg.clone());

    let stale = ExecutionLease::builder(execution.id.clone(), harness.worker.id.clone())
        .fencing_token(lease.fencing_token + 1)
        .build()
        .expect("stale lease");
    assert!(capabilities.issue(&harness.owner, &stale).await.is_err());

    let foreign_worker = ExecutionLease::builder(execution.id.clone(), EvalWorkerId::generate())
        .fencing_token(lease.fencing_token)
        .build()
        .expect("foreign lease");
    assert!(
        capabilities
            .issue(&harness.owner, &foreign_worker)
            .await
            .is_err()
    );

    let unknown = ExecutionLease::builder(EvalExecutionId::generate(), harness.worker.id.clone())
        .fencing_token(lease.fencing_token)
        .build()
        .expect("unknown lease");
    assert!(capabilities.issue(&harness.owner, &unknown).await.is_err());

    assert!(
        capabilities
            .issue(&UserId::new("someone-else"), &lease)
            .await
            .is_err(),
        "issuance is owner scoped"
    );

    harness.set_lease_expiry(&execution.id, -60.0).await;
    assert!(
        capabilities.issue(&harness.owner, &lease).await.is_err(),
        "an expired lease cannot mint credentials"
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn a_capability_stops_authenticating_once_its_execution_finishes() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (_, lease) = harness.claimed_lease().await;
    let capabilities = ExecutionCapabilityRepository::new(harness.pg.clone());
    let access = capabilities
        .issue(&harness.owner, &lease)
        .await
        .expect("issue");

    harness
        .experiments()
        .complete(
            &harness.owner,
            &lease,
            &ExecutionCompletion {
                outcome: TerminalOutcome::Completed,
                summary: "done".to_owned(),
            },
        )
        .await
        .expect("complete");

    assert!(
        capabilities
            .authenticate(access.expose_token(), &harness.environment)
            .await
            .is_err(),
        "a finished execution's credential is inert"
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn malformed_and_unknown_capabilities_are_rejected() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let capabilities = ExecutionCapabilityRepository::new(harness.pg.clone());

    for token in [
        "speval_worker-token".to_owned(),
        format!("{EXECUTION_TOKEN_PREFIX}{}", "x".repeat(200)),
        format!("{EXECUTION_TOKEN_PREFIX}{}", Uuid::new_v4()),
    ] {
        assert!(
            capabilities
                .authenticate(&token, &harness.environment)
                .await
                .is_err(),
            "token {token} must be rejected"
        );
    }

    harness.cleanup().await;
}

#[test]
fn execution_identity_builder_carries_owner_execution_and_roles() {
    let owner = UserId::new("owner");
    let execution = EvalExecutionId::generate();
    let identity = ExecutionIdentity::builder(owner.clone(), execution.clone())
        .roles(vec!["user".to_owned()])
        .build();

    assert_eq!(identity.owner_id, owner);
    assert_eq!(identity.execution_id, execution);
    assert_eq!(identity.roles, vec!["user".to_owned()]);

    let bare = ExecutionIdentity::builder(owner, execution).build();
    assert!(bare.roles.is_empty());
}
