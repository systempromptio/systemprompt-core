use std::collections::BTreeMap;

use systemprompt_identifiers::UserId;
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, DependencyRef, ManagedError, ManagedRepository, NewResource,
    NewRevision, ResourceKind, RevisionFiles, SnapshotProvenance, SourceSpec,
};
use systemprompt_test_fixtures::{DisposableDb, seed_user_row};
use uuid::Uuid;

fn files(bytes: &[u8]) -> RevisionFiles {
    RevisionFiles(BTreeMap::from([(
        "SKILL.md".to_owned(),
        AssetFile {
            bytes: bytes.to_vec(),
            media_type: "text/markdown".to_owned(),
            executable: false,
        },
    )]))
}

#[tokio::test]
async fn source_registration_and_resource_binding_reject_conflicting_identity() {
    let database = DisposableDb::installed("managed_source_identity")
        .await
        .expect("private database");
    let db = database.pool().await.expect("private pool");
    let owner = UserId::new(Uuid::new_v4().to_string());
    seed_user_row(&db, &owner, &format!("{owner}@managed.invalid"))
        .await
        .expect("owner");
    let repository = ManagedRepository::new(&db).expect("repository");

    let source = repository
        .register_source(&owner, "catalog", &SourceSpec::Managed)
        .await
        .expect("source");
    assert_eq!(
        repository
            .register_source(&owner, "catalog", &SourceSpec::Managed)
            .await
            .expect("idempotent source"),
        source
    );
    let changed = SourceSpec::LocalTree {
        root: "/owned/catalog".to_owned(),
    };
    assert!(matches!(
        repository.register_source(&owner, "catalog", &changed).await,
        Err(ManagedError::Conflict(message)) if message.contains("different specification")
    ));
    assert!(matches!(
        repository
            .get_source(&owner, &source)
            .await
            .expect("source"),
        SourceSpec::Managed
    ));

    let input = NewResource {
        source_id: source,
        upstream_key: "alpha".to_owned(),
        kind: ResourceKind::Skill,
        resource_key: "alpha".to_owned(),
    };
    let resource = repository
        .bind_resource(&owner, &input)
        .await
        .expect("binding");
    assert_eq!(
        repository
            .bind_resource(&owner, &input)
            .await
            .expect("idempotent binding"),
        resource
    );
    let changed = NewResource {
        resource_key: "beta".to_owned(),
        ..input
    };
    assert!(matches!(
        repository.bind_resource(&owner, &changed).await,
        Err(ManagedError::Conflict(message)) if message.contains("different binding")
    ));

    drop(repository);
    drop(db);
    database.drop_now().await;
}

#[tokio::test]
async fn revision_creation_rejects_invalid_lineage_and_dependency_digest() {
    let database = DisposableDb::installed("managed_revision_integrity")
        .await
        .expect("private database");
    let db = database.pool().await.expect("private pool");
    let owner = UserId::new(Uuid::new_v4().to_string());
    seed_user_row(&db, &owner, &format!("{owner}@managed.invalid"))
        .await
        .expect("owner");
    let repository = ManagedRepository::new(&db).expect("repository");
    let source = repository
        .register_source(&owner, "authoring", &SourceSpec::Managed)
        .await
        .expect("source");
    let snapshot = repository
        .capture_snapshot(
            &owner,
            &source,
            &SnapshotProvenance {
                source_kind: "managed".to_owned(),
                commit: None,
                tree_digest: AssetDigest::of(b"tree"),
                importer_version: "test".to_owned(),
            },
        )
        .await
        .expect("snapshot");
    let resource = repository
        .bind_resource(
            &owner,
            &NewResource {
                source_id: source.clone(),
                upstream_key: "alpha".to_owned(),
                kind: ResourceKind::Skill,
                resource_key: "alpha".to_owned(),
            },
        )
        .await
        .expect("resource");
    assert!(matches!(
        repository
            .create_revision(
                &owner,
                &NewRevision {
                    resource_id: resource.clone(),
                    snapshot_id: snapshot.clone(),
                    parent_id: None,
                    files: files(b"# alpha\n"),
                    dependencies: BTreeMap::new(),
                    rationale: "  ".to_owned(),
                },
            )
            .await,
        Err(ManagedError::Invalid(_))
    ));
    let revision = repository
        .create_revision(
            &owner,
            &NewRevision {
                resource_id: resource.clone(),
                snapshot_id: snapshot.clone(),
                parent_id: None,
                files: files(b"# alpha\n"),
                dependencies: BTreeMap::new(),
                rationale: "initial revision".to_owned(),
            },
        )
        .await
        .expect("revision");

    let other_source = repository
        .register_source(&owner, "other-authoring", &SourceSpec::Managed)
        .await
        .expect("other source");
    let other_snapshot = repository
        .capture_snapshot(
            &owner,
            &other_source,
            &SnapshotProvenance {
                source_kind: "managed".to_owned(),
                commit: None,
                tree_digest: AssetDigest::of(b"other-tree"),
                importer_version: "test".to_owned(),
            },
        )
        .await
        .expect("other snapshot");
    assert!(matches!(
        repository
            .create_revision(
                &owner,
                &NewRevision {
                    resource_id: resource.clone(),
                    snapshot_id: other_snapshot,
                    parent_id: None,
                    files: files(b"# wrong source\n"),
                    dependencies: BTreeMap::new(),
                    rationale: "wrong source snapshot".to_owned(),
                },
            )
            .await,
        Err(ManagedError::Unavailable)
    ));
    let other_resource = repository
        .bind_resource(
            &owner,
            &NewResource {
                source_id: source,
                upstream_key: "other".to_owned(),
                kind: ResourceKind::Skill,
                resource_key: "other".to_owned(),
            },
        )
        .await
        .expect("other resource");
    assert!(matches!(
        repository
            .create_revision(
                &owner,
                &NewRevision {
                    resource_id: other_resource,
                    snapshot_id: snapshot.clone(),
                    parent_id: Some(revision.clone()),
                    files: files(b"# wrong parent\n"),
                    dependencies: BTreeMap::new(),
                    rationale: "wrong resource parent".to_owned(),
                },
            )
            .await,
        Err(ManagedError::Unavailable)
    ));
    assert!(matches!(
        repository
            .create_revision(
                &owner,
                &NewRevision {
                    resource_id: resource.clone(),
                    snapshot_id: snapshot,
                    parent_id: None,
                    files: files(b"# forged dependency\n"),
                    dependencies: BTreeMap::from([(
                        "dependency".to_owned(),
                        DependencyRef {
                            revision_id: revision.clone(),
                            digest: AssetDigest::of(b"forged dependency digest"),
                        },
                    )]),
                    rationale: "forged dependency".to_owned(),
                },
            )
            .await,
        Err(ManagedError::Integrity)
    ));
    drop(repository);
    drop(db);
    database.drop_now().await;
}
