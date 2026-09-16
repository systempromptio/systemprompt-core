//! Automatic publication of the configured tree advances generations without
//! discarding history, and never publishes over withdrawals or open
//! reconciliations.

use super::*;
use systemprompt_marketplace::inventory::{LatestPublicationStatus, PublishGuard};
use systemprompt_marketplace::managed::ManagedResolution;

fn served_skill(bundle: &systemprompt_marketplace::managed::RevisionBundle) -> &[u8] {
    let digest = &bundle.revisions[&bundle.root].files["SKILL.md"].digest;
    &bundle.assets[digest]
}

impl Fixture {
    async fn publish_latest(
        &self,
        guard: &mut PublishGuard,
    ) -> Vec<systemprompt_marketplace::inventory::LatestPublication> {
        InventoryService::new(self.repository.clone())
            .publish_latest(
                &BaselineScope {
                    owner: &self.owner,
                    actor: &self.owner,
                    root: self.root.path(),
                    services: &ServicesConfig::default(),
                },
                guard,
            )
            .await
            .expect("publish latest")
    }

    async fn resolution(&self, key: &str) -> ManagedResolution {
        self.repository
            .resolve_managed(&self.owner, ResourceKind::Skill, key)
            .await
            .expect("resolution")
    }
}

#[tokio::test]
async fn fresh_configured_skill_is_adopted_and_served() {
    let f = Fixture::new().await;
    f.skill("local", true);
    f.refresh().await;
    let mut guard = PublishGuard::default();
    let outcome = f.publish_latest(&mut guard).await.remove(0);
    assert_eq!(outcome.status, LatestPublicationStatus::Published);
    assert_eq!(outcome.generation, Some(1));
    let revision = outcome.revision_id.expect("revision");
    let ManagedResolution::Published {
        resource_id,
        generation,
        revision_id,
        bundle_digest,
        ..
    } = f.resolution("local").await
    else {
        panic!("published resolution")
    };
    assert_eq!(generation, 1);
    assert_eq!(revision_id, revision);
    let bundle = f
        .repository
        .get_publication_bundle(&f.owner, &resource_id, 1, &bundle_digest)
        .await
        .expect("served bundle");
    assert_eq!(served_skill(&bundle), b"# original\n");
    let history = f
        .repository
        .list_publication_history(&f.owner, &resource_id)
        .await
        .expect("history");
    assert_eq!(history.len(), 1);
    assert_eq!(
        history[0].decision.action,
        PublicationAction::InitialAdoption
    );
    assert_eq!(
        history[0].comparison_evidence.recorded["source"],
        serde_json::json!("inventory_refresh")
    );
    let entry = f
        .repository
        .inventory_entry(&f.owner, &configured_identity(&f.owner, "skill", "local"))
        .await
        .expect("entry");
    assert_eq!(entry.published_revision_id, Some(revision));
}

#[tokio::test]
async fn unchanged_tree_publishes_nothing_on_repeat() {
    let f = Fixture::new().await;
    f.skill("local", true);
    f.refresh().await;
    let mut guard = PublishGuard::default();
    let first = f.publish_latest(&mut guard).await.remove(0);
    let repeated = f.publish_latest(&mut guard).await.remove(0);
    assert_eq!(repeated.status, LatestPublicationStatus::Unchanged);
    assert_eq!(repeated.revision_id, first.revision_id);
    let cold = f
        .publish_latest(&mut PublishGuard::default())
        .await
        .remove(0);
    assert_eq!(cold.status, LatestPublicationStatus::Unchanged);
    assert_eq!(cold.revision_id, first.revision_id);
    assert_eq!(cold.generation, Some(1));
    let ManagedResolution::Published {
        resource_id,
        generation: 1,
        ..
    } = f.resolution("local").await
    else {
        panic!("still generation 1")
    };
    assert_eq!(
        f.repository
            .list_publication_history(&f.owner, &resource_id)
            .await
            .expect("history")
            .len(),
        1
    );
    assert_eq!(
        f.repository
            .list_revisions(&f.owner, &resource_id, 0)
            .await
            .expect("revisions")
            .items
            .len(),
        1
    );
}

#[tokio::test]
async fn edited_skill_publishes_next_generation_and_retains_previous() {
    let f = Fixture::new().await;
    f.skill("local", true);
    f.refresh().await;
    let mut guard = PublishGuard::default();
    let first = f.publish_latest(&mut guard).await.remove(0);
    std::fs::write(f.root.path().join("skills/local/SKILL.md"), "# edited\n").expect("edit");
    f.refresh().await;
    let second = f.publish_latest(&mut guard).await.remove(0);
    assert_eq!(second.status, LatestPublicationStatus::Published);
    assert_eq!(second.generation, Some(2));
    assert_ne!(second.revision_id, first.revision_id);
    let ManagedResolution::Published {
        resource_id,
        generation: 2,
        revision_id,
        bundle_digest,
        ..
    } = f.resolution("local").await
    else {
        panic!("generation 2")
    };
    assert_eq!(Some(revision_id), second.revision_id);
    let served = f
        .repository
        .get_publication_bundle(&f.owner, &resource_id, 2, &bundle_digest)
        .await
        .expect("served bundle");
    assert_eq!(served_skill(&served), b"# edited\n");
    let history = f
        .repository
        .list_publication_history(&f.owner, &resource_id)
        .await
        .expect("history");
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].decision.generation, 2);
    assert_eq!(
        history[0].decision.action,
        PublicationAction::PublishImprovement
    );
    assert_eq!(
        history[0].comparison_evidence.recorded["previous_revision"],
        serde_json::json!(first.revision_id),
    );
    assert_eq!(history[1].decision.generation, 1);
    assert_eq!(history[1].decision.revision_id, first.revision_id);
    let retained = f
        .repository
        .get_revision_files(&f.owner, first.revision_id.as_ref().expect("first"))
        .await
        .expect("retained files");
    assert_eq!(retained.0["SKILL.md"].bytes, b"# original\n");
}

#[tokio::test]
async fn withdrawn_resource_stays_withheld() {
    let f = Fixture::new().await;
    f.skill("local", true);
    f.refresh().await;
    let mut guard = PublishGuard::default();
    let first = f.publish_latest(&mut guard).await.remove(0);
    let ManagedResolution::Published { resource_id, .. } = f.resolution("local").await else {
        panic!("published")
    };
    f.repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &PublicationRequest {
                resource_id: resource_id.clone(),
                revision_id: None,
                action: PublicationAction::Withdraw,
                expected_generation: 1,
                operation_key: "withdraw-local".to_owned(),
                comparison_evidence: systemprompt_marketplace::managed::ComparisonEvidence::default(
                ),
                limitations: String::new(),
            },
        )
        .await
        .expect("withdraw");
    f.refresh().await;
    std::fs::write(f.root.path().join("skills/local/SKILL.md"), "# edited\n").expect("edit");
    f.refresh().await;
    let outcome = f.publish_latest(&mut guard).await.remove(0);
    assert_eq!(outcome.status, LatestPublicationStatus::Withdrawn);
    assert!(matches!(
        f.resolution("local").await,
        ManagedResolution::Withdrawn { generation: 2, .. }
    ));
    let history = f
        .repository
        .list_publication_history(&f.owner, &resource_id)
        .await
        .expect("history");
    assert_eq!(history.len(), 2);
    assert_eq!(history[1].decision.revision_id, first.revision_id);
}

#[tokio::test]
async fn open_reconciliation_blocks_automatic_publication() {
    let f = Fixture::new().await;
    f.skill("local", true);
    f.refresh().await;
    let mut guard = PublishGuard::default();
    let first = f.publish_latest(&mut guard).await.remove(0);
    let baseline = first.revision_id.clone().expect("baseline");
    let manifest = f
        .repository
        .get_revision(&f.owner, &baseline)
        .await
        .expect("manifest");
    let resource = f
        .repository
        .revision_resource(&f.owner, &baseline)
        .await
        .expect("resource");
    let mut files = f
        .repository
        .get_revision_files(&f.owner, &baseline)
        .await
        .expect("files");
    files.0.get_mut("SKILL.md").expect("skill").bytes = b"# candidate\n".to_vec();
    f.repository
        .create_revision(
            &f.owner,
            &NewRevision {
                resource_id: resource.clone(),
                snapshot_id: manifest.snapshot_id,
                parent_id: Some(baseline.clone()),
                files,
                dependencies: manifest.dependencies,
                rationale: "unpublished optimization candidate".to_owned(),
            },
        )
        .await
        .expect("candidate");
    std::fs::write(
        f.root.path().join("skills/local/SKILL.md"),
        "# upstream update\n",
    )
    .expect("source edit");
    f.refresh().await;
    let outcome = f.publish_latest(&mut guard).await.remove(0);
    assert_eq!(
        outcome.status,
        LatestPublicationStatus::ReconciliationRequired
    );
    let again = f.publish_latest(&mut guard).await.remove(0);
    assert_eq!(
        again.status,
        LatestPublicationStatus::ReconciliationRequired
    );
    let ManagedResolution::Published {
        generation: 1,
        revision_id,
        ..
    } = f.resolution("local").await
    else {
        panic!("generation 1 retained")
    };
    assert_eq!(revision_id, baseline);
    assert_eq!(
        f.repository
            .list_publication_history(&f.owner, &resource)
            .await
            .expect("history")
            .len(),
        1
    );
}
