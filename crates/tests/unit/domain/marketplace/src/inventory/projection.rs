use super::*;

#[tokio::test]
async fn configured_unused_disabled_and_imported_unpublished_entries_are_included() {
    let f = Fixture::new().await;
    f.skill("unused", true);
    f.skill("disabled", false);
    let (_, revision) = f.imported("remote").await;
    f.refresh().await;
    let entries = f
        .repository
        .inventory(&f.owner, None, 100)
        .await
        .expect("inventory");
    assert_eq!(entries.len(), 3);
    assert!(entries.iter().any(|entry| entry.resource_key == "disabled"
        && entry.availability == InventoryAvailability::Unavailable));
    assert!(
        entries
            .iter()
            .any(|entry| entry.origin == InventoryOrigin::Imported
                && entry.latest_revision_id.as_ref() == Some(&revision)
                && entry.published_revision_id.is_none())
    );
    let results = f.publish().await;
    assert_eq!(results.len(), 2, "only configured skills are published");
    assert!(
        results
            .iter()
            .any(|outcome| outcome.resource_key == "unused"
                && outcome.status == LatestPublicationStatus::Published)
    );
    assert!(
        results
            .iter()
            .any(|outcome| outcome.resource_key == "disabled"
                && outcome.status == LatestPublicationStatus::Blocked)
    );
}

#[tokio::test]
async fn naming_conflicts_require_explicit_binding() {
    let f = Fixture::new().await;
    f.skill("same", true);
    let (resource, _) = f.imported("same").await;
    f.refresh().await;
    let id = configured_identity(&f.owner, "skill", "same");
    let entries = f
        .repository
        .inventory(&f.owner, None, 100)
        .await
        .expect("inventory");
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.availability == InventoryAvailability::Conflicting)
            .count(),
        2
    );
    f.repository
        .bind_inventory_resource(&f.owner, &f.owner, &id, &resource)
        .await
        .expect("explicit binding");
    f.refresh().await;
    let entry = f
        .repository
        .inventory_entry(&f.owner, &id)
        .await
        .expect("bound entry");
    assert_eq!(entry.resource_id, Some(resource));
    assert_eq!(entry.availability, InventoryAvailability::Available);
}

#[tokio::test]
async fn upstream_removal_retains_publication() {
    let f = Fixture::new().await;
    f.skill("local", true);
    f.refresh().await;
    let published = f.publish().await.remove(0);
    assert_eq!(published.status, LatestPublicationStatus::Published);
    let id = configured_identity(&f.owner, "skill", "local");
    f.refresh().await;
    std::fs::remove_dir_all(f.root.path().join("skills/local")).expect("remove source");
    f.refresh().await;
    let entry = f
        .repository
        .inventory_entry(&f.owner, &id)
        .await
        .expect("withdrawn inventory");
    assert_eq!(entry.availability, InventoryAvailability::Withdrawn);
    assert_eq!(entry.published_revision_id, published.revision_id);
}

#[tokio::test]
async fn changed_authoring_reuses_three_way_reconciliation_without_replacing_candidates() {
    let f = Fixture::new().await;
    f.skill("local", true);
    f.refresh().await;
    let baseline = f.publish().await.remove(0).revision_id.expect("baseline");
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
    let candidate = f
        .repository
        .create_revision(
            &f.owner,
            &NewRevision {
                resource_id: resource,
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
    let result = f.publish().await.remove(0);
    assert_eq!(
        result.status,
        LatestPublicationStatus::ReconciliationRequired
    );
    assert!(
        f.repository
            .get_revision(&f.owner, &candidate)
            .await
            .is_ok()
    );
}
