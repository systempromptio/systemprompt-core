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
    let results = f.baselines().await;
    assert_eq!(
        results
            .iter()
            .filter(|entry| entry.status == "ready")
            .count(),
        2
    );
    assert_eq!(
        results
            .iter()
            .filter(|entry| entry.status == "blocked")
            .count(),
        1
    );
}

#[tokio::test]
async fn naming_conflicts_require_explicit_binding_and_history_before_observation_is_unknown() {
    let f = Fixture::new().await;
    f.skill("same", true);
    let (resource, _) = f.imported("same").await;
    let before = chrono::Utc::now();
    f.refresh().await;
    let id = configured_identity(&f.owner, "skill", "same");
    assert!(matches!(
        f.repository
            .inventory_membership(&f.owner, &id, before)
            .await
            .expect("membership"),
        ObservedMembership::Unknown
    ));
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
    let coverage = f
        .repository
        .inventory_coverage_at(&f.owner, chrono::Utc::now())
        .await
        .expect("coverage");
    assert_eq!(coverage.known_available, 1);
}

#[tokio::test]
async fn upstream_removal_retains_publication_and_records_effective_membership() {
    let f = Fixture::new().await;
    f.skill("local", true);
    f.refresh().await;
    let capture = f.baselines().await.remove(0);
    let id = configured_identity(&f.owner, "skill", "local");
    let entry = f
        .repository
        .inventory_entry(&f.owner, &id)
        .await
        .expect("entry");
    f.repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &PublicationRequest {
                resource_id: entry.resource_id.expect("resource"),
                revision_id: capture.revision_id.clone(),
                action: PublicationAction::InitialAdoption,
                expected_generation: 0,
                operation_key: "adopt-local".to_owned(),
                comparison_evidence: systemprompt_marketplace::managed::ComparisonEvidence::default(
                ),
                limitations: String::new(),
            },
        )
        .await
        .expect("approve");
    f.refresh().await;
    let prior = chrono::Utc::now();
    std::fs::remove_dir_all(f.root.path().join("skills/local")).expect("remove source");
    f.refresh().await;
    let entry = f
        .repository
        .inventory_entry(&f.owner, &id)
        .await
        .expect("withdrawn inventory");
    assert_eq!(entry.availability, InventoryAvailability::Withdrawn);
    assert_eq!(entry.published_revision_id, capture.revision_id);
    let ObservedMembership::Known { entry, .. } = f
        .repository
        .inventory_membership(&f.owner, &id, prior)
        .await
        .expect("prior membership")
    else {
        panic!("known membership")
    };
    assert_eq!(entry.availability, InventoryAvailability::Available);
}

#[tokio::test]
async fn changed_authoring_reuses_three_way_reconciliation_without_replacing_candidates() {
    let f = Fixture::new().await;
    f.skill("local", true);
    f.refresh().await;
    let baseline = f.baselines().await.remove(0).revision_id.expect("baseline");
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
    let result = f.baselines().await.remove(0);
    assert_eq!(result.status, "reconciliation_required");
    assert!(result.reconciliation_id.is_some());
    assert_ne!(result.revision_id.as_ref(), Some(&candidate));
    assert!(
        f.repository
            .get_revision(&f.owner, &candidate)
            .await
            .is_ok()
    );
}
