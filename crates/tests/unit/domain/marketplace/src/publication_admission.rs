//! Inventory-sync admission is a separate seam: the reviewed HTTP path still
//! demands an attested experiment, and the sync path only moves forward.

use super::*;
use systemprompt_marketplace::inventory::PublishGuard;
use systemprompt_marketplace::managed::{ManagedError, ManagedResolution, PublicationAdmission};

async fn adopted(f: &Fixture) -> (ManagedResourceId, ResourceRevisionId) {
    f.skill("local", true);
    f.refresh().await;
    InventoryService::new(f.repository.clone())
        .publish_latest(
            &BaselineScope {
                owner: &f.owner,
                actor: &f.owner,
                root: f.root.path(),
                services: &ServicesConfig::default(),
            },
            &mut PublishGuard::default(),
        )
        .await
        .expect("adopt");
    let ManagedResolution::Published {
        resource_id,
        revision_id,
        ..
    } = f
        .repository
        .resolve_managed(&f.owner, ResourceKind::Skill, "local")
        .await
        .expect("resolution")
    else {
        panic!("adopted")
    };
    (resource_id, revision_id)
}

async fn edited_revision(f: &Fixture) -> ResourceRevisionId {
    std::fs::write(f.root.path().join("skills/local/SKILL.md"), "# edited\n").expect("edit");
    f.refresh().await;
    f.baselines()
        .await
        .remove(0)
        .revision_id
        .expect("captured revision")
}

fn request(
    resource_id: &ManagedResourceId,
    revision_id: Option<ResourceRevisionId>,
    action: PublicationAction,
    evidence: serde_json::Value,
) -> PublicationRequest {
    PublicationRequest {
        resource_id: resource_id.clone(),
        revision_id,
        action,
        expected_generation: 1,
        operation_key: format!("admission-{}", uuid::Uuid::new_v4()),
        comparison_evidence: serde_json::from_value(evidence).expect("evidence object"),
        limitations: String::new(),
    }
}

#[tokio::test]
async fn reviewed_path_does_not_accept_inventory_refresh_evidence() {
    let f = Fixture::new().await;
    let (resource, _) = adopted(&f).await;
    let revision = edited_revision(&f).await;
    let result = f
        .repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &request(
                &resource,
                Some(revision),
                PublicationAction::PublishImprovement,
                serde_json::json!({"source": "inventory_refresh"}),
            ),
        )
        .await;
    assert!(matches!(result, Err(ManagedError::Conflict(_))));
    assert!(matches!(
        f.repository
            .resolve_managed(&f.owner, ResourceKind::Skill, "local")
            .await
            .expect("resolution"),
        ManagedResolution::Published { generation: 1, .. }
    ));
}

#[tokio::test]
async fn inventory_sync_admission_refuses_withdraw_and_rollback() {
    let f = Fixture::new().await;
    let (resource, published) = adopted(&f).await;
    let evidence = serde_json::json!({"source": "inventory_refresh"});
    let withdraw = f
        .repository
        .publish_with_admission(
            &f.owner,
            &f.owner,
            &request(
                &resource,
                None,
                PublicationAction::Withdraw,
                evidence.clone(),
            ),
            PublicationAdmission::InventorySync,
        )
        .await;
    assert!(matches!(withdraw, Err(ManagedError::Conflict(_))));
    let rollback = f
        .repository
        .publish_with_admission(
            &f.owner,
            &f.owner,
            &request(
                &resource,
                Some(published),
                PublicationAction::Rollback,
                evidence,
            ),
            PublicationAdmission::InventorySync,
        )
        .await;
    assert!(matches!(rollback, Err(ManagedError::Conflict(_))));
    assert_eq!(
        f.repository
            .list_publication_history(&f.owner, &resource)
            .await
            .expect("history")
            .len(),
        1
    );
}

#[tokio::test]
async fn inventory_sync_admission_requires_refresh_evidence() {
    let f = Fixture::new().await;
    let (resource, _) = adopted(&f).await;
    let revision = edited_revision(&f).await;
    let result = f
        .repository
        .publish_with_admission(
            &f.owner,
            &f.owner,
            &request(
                &resource,
                Some(revision),
                PublicationAction::PublishImprovement,
                serde_json::json!({"source": "manual"}),
            ),
            PublicationAdmission::InventorySync,
        )
        .await;
    assert!(matches!(result, Err(ManagedError::Invalid(_))));
}

#[tokio::test]
async fn an_experiment_id_is_only_accepted_on_an_improvement_review() {
    let f = Fixture::new().await;
    let (resource, published) = adopted(&f).await;
    let result = f
        .repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &request(
                &resource,
                Some(published),
                PublicationAction::Rollback,
                serde_json::json!({"experiment_id": "experiment-1"}),
            ),
        )
        .await;
    assert!(
        matches!(result, Err(ManagedError::Invalid(_))),
        "a rollback naming an experiment is malformed input, not a review: {result:?}"
    );
    assert_eq!(
        f.repository
            .list_publication_history(&f.owner, &resource)
            .await
            .expect("history")
            .len(),
        1
    );
}
