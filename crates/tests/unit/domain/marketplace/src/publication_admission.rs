//! Inventory-sync admission is a separate seam: the reviewed HTTP path still
//! demands an attested experiment, and the sync path only moves forward.

use super::*;
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
    let manifest = f
        .repository
        .get_revision(&f.owner, &revision_id)
        .await
        .expect("manifest");
    let mut files = f
        .repository
        .get_revision_files(&f.owner, &revision_id)
        .await
        .expect("files");
    files.0.get_mut("SKILL.md").expect("skill").bytes = b"# edited\n".to_vec();
    f.repository
        .create_revision(
            &f.owner,
            &NewRevision {
                resource_id,
                snapshot_id: manifest.snapshot_id,
                parent_id: Some(revision_id),
                files,
                dependencies: manifest.dependencies,
                rationale: "edited candidate".to_owned(),
            },
        )
        .await
        .expect("edited revision")
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
async fn publication_operation_key_replays_same_decision_and_rejects_changed_request() {
    let f = Fixture::new().await;
    let (resource, _) = adopted(&f).await;
    let revision = edited_revision(&f).await;
    let evidence = serde_json::json!({"source": "inventory_refresh"});
    let request = request(
        &resource,
        Some(revision.clone()),
        PublicationAction::PublishImprovement,
        evidence,
    );
    let first = f
        .repository
        .publish_with_admission(
            &f.owner,
            &f.owner,
            &request,
            PublicationAdmission::InventorySync,
        )
        .await
        .expect("first publication");
    let replay = f
        .repository
        .publish_with_admission(
            &f.owner,
            &f.owner,
            &request,
            PublicationAdmission::InventorySync,
        )
        .await
        .expect("same operation is idempotent");
    assert_eq!(replay, first);
    assert_eq!(
        f.repository
            .list_publication_history(&f.owner, &resource)
            .await
            .expect("publication history")
            .len(),
        2
    );
    let published_before_conflict = f
        .repository
        .resolve_managed(&f.owner, ResourceKind::Skill, "local")
        .await
        .expect("published resolution");

    let mut changed = request.clone();
    changed.limitations = "reviewed after replay".into();
    assert!(matches!(
        f.repository
            .publish_with_admission(
                &f.owner,
                &f.owner,
                &changed,
                PublicationAdmission::InventorySync,
            )
            .await,
        Err(ManagedError::Conflict(_))
    ));
    assert_eq!(
        f.repository
            .resolve_managed(&f.owner, ResourceKind::Skill, "local")
            .await
            .expect("published resolution after rejected replay"),
        published_before_conflict
    );
    assert_eq!(
        f.repository
            .list_publication_history(&f.owner, &resource)
            .await
            .expect("publication history after rejected replay")
            .len(),
        2
    );
}

#[tokio::test]
async fn publication_operation_keys_are_scoped_to_the_resource_owner() {
    let first_owner = Fixture::new().await;
    let (first_resource, _) = adopted(&first_owner).await;
    let first_revision = edited_revision(&first_owner).await;
    let first_request = request(
        &first_resource,
        Some(first_revision),
        PublicationAction::PublishImprovement,
        serde_json::json!({"source": "inventory_refresh"}),
    );
    let first_decision = first_owner
        .repository
        .publish_with_admission(
            &first_owner.owner,
            &first_owner.owner,
            &first_request,
            PublicationAdmission::InventorySync,
        )
        .await
        .expect("first owner's publication");

    let second_owner = Fixture::new().await;
    let (second_resource, _) = adopted(&second_owner).await;
    let second_revision = edited_revision(&second_owner).await;
    let mut second_request = request(
        &second_resource,
        Some(second_revision),
        PublicationAction::PublishImprovement,
        serde_json::json!({"source": "inventory_refresh"}),
    );
    second_request.operation_key = first_request.operation_key.clone();
    let second_decision = second_owner
        .repository
        .publish_with_admission(
            &second_owner.owner,
            &second_owner.owner,
            &second_request,
            PublicationAdmission::InventorySync,
        )
        .await
        .expect("same key is valid for an independent owner");
    assert_ne!(
        second_decision.publication_id,
        first_decision.publication_id
    );
    assert_eq!(
        first_owner
            .repository
            .list_publication_history(&first_owner.owner, &first_resource)
            .await
            .expect("first owner's history")
            .len(),
        2
    );
    assert_eq!(
        second_owner
            .repository
            .list_publication_history(&second_owner.owner, &second_resource)
            .await
            .expect("second owner's history")
            .len(),
        2
    );
}
