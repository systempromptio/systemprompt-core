use std::collections::BTreeMap;

use systemprompt_identifiers::UserId;
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, ComparisonEvidence, ManagedError, ManagedRepository, ManagedResolution,
    NewResource, NewRevision, PublicationAction, PublicationRequest, ResourceKind, RevisionFiles,
    SnapshotProvenance, SourceSpec,
};
use systemprompt_test_fixtures::{DisposableDb, seed_user_row};

#[tokio::test]
async fn final_outbox_write_failure_rolls_back_publication_then_identical_retry_commits() {
    let db = DisposableDb::installed("publication_outbox_rollback")
        .await
        .expect("isolated database");
    let pool = db.pool().await.expect("database pool");
    let raw = pool.write_pool_arc().expect("write pool");
    let owner = UserId::new(format!("publication-tx-{}", uuid::Uuid::new_v4()));
    seed_user_row(&pool, &owner, &format!("{owner}@publication-tx.invalid"))
        .await
        .expect("owner");
    let repository = ManagedRepository::new(&pool).expect("managed repository");
    let source = repository
        .register_source(&owner, "publication-transaction", &SourceSpec::Managed)
        .await
        .expect("source");
    let snapshot = repository
        .capture_snapshot(
            &owner,
            &source,
            &SnapshotProvenance {
                source_kind: "managed".to_owned(),
                commit: None,
                tree_digest: AssetDigest::of(b"publication transaction tree"),
                importer_version: "transaction-fixture".to_owned(),
            },
        )
        .await
        .expect("snapshot");
    let resource = repository
        .bind_resource(
            &owner,
            &NewResource {
                source_id: source,
                upstream_key: "atomic-skill".to_owned(),
                kind: ResourceKind::Skill,
                resource_key: "atomic-skill".to_owned(),
            },
        )
        .await
        .expect("resource");
    let revision = repository
        .create_revision(
            &owner,
            &NewRevision {
                resource_id: resource.clone(),
                snapshot_id: snapshot,
                parent_id: None,
                files: RevisionFiles(BTreeMap::from([
                    (
                        "config.yaml".to_owned(),
                        AssetFile {
                            bytes: b"id: atomic-skill\nenabled: true\n".to_vec(),
                            media_type: "application/yaml".to_owned(),
                            executable: false,
                        },
                    ),
                    (
                        "SKILL.md".to_owned(),
                        AssetFile {
                            bytes: b"# Atomic publication\n".to_vec(),
                            media_type: "text/markdown".to_owned(),
                            executable: false,
                        },
                    ),
                ])),
                dependencies: BTreeMap::new(),
                rationale: "transaction boundary fixture".to_owned(),
            },
        )
        .await
        .expect("revision");
    let request = PublicationRequest {
        resource_id: resource.clone(),
        revision_id: Some(revision.clone()),
        action: PublicationAction::InitialAdoption,
        expected_generation: 0,
        operation_key: format!("publication-tx-{}", uuid::Uuid::new_v4()),
        comparison_evidence: ComparisonEvidence::default(),
        limitations: String::new(),
    };

    sqlx::query(
        "CREATE FUNCTION reject_publication_outbox() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN RAISE EXCEPTION 'fixture rejects final outbox write'; END $$",
    )
    .execute(raw.as_ref())
    .await
    .expect("failure function");
    sqlx::query(
        "CREATE TRIGGER reject_publication_outbox BEFORE INSERT ON managed_distribution_outbox \
         FOR EACH ROW EXECUTE FUNCTION reject_publication_outbox()",
    )
    .execute(raw.as_ref())
    .await
    .expect("failure trigger");

    let error = repository
        .review_and_publish(&owner, &owner, &request)
        .await
        .expect_err("final outbox failure must abort publication");
    assert!(matches!(error, ManagedError::Database(_)));
    assert!(matches!(
        repository
            .resolve_managed(&owner, ResourceKind::Skill, "atomic-skill")
            .await
            .expect("resolution after rollback"),
        ManagedResolution::NeverAdopted { resource_id } if resource_id == resource
    ));
    for (table, query) in [
        (
            "managed_publication_reviews",
            "SELECT count(*) FROM managed_publication_reviews WHERE owner_id = $1",
        ),
        (
            "managed_publications",
            "SELECT count(*) FROM managed_publications WHERE owner_id = $1",
        ),
        (
            "managed_publication_selections",
            "SELECT count(*) FROM managed_publication_selections WHERE owner_id = $1",
        ),
        (
            "managed_distribution_outbox",
            "SELECT count(*) FROM managed_distribution_outbox WHERE owner_id = $1",
        ),
    ] {
        let count = sqlx::query_scalar::<_, i64>(query)
            .bind(owner.as_str())
            .fetch_one(raw.as_ref())
            .await
            .expect("count rolled-back publication rows");
        assert_eq!(
            count, 0,
            "{table} must roll back with the failed outbox write"
        );
    }

    sqlx::query("DROP TRIGGER reject_publication_outbox ON managed_distribution_outbox")
        .execute(raw.as_ref())
        .await
        .expect("remove failure trigger");
    sqlx::query("DROP FUNCTION reject_publication_outbox()")
        .execute(raw.as_ref())
        .await
        .expect("remove failure function");

    let decision = repository
        .review_and_publish(&owner, &owner, &request)
        .await
        .expect("identical retry commits after storage recovery");
    assert_eq!(decision.generation, 1);
    assert_eq!(decision.revision_id.as_ref(), Some(&revision));
    assert!(matches!(
        repository
            .resolve_managed(&owner, ResourceKind::Skill, "atomic-skill")
            .await
            .expect("published resolution"),
        ManagedResolution::Published {
            publication_id,
            revision_id,
            generation: 1,
            ..
        } if publication_id == decision.publication_id && revision_id == revision
    ));
    let outbox: (String, i64, serde_json::Value) = sqlx::query_as(
        "SELECT publication_id, generation, payload FROM managed_distribution_outbox \
         WHERE owner_id = $1",
    )
    .bind(owner.as_str())
    .fetch_one(raw.as_ref())
    .await
    .expect("committed outbox event");
    assert_eq!(outbox.0, decision.publication_id.as_str());
    assert_eq!(outbox.1, 1);
    assert_eq!(
        outbox
            .2
            .get("revision_id")
            .and_then(serde_json::Value::as_str),
        Some(revision.as_str())
    );

    drop(repository);
    drop(raw);
    drop(pool);
    db.drop_now().await;
}
