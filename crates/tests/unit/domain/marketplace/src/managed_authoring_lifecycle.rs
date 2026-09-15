//! Candidate authoring must preserve reviewed content and retained review
//! history.

use crate::managed_resolution::{fixture, publish, withdraw};
use systemprompt_identifiers::UserId;
use systemprompt_marketplace::managed::{
    AssetDigest, ComparisonEvidence, ManagedError, ManagedRepository, ManagedResolution,
    NewResource, NewRevision, PublicationAction, PublicationRequest, ResourceKind, TextCandidate,
};

fn human_review() -> ComparisonEvidence {
    ComparisonEvidence {
        experiment_id: None,
        recorded: std::collections::BTreeMap::from([(
            "review".to_owned(),
            serde_json::Value::from("independent human"),
        )]),
    }
}

#[tokio::test]
async fn text_candidate_preserves_baseline_metadata_and_publication_while_inventory_advances() {
    let f = fixture()
        .await
        .expect("managed lifecycle database required");
    publish(&f).await;
    let before = f
        .repository
        .get_revision(&f.owner, &f.revision)
        .await
        .unwrap();
    let before_files = f
        .repository
        .get_revision_files(&f.owner, &f.revision)
        .await
        .unwrap();
    let published = f
        .repository
        .resolve_managed(&f.owner, ResourceKind::Skill, &f.key)
        .await
        .unwrap();
    let candidate = f
        .repository
        .create_text_candidate(
            &f.owner,
            &f.revision,
            &TextCandidate {
                path: "index.md".into(),
                content: "# independently reviewed candidate\n".into(),
                rationale: "candidate is not publication".into(),
            },
        )
        .await
        .unwrap();
    let after = f
        .repository
        .get_revision(&f.owner, &candidate)
        .await
        .unwrap();
    assert_eq!(after.parent_id.as_ref(), Some(&f.revision));
    assert_eq!(after.snapshot_id, before.snapshot_id);
    assert_eq!(after.dependencies, before.dependencies);
    let files = f
        .repository
        .get_revision_files(&f.owner, &candidate)
        .await
        .unwrap();
    assert_eq!(
        files.0["config.yaml"].bytes,
        before_files.0["config.yaml"].bytes
    );
    assert_eq!(
        files.0["index.md"].media_type,
        before_files.0["index.md"].media_type
    );
    assert_eq!(
        files.0["index.md"].executable,
        before_files.0["index.md"].executable
    );
    assert!(
        f.repository
            .get_revision_files(&f.owner, &f.revision)
            .await
            .unwrap()
            .same_content(&before_files)
    );
    let comparison = f
        .repository
        .compare_revisions(&f.owner, &f.revision, &candidate)
        .await
        .unwrap();
    assert_eq!(comparison.baseline, f.revision);
    assert_eq!(comparison.candidate, candidate);
    assert_eq!(comparison.changes.len(), 1);
    assert!(!comparison.dependencies_changed);
    assert!(!comparison.source_snapshot_changed);
    assert!(
        f.repository
            .compare_revisions(&f.owner, &candidate, &candidate)
            .await
            .unwrap()
            .changes
            .is_empty()
    );
    let inventory = f.repository.list_resources(&f.owner, 0).await.unwrap();
    assert!(!inventory.has_more);
    assert_eq!(inventory.items.len(), 1);
    assert_eq!(inventory.items[0].id, f.resource);
    assert_eq!(inventory.items[0].kind, ResourceKind::Skill);
    assert_eq!(inventory.items[0].revision_count, 2);
    assert_eq!(
        inventory.items[0].latest_revision.as_ref(),
        Some(&candidate)
    );
    let revisions = f
        .repository
        .list_revisions(&f.owner, &f.resource, 0)
        .await
        .unwrap();
    assert!(!revisions.has_more);
    assert_eq!(revisions.items.len(), 2);
    assert_eq!(revisions.items[0].id, candidate);
    assert_eq!(revisions.items[0].parent_id.as_ref(), Some(&f.revision));
    assert_eq!(revisions.items[0].rationale, "candidate is not publication");
    assert!(revisions.items[0].created_at >= revisions.items[1].created_at);
    assert_eq!(
        f.repository
            .list_revisions(&f.owner, &f.resource, 1)
            .await
            .unwrap()
            .items[0]
            .id,
        f.revision
    );
    assert!(
        f.repository
            .list_resources(&f.owner, 1)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        f.repository
            .resolve_managed(&f.owner, ResourceKind::Skill, &f.key)
            .await
            .unwrap(),
        published
    );
}

#[tokio::test]
async fn invalid_text_edits_and_foreign_comparisons_do_not_create_revisions() {
    let f = fixture()
        .await
        .expect("managed lifecycle database required");
    for (path, content) in [
        ("missing.md", "new".to_owned()),
        ("index.md", "# managed instructions\n".to_owned()),
        ("index.md", "x".repeat(1024 * 1024 + 1)),
    ] {
        assert!(matches!(
            f.repository
                .create_text_candidate(
                    &f.owner,
                    &f.revision,
                    &TextCandidate {
                        path: path.into(),
                        content,
                        rationale: "invalid edit".into(),
                    }
                )
                .await,
            Err(ManagedError::Invalid(_))
        ));
    }
    let manifest = f
        .repository
        .get_revision(&f.owner, &f.revision)
        .await
        .unwrap();
    let mut binary = f
        .repository
        .get_revision_files(&f.owner, &f.revision)
        .await
        .unwrap();
    binary.0.get_mut("index.md").unwrap().bytes = vec![0xff, 0xfe];
    let binary_revision = f
        .repository
        .create_revision(
            &f.owner,
            &NewRevision {
                resource_id: f.resource.clone(),
                snapshot_id: manifest.snapshot_id.clone(),
                parent_id: Some(f.revision.clone()),
                files: binary.clone(),
                dependencies: manifest.dependencies.clone(),
                rationale: "binary asset".into(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        f.repository
            .create_text_candidate(
                &f.owner,
                &binary_revision,
                &TextCandidate {
                    path: "index.md".into(),
                    content: "replacement".into(),
                    rationale: "requires asset editor".into(),
                }
            )
            .await,
        Err(ManagedError::Invalid(_))
    ));
    let source = f
        .repository
        .list_resources(&f.owner, 0)
        .await
        .unwrap()
        .items[0]
        .source_id
        .clone();
    let other = f
        .repository
        .bind_resource(
            &f.owner,
            &NewResource {
                source_id: source,
                upstream_key: "other".into(),
                kind: ResourceKind::Skill,
                resource_key: "other".into(),
            },
        )
        .await
        .unwrap();
    let other_revision = f
        .repository
        .create_revision(
            &f.owner,
            &NewRevision {
                resource_id: other,
                snapshot_id: manifest.snapshot_id,
                parent_id: None,
                files: binary,
                dependencies: manifest.dependencies,
                rationale: "separate resource".into(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        f.repository
            .compare_revisions(&f.owner, &f.revision, &other_revision)
            .await,
        Err(ManagedError::Invalid(_))
    ));
    let stranger = UserId::new("foreign-authoring-owner");
    assert!(matches!(
        f.repository.revision_resource(&stranger, &f.revision).await,
        Err(ManagedError::Unavailable)
    ));
    assert!(matches!(
        f.repository
            .compare_revisions(&stranger, &f.revision, &binary_revision)
            .await,
        Err(ManagedError::Unavailable)
    ));
    assert!(matches!(
        f.repository
            .create_text_candidate(
                &stranger,
                &f.revision,
                &TextCandidate {
                    path: "index.md".into(),
                    content: "foreign edit".into(),
                    rationale: "denied".into(),
                }
            )
            .await,
        Err(ManagedError::Unavailable)
    ));
    assert!(
        f.repository
            .list_resources(&stranger, 0)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(matches!(
        f.repository.list_revisions(&stranger, &f.resource, 0).await,
        Err(ManagedError::Unavailable)
    ));
    for offset in [-1, 500_001] {
        assert!(f.repository.list_resources(&f.owner, offset).await.is_err());
        assert!(
            f.repository
                .list_revisions(&f.owner, &f.resource, offset)
                .await
                .is_err()
        );
    }
    assert_eq!(
        f.repository
            .list_revisions(&f.owner, &f.resource, 0)
            .await
            .unwrap()
            .items
            .len(),
        2
    );
    assert!(matches!(
        f.repository
            .resolve_managed(&f.owner, ResourceKind::Skill, &f.key)
            .await
            .unwrap(),
        ManagedResolution::NeverAdopted { .. }
    ));
}

#[tokio::test]
async fn withdrawal_and_rollback_keep_bounded_review_history_and_generation_pinned_content() {
    let f = fixture()
        .await
        .expect("managed lifecycle database required");
    publish(&f).await;
    let initial = f
        .repository
        .list_publication_history(&f.owner, &f.resource)
        .await
        .unwrap()[0]
        .decision
        .clone();
    let digest = initial.bundle_digest.clone().unwrap();
    let candidate = f
        .repository
        .create_text_candidate(
            &f.owner,
            &f.revision,
            &TextCandidate {
                path: "index.md".into(),
                content: "# unpublished".into(),
                rationale: "not rollback eligible".into(),
            },
        )
        .await
        .unwrap();
    let mut rollback = PublicationRequest {
        resource_id: f.resource.clone(),
        revision_id: Some(candidate),
        action: PublicationAction::Rollback,
        expected_generation: 1,
        operation_key: "retained-rollback".into(),
        comparison_evidence: human_review(),
        limitations: "prior content only".into(),
    };
    assert!(matches!(
        f.repository
            .review_and_publish(&f.owner, &f.owner, &rollback)
            .await,
        Err(ManagedError::Conflict(_))
    ));
    assert_eq!(
        f.repository
            .list_publication_history(&f.owner, &f.resource)
            .await
            .unwrap()
            .len(),
        1
    );
    withdraw(&f).await;
    rollback.revision_id = Some(f.revision.clone());
    assert!(matches!(
        f.repository
            .review_and_publish(&f.owner, &f.owner, &rollback)
            .await,
        Err(ManagedError::Conflict(_))
    ));
    rollback.expected_generation = 2;
    let restored = f
        .repository
        .review_and_publish(&f.owner, &f.owner, &rollback)
        .await
        .unwrap();
    assert_eq!(restored.generation, 3);
    assert_eq!(restored.bundle_digest.as_ref(), Some(&digest));
    assert_eq!(
        f.repository
            .review_and_publish(&f.owner, &f.owner, &rollback)
            .await
            .unwrap(),
        restored
    );
    rollback.limitations.push_str(" conflicting retry");
    assert!(matches!(
        f.repository
            .review_and_publish(&f.owner, &f.owner, &rollback)
            .await,
        Err(ManagedError::Conflict(_))
    ));
    let all = f
        .repository
        .list_publication_history(&f.owner, &f.resource)
        .await
        .unwrap();
    assert_eq!(
        all.iter()
            .map(|r| r.decision.generation)
            .collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
    assert_eq!(all[0].reviewer_id, f.owner);
    assert_eq!(all[0].comparison_evidence, human_review());
    assert_eq!(all[0].limitations, "prior content only");
    assert!(
        all.iter()
            .all(|r| !r.distributed && !r.installation_verified)
    );
    let first = f
        .repository
        .publication_history_page(&f.owner, &f.resource, None, 1)
        .await
        .unwrap();
    assert_eq!(first[0].decision, restored);
    let rest = f
        .repository
        .publication_history_page(&f.owner, &f.resource, Some(3), 100)
        .await
        .unwrap();
    assert_eq!(rest.len(), 2);
    assert_eq!(rest[0].decision.action, PublicationAction::Withdraw);
    assert_eq!(rest[1].decision, initial);
    assert!(
        f.repository
            .publication_history_page(&f.owner, &f.resource, Some(1), 1)
            .await
            .unwrap()
            .is_empty()
    );
    for limit in [0, 101] {
        assert!(
            f.repository
                .publication_history_page(&f.owner, &f.resource, None, limit)
                .await
                .is_err()
        );
    }
    let stranger = UserId::new("foreign-history-owner");
    assert!(
        f.repository
            .publication_history_page(&stranger, &f.resource, None, 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        f.repository
            .list_publication_history(&stranger, &f.resource)
            .await
            .unwrap()
            .is_empty()
    );
    let old = f
        .repository
        .get_publication_bundle(&f.owner, &f.resource, 1, &digest)
        .await
        .unwrap();
    let new = f
        .repository
        .get_publication_bundle(&f.owner, &f.resource, 3, &digest)
        .await
        .unwrap();
    assert_eq!(old.digest().unwrap(), new.digest().unwrap());
    assert!(matches!(
        f.repository
            .get_publication_bundle(&f.owner, &f.resource, 2, &digest)
            .await,
        Err(ManagedError::Unavailable)
    ));
    assert!(matches!(
        f.repository
            .get_publication_bundle(&stranger, &f.resource, 1, &digest)
            .await,
        Err(ManagedError::Unavailable)
    ));
    assert!(matches!(
        f.repository
            .get_publication_bundle(&f.owner, &f.resource, 1, &AssetDigest::of(b"wrong"))
            .await,
        Err(ManagedError::Integrity)
    ));
}

#[tokio::test]
async fn resource_listing_reports_has_more_across_the_page_boundary() {
    let Some(f) = fixture().await else {
        return;
    };
    let source = f
        .repository
        .list_resources(&f.owner, 0)
        .await
        .unwrap()
        .items[0]
        .source_id
        .clone();
    for index in 0..ManagedRepository::PAGE_SIZE {
        f.repository
            .bind_resource(
                &f.owner,
                &NewResource {
                    source_id: source.clone(),
                    upstream_key: format!("paged_{index:03}"),
                    kind: ResourceKind::Skill,
                    resource_key: format!("paged_{index:03}"),
                },
            )
            .await
            .unwrap();
    }
    let first = f.repository.list_resources(&f.owner, 0).await.unwrap();
    assert!(first.has_more, "51 resources overflow a 50-item page");
    assert_eq!(
        i64::try_from(first.items.len()).unwrap(),
        ManagedRepository::PAGE_SIZE
    );
    let second = f
        .repository
        .list_resources(&f.owner, ManagedRepository::PAGE_SIZE)
        .await
        .unwrap();
    assert!(!second.has_more);
    assert_eq!(second.items.len(), 1);
    let ids: std::collections::BTreeSet<_> = first
        .items
        .iter()
        .chain(second.items.iter())
        .map(|item| item.id.clone())
        .collect();
    assert_eq!(ids.len(), 51, "the two pages partition the listing");
}
