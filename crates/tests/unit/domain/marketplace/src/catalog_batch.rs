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
