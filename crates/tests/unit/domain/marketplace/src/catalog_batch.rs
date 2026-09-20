//! The set-based overlay resolves every managed skill of both principals in
//! a handful of queries and lands on the same catalogue the per-key resolver
//! describes: published overlaid, withdrawn and never-adopted withheld,
//! revoked withheld for that consumer only.

use std::collections::BTreeMap;

use systemprompt_identifiers::{ManagedResourceId, ResourceRevisionId};
use systemprompt_marketplace::managed::{
    AssetDigest, ManagedResolution, NewResource, NewRevision, PublicationAction,
    PublicationRequest, ResourceKind, SnapshotProvenance, SourceSpec,
};

use crate::managed_resolution::{
    Fixture, disk_catalog_with, fixture, publish, skill_files, withdraw,
};

async fn bind_second_skill(f: &Fixture, key: &str) -> (ManagedResourceId, ResourceRevisionId) {
    let source = f
        .repository
        .register_source(&f.owner, &format!("authoring-{key}"), &SourceSpec::Managed)
        .await
        .expect("second source");
    let snapshot = f
        .repository
        .capture_snapshot(
            &f.owner,
            &source,
            &SnapshotProvenance {
                source_kind: "managed".to_owned(),
                commit: None,
                tree_digest: AssetDigest::of(key.as_bytes()),
                importer_version: "test".to_owned(),
            },
        )
        .await
        .expect("second snapshot");
    let resource = f
        .repository
        .bind_resource(
            &f.owner,
            &NewResource {
                source_id: source,
                upstream_key: key.to_owned(),
                kind: ResourceKind::Skill,
                resource_key: key.to_owned(),
            },
        )
        .await
        .expect("second resource");
    let revision = f
        .repository
        .create_revision(
            &f.owner,
            &NewRevision {
                resource_id: resource.clone(),
                snapshot_id: snapshot,
                parent_id: None,
                files: skill_files(key, "# second managed instructions\n"),
                dependencies: BTreeMap::new(),
                rationale: "first revision".to_owned(),
            },
        )
        .await
        .expect("second revision");
    (resource, revision)
}

#[tokio::test]
async fn batched_resolution_matches_the_per_key_states_and_bundles() {
    let Some(f) = fixture().await else {
        return;
    };
    publish(&f).await;
    let withdrawn_key = format!("{}_withdrawn", f.key);
    let (withdrawn_resource, withdrawn_revision) = bind_second_skill(&f, &withdrawn_key).await;
    f.repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &PublicationRequest {
                resource_id: withdrawn_resource.clone(),
                revision_id: Some(withdrawn_revision.clone()),
                action: PublicationAction::InitialAdoption,
                expected_generation: 0,
                operation_key: format!("adopt-{withdrawn_key}"),
                comparison_evidence: Default::default(),
                limitations: String::new(),
            },
        )
        .await
        .expect("adopt second");
    f.repository
        .review_and_publish(
            &f.owner,
            &f.owner,
            &PublicationRequest {
                resource_id: withdrawn_resource.clone(),
                revision_id: None,
                action: PublicationAction::Withdraw,
                expected_generation: 1,
                operation_key: format!("withdraw-{withdrawn_key}"),
                comparison_evidence: Default::default(),
                limitations: String::new(),
            },
        )
        .await
        .expect("withdraw second");
    let never_key = format!("{}_never", f.key);
    let (never_resource, _) = bind_second_skill(&f, &never_key).await;

    let rows = f
        .repository
        .list_skill_resolutions(&f.owner)
        .await
        .expect("batched states");
    let by_key: BTreeMap<&str, &ManagedResolution> = rows
        .iter()
        .map(|row| (row.resource_key.as_str(), &row.resolution))
        .collect();
    assert_eq!(by_key.len(), 3);
    let published = f
        .repository
        .resolve_managed(&f.owner, ResourceKind::Skill, &f.key)
        .await
        .expect("per-key published");
    assert_eq!(by_key[f.key.as_str()], &published);
    assert!(matches!(
        by_key[withdrawn_key.as_str()],
        ManagedResolution::Withdrawn { resource_id, generation: 2, .. } if resource_id == &withdrawn_resource
    ));
    assert!(matches!(
        by_key[never_key.as_str()],
        ManagedResolution::NeverAdopted { resource_id } if resource_id == &never_resource
    ));

    let ManagedResolution::Published { bundle_digest, .. } = &published else {
        panic!("published");
    };
    let bundles = f
        .repository
        .get_revision_bundles(&[
            (f.owner.clone(), f.revision.clone()),
            (f.owner.clone(), withdrawn_revision.clone()),
        ])
        .await
        .expect("batched bundles");
    assert_eq!(bundles.len(), 2);
    assert_eq!(
        &bundles[&f.revision].digest().expect("digest"),
        bundle_digest
    );
    let single = f
        .repository
        .get_revision_bundle(&f.owner, &withdrawn_revision)
        .await
        .expect("per-key bundle");
    assert_eq!(
        bundles[&withdrawn_revision].digest().expect("digest"),
        single.digest().expect("digest")
    );
}

#[tokio::test]
async fn the_overlay_withholds_revoked_keys_for_that_consumer_only() {
    let Some(f) = fixture().await else {
        return;
    };
    let revoked = fixture().await.expect("revoked consumer");
    let granted = fixture().await.expect("granted consumer");
    publish(&f).await;
    let stamp_before = f
        .repository
        .managed_catalog_stamp(&f.owner, &revoked.owner)
        .await
        .expect("stamp");
    f.repository
        .set_consumer_grant(&f.owner, &f.resource, &revoked.owner, false)
        .await
        .expect("revoke");
    let stamp_after = f
        .repository
        .managed_catalog_stamp(&f.owner, &revoked.owner)
        .await
        .expect("stamp");
    assert_ne!(stamp_before, stamp_after, "a revocation moves the stamp");
    assert_eq!(
        f.repository
            .revoked_skill_keys(&f.owner, &revoked.owner)
            .await
            .expect("revoked keys")
            .into_iter()
            .collect::<Vec<_>>(),
        vec![f.key.clone()]
    );

    let (_dir, disk) = disk_catalog_with(&f.key);
    let denied = disk
        .clone()
        .with_organization_skills(f.repository.clone(), &f.owner, &revoked.owner)
        .await
        .expect("denied catalogue");
    assert!(denied.as_content().skills.is_empty());
    let served = disk
        .clone()
        .with_organization_skills(f.repository.clone(), &f.owner, &granted.owner)
        .await
        .expect("served catalogue");
    assert_eq!(served.as_content().skills.len(), 1);
    assert!(
        served.as_content().skills[0]
            .instructions
            .contains("managed instructions")
    );

    withdraw(&f).await;
    let stamp_withdrawn = f
        .repository
        .managed_catalog_stamp(&f.owner, &granted.owner)
        .await
        .expect("stamp");
    assert_ne!(stamp_after, stamp_withdrawn, "a withdrawal moves the stamp");
    let gone = disk
        .with_organization_skills(f.repository.clone(), &f.owner, &granted.owner)
        .await
        .expect("withdrawn catalogue");
    assert!(gone.as_content().skills.is_empty());
}
#[tokio::test]
async fn rejects_mismatched_selection_until_exact_publication_digest_is_restored() {
    use systemprompt_identifiers::UserId;
    use systemprompt_marketplace::CatalogContent;
    use systemprompt_marketplace::managed::ManagedRepository;
    use systemprompt_models::services::ServicesConfig;
    use systemprompt_test_fixtures::{DisposableDb, seed_user_row};
    use uuid::Uuid;

    let database = DisposableDb::installed("managed_catalog_integrity_recovery")
        .await
        .expect("isolated managed catalog database");
    let pool = database.pool().await.expect("managed catalog pool");
    let owner = UserId::new(format!("catalog-owner-{}", Uuid::new_v4().simple()));
    seed_user_row(
        &pool,
        &owner,
        &format!("{}@catalog.invalid", owner.as_str()),
    )
    .await
    .expect("seed catalog owner");
    let repository = ManagedRepository::new(&pool).expect("managed repository");
    let source = repository
        .register_source(&owner, "catalog-authoring", &SourceSpec::Managed)
        .await
        .expect("register managed source");
    let snapshot = repository
        .capture_snapshot(
            &owner,
            &source,
            &SnapshotProvenance {
                source_kind: "managed".to_owned(),
                commit: None,
                tree_digest: AssetDigest::of(b"catalog-integrity-tree"),
                importer_version: "test".to_owned(),
            },
        )
        .await
        .expect("capture managed snapshot");

    let mut published = Vec::new();
    for (label, body) in [
        ("corrupt", "# selected publication\n"),
        ("healthy", "# healthy publication\n"),
    ] {
        let key = format!("catalog_{label}_{}", Uuid::new_v4().simple());
        let resource = repository
            .bind_resource(
                &owner,
                &NewResource {
                    source_id: source.clone(),
                    upstream_key: key.clone(),
                    kind: ResourceKind::Skill,
                    resource_key: key.clone(),
                },
            )
            .await
            .expect("bind managed skill");
        let revision = repository
            .create_revision(
                &owner,
                &NewRevision {
                    resource_id: resource.clone(),
                    snapshot_id: snapshot.clone(),
                    parent_id: None,
                    files: skill_files(&key, body),
                    dependencies: BTreeMap::new(),
                    rationale: "catalog integrity fixture".to_owned(),
                },
            )
            .await
            .expect("create managed revision");
        repository
            .review_and_publish(
                &owner,
                &owner,
                &PublicationRequest {
                    resource_id: resource.clone(),
                    revision_id: Some(revision),
                    action: PublicationAction::InitialAdoption,
                    expected_generation: 0,
                    operation_key: format!("publish-{key}"),
                    comparison_evidence: Default::default(),
                    limitations: String::new(),
                },
            )
            .await
            .expect("publish managed skill");
        published.push((key, resource));
    }

    let baseline = repository
        .list_skill_resolutions(&owner)
        .await
        .expect("baseline catalog resolutions");
    let digest_for = |key: &str| {
        baseline
            .iter()
            .find(|row| row.resource_key == key)
            .and_then(|row| match &row.resolution {
                ManagedResolution::Published { bundle_digest, .. } => {
                    Some(bundle_digest.as_str().to_owned())
                },
                _ => None,
            })
            .expect("published bundle digest")
    };
    let corrupt_digest = digest_for(&published[0].0);
    let healthy_digest = digest_for(&published[1].0);
    assert_ne!(corrupt_digest, healthy_digest);
    let raw = pool.write_pool_arc().expect("managed write pool");
    sqlx::query(
        "UPDATE managed_publication_selections SET bundle_digest=$1 \
         WHERE owner_id=$2 AND resource_id=$3",
    )
    .bind(&healthy_digest)
    .bind(owner.as_str())
    .bind(published[0].1.as_str())
    .execute(raw.as_ref())
    .await
    .expect("diverge selected digest from publication");

    let divergent = repository
        .list_skill_resolutions(&owner)
        .await
        .expect("divergent catalog resolutions");
    assert!(divergent.iter().any(|row| {
        row.resource_key == published[0].0
            && matches!(
                &row.resolution,
                ManagedResolution::IntegrityFailure {
                    resource_id,
                    generation: 1,
                } if resource_id == &published[0].1
            )
    }));
    assert!(divergent.iter().any(|row| {
        row.resource_key == published[1].0
            && matches!(row.resolution, ManagedResolution::Published { .. })
    }));

    let services = tempfile::tempdir().expect("services root");
    for (key, _) in &published {
        crate::helpers::write_skill_on_disk(services.path(), key);
    }
    let disk = CatalogContent::load(
        &ServicesConfig::default(),
        services.path(),
        "https://api.example.invalid",
    )
    .expect("filesystem catalog");
    let error = disk
        .clone()
        .with_managed_skills(repository.clone(), &owner)
        .await
        .expect_err("catalog must refuse a selection not backed by its publication");
    assert!(matches!(
        error,
        systemprompt_marketplace::MarketplaceError::Managed(
            systemprompt_marketplace::managed::ManagedError::Integrity
        )
    ));

    sqlx::query(
        "UPDATE managed_publication_selections SET bundle_digest=$1 \
         WHERE owner_id=$2 AND resource_id=$3",
    )
    .bind(&corrupt_digest)
    .bind(owner.as_str())
    .bind(published[0].1.as_str())
    .execute(raw.as_ref())
    .await
    .expect("restore selected publication digest");
    let recovered = disk
        .with_managed_skills(repository.clone(), &owner)
        .await
        .expect("matching selection restores catalog");
    assert_eq!(recovered.as_content().skills.len(), 2);
    assert!(recovered.as_content().skills.iter().any(|skill| {
        skill.id.as_str() == published[0].0 && skill.instructions.contains("selected publication")
    }));
    assert!(recovered.as_content().skills.iter().any(|skill| {
        skill.id.as_str() == published[1].0 && skill.instructions.contains("healthy publication")
    }));

    drop(repository);
    drop(raw);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}
