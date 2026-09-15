use crate::consumer_fixture::{Fixture, fixture};
use systemprompt_identifiers::ResourceRevisionId;
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, ConflictDecision, ConflictResolution, ManagedError, NewRevision,
    ReconciliationRequest, ReconciliationStatus, RevisionFiles,
};

async fn files(f: &Fixture) -> RevisionFiles {
    f.repo
        .get_revision_files(&f.owner, &f.request.revision_id)
        .await
        .unwrap()
}

async fn revision(f: &Fixture, files: RevisionFiles) -> ResourceRevisionId {
    let manifest = f
        .repo
        .get_revision(&f.owner, &f.request.revision_id)
        .await
        .unwrap();
    f.repo
        .create_revision(
            &f.owner,
            &NewRevision {
                resource_id: f.request.resource_id.clone(),
                snapshot_id: manifest.snapshot_id,
                parent_id: Some(f.request.revision_id.clone()),
                files,
                dependencies: manifest.dependencies,
                rationale: "retained reconciliation fixture".to_owned(),
            },
        )
        .await
        .unwrap()
}

fn request(
    f: &Fixture,
    candidate: ResourceRevisionId,
    incoming: ResourceRevisionId,
) -> ReconciliationRequest {
    ReconciliationRequest {
        resource_id: f.request.resource_id.clone(),
        upstream_base_revision_id: f.request.revision_id.clone(),
        managed_candidate_revision_id: candidate,
        incoming_revision_id: incoming,
    }
}

fn asset(bytes: &[u8], executable: bool) -> AssetFile {
    AssetFile {
        bytes: bytes.to_vec(),
        media_type: "text/plain".to_owned(),
        executable,
    }
}

#[tokio::test]
async fn mode_or_media_changes_conflict_with_independent_content_edits_and_remain_resolvable() {
    let f = fixture().await;
    for media in [false, true] {
        let mut candidate_files = files(&f).await;
        let candidate_file = candidate_files.0.get_mut("run.sh").unwrap();
        if media {
            candidate_file.media_type = "application/x-sh".to_owned();
        } else {
            candidate_file.executable = false;
        }
        let mut incoming_files = files(&f).await;
        incoming_files.0.get_mut("run.sh").unwrap().bytes =
            format!("echo incoming {media}").into_bytes();
        let candidate = revision(&f, candidate_files.clone()).await;
        let incoming = revision(&f, incoming_files).await;
        let input = request(&f, candidate.clone(), incoming);
        let opened = f.repo.begin_reconciliation(&f.owner, &input).await.unwrap();
        assert_eq!(
            opened.conflicts.len(),
            1,
            "metadata changes must be compared with the same identity as merge completion"
        );
        assert_eq!(opened.conflicts[0].path, "run.sh");
        f.repo
            .resolve_reconciliation_conflict(
                &f.owner,
                &opened.id,
                &ConflictDecision {
                    path: "run.sh",
                    resolution: ConflictResolution::Candidate,
                    resolved_digest: None,
                },
            )
            .await
            .unwrap();
        let mut wrong = candidate_files;
        let wrong_mode = !wrong.0["run.sh"].executable;
        wrong.0.get_mut("run.sh").unwrap().executable = wrong_mode;
        let wrong_revision = revision(&f, wrong).await;
        assert!(matches!(
            f.repo
                .complete_reconciliation(&f.owner, &f.owner, &opened.id, &wrong_revision)
                .await,
            Err(ManagedError::Conflict(_))
        ));
        f.repo
            .complete_reconciliation(&f.owner, &f.owner, &opened.id, &candidate)
            .await
            .unwrap();
        let retained = f.repo.begin_reconciliation(&f.owner, &input).await.unwrap();
        assert_eq!(retained.status, ReconciliationStatus::Resolved);
        assert_eq!(retained.resolved_revision_id, Some(candidate));
        assert!(matches!(
            retained.conflicts[0].resolution,
            Some(ConflictResolution::Candidate)
        ));
    }
}

#[tokio::test]
async fn each_explicit_resolution_is_durable_owner_scoped_and_checks_resolved_content() {
    let f = fixture().await;
    for (index, choice) in [
        ConflictResolution::Candidate,
        ConflictResolution::Incoming,
        ConflictResolution::Manual,
        ConflictResolution::Delete,
    ]
    .into_iter()
    .enumerate()
    {
        let mut candidate_files = files(&f).await;
        candidate_files.0.insert(
            "run.sh".to_owned(),
            asset(format!("candidate {index}").as_bytes(), false),
        );
        let mut incoming_files = files(&f).await;
        incoming_files.0.insert(
            "run.sh".to_owned(),
            asset(format!("incoming {index}").as_bytes(), true),
        );
        let candidate = revision(&f, candidate_files.clone()).await;
        let incoming = revision(&f, incoming_files.clone()).await;
        let input = request(&f, candidate.clone(), incoming.clone());
        assert!(matches!(
            f.repo.begin_reconciliation(&f.consumer, &input).await,
            Err(ManagedError::Unavailable)
        ));
        let opened = f.repo.begin_reconciliation(&f.owner, &input).await.unwrap();
        assert_eq!(opened.status, ReconciliationStatus::Open);
        assert_eq!(opened.conflicts.len(), 1);
        assert_eq!(
            f.repo
                .begin_reconciliation(&f.owner, &input)
                .await
                .unwrap()
                .id,
            opened.id
        );
        assert!(matches!(
            f.repo
                .complete_reconciliation(&f.owner, &f.owner, &opened.id, &candidate)
                .await,
            Err(ManagedError::Conflict(_))
        ));
        let manual_digest = AssetDigest::of(b"manual resolution");
        let decision = ConflictDecision {
            path: "run.sh",
            resolution: choice,
            resolved_digest: if matches!(choice, ConflictResolution::Manual) {
                Some(manual_digest.as_str())
            } else {
                None
            },
        };
        assert!(
            f.repo
                .resolve_reconciliation_conflict(&f.consumer, &opened.id, &decision)
                .await
                .is_err()
        );
        for invalid in [
            ConflictDecision {
                path: "../run.sh",
                ..decision
            },
            ConflictDecision {
                path: "missing",
                ..decision
            },
            ConflictDecision {
                path: "run.sh",
                resolution: ConflictResolution::Manual,
                resolved_digest: None,
            },
            ConflictDecision {
                path: "run.sh",
                resolution: ConflictResolution::Manual,
                resolved_digest: Some("ABC"),
            },
            ConflictDecision {
                path: "run.sh",
                resolution: ConflictResolution::Candidate,
                resolved_digest: Some(manual_digest.as_str()),
            },
        ] {
            assert!(
                f.repo
                    .resolve_reconciliation_conflict(&f.owner, &opened.id, &invalid)
                    .await
                    .is_err()
            );
        }
        f.repo
            .resolve_reconciliation_conflict(&f.owner, &opened.id, &decision)
            .await
            .unwrap();
        let replay = f.repo.begin_reconciliation(&f.owner, &input).await.unwrap();
        assert!(replay.conflicts[0].resolution.is_some());
        let resolved = match choice {
            ConflictResolution::Candidate => candidate,
            ConflictResolution::Incoming => incoming,
            ConflictResolution::Manual => {
                let wrong = revision(&f, candidate_files.clone()).await;
                assert!(
                    f.repo
                        .complete_reconciliation(&f.owner, &f.owner, &opened.id, &wrong)
                        .await
                        .is_err()
                );
                candidate_files
                    .0
                    .insert("run.sh".to_owned(), asset(b"manual resolution", false));
                revision(&f, candidate_files).await
            },
            ConflictResolution::Delete => {
                candidate_files.0.remove("run.sh");
                revision(&f, candidate_files).await
            },
        };
        assert!(
            f.repo
                .complete_reconciliation(&f.consumer, &f.consumer, &opened.id, &resolved)
                .await
                .is_err()
        );
        f.repo
            .complete_reconciliation(&f.owner, &f.owner, &opened.id, &resolved)
            .await
            .unwrap();
        assert!(
            f.repo
                .resolve_reconciliation_conflict(&f.owner, &opened.id, &decision)
                .await
                .is_err()
        );
        let retained = f.repo.begin_reconciliation(&f.owner, &input).await.unwrap();
        assert_eq!(retained.status, ReconciliationStatus::Resolved);
        assert_eq!(retained.resolved_revision_id, Some(resolved));
    }
}

#[tokio::test]
async fn nonconflicting_deletions_additions_and_identical_changes_merge_without_unreviewed_files() {
    let f = fixture().await;
    let mut base_files = files(&f).await;
    base_files
        .0
        .insert("remove.txt".to_owned(), asset(b"remove", false));
    let base = revision(&f, base_files.clone()).await;
    let mut candidate_files = base_files.clone();
    candidate_files.0.remove("remove.txt");
    candidate_files
        .0
        .insert("candidate.txt".to_owned(), asset(b"candidate only", false));
    candidate_files.0.get_mut("run.sh").unwrap().bytes = b"identical edit".to_vec();
    let mut incoming_files = base_files;
    incoming_files
        .0
        .insert("incoming.txt".to_owned(), asset(b"incoming only", false));
    incoming_files.0.get_mut("run.sh").unwrap().bytes = b"identical edit".to_vec();
    let candidate = revision(&f, candidate_files.clone()).await;
    let incoming = revision(&f, incoming_files).await;
    let mut input = request(&f, candidate, incoming);
    input.upstream_base_revision_id = base;
    let opened = f.repo.begin_reconciliation(&f.owner, &input).await.unwrap();
    assert!(opened.conflicts.is_empty());
    candidate_files
        .0
        .insert("incoming.txt".to_owned(), asset(b"incoming only", false));
    let resolved = revision(&f, candidate_files.clone()).await;
    candidate_files
        .0
        .insert("unreviewed.txt".to_owned(), asset(b"not in any side", true));
    let wrong = revision(&f, candidate_files).await;
    assert!(
        f.repo
            .complete_reconciliation(&f.owner, &f.owner, &opened.id, &wrong)
            .await
            .is_err()
    );
    f.repo
        .complete_reconciliation(&f.owner, &f.owner, &opened.id, &resolved)
        .await
        .unwrap();
    let retained_files = f
        .repo
        .get_revision_files(&f.owner, &resolved)
        .await
        .unwrap();
    assert!(!retained_files.0.contains_key("remove.txt"));
    assert!(retained_files.0.contains_key("candidate.txt"));
    assert!(retained_files.0.contains_key("incoming.txt"));
}

#[tokio::test]
async fn delete_edit_and_add_add_conflicts_are_retained_and_changed_base_retries_rejected() {
    let f = fixture().await;
    let mut candidate_files = files(&f).await;
    candidate_files.0.remove("run.sh");
    candidate_files
        .0
        .insert("added.txt".to_owned(), asset(b"candidate addition", false));
    let mut incoming_files = files(&f).await;
    incoming_files
        .0
        .insert("run.sh".to_owned(), asset(b"incoming edit", true));
    incoming_files
        .0
        .insert("added.txt".to_owned(), asset(b"incoming addition", false));
    let candidate = revision(&f, candidate_files).await;
    let incoming = revision(&f, incoming_files).await;
    let input = request(&f, candidate.clone(), incoming);
    let opened = f.repo.begin_reconciliation(&f.owner, &input).await.unwrap();
    assert_eq!(
        opened
            .conflicts
            .iter()
            .map(|c| c.path.as_str())
            .collect::<Vec<_>>(),
        vec!["added.txt", "run.sh"]
    );
    assert!(opened.conflicts[0].base_digest.is_none());
    assert!(opened.conflicts[1].candidate_digest.is_none());
    let mut changed = input.clone();
    changed.upstream_base_revision_id = candidate.clone();
    assert!(matches!(
        f.repo.begin_reconciliation(&f.owner, &changed).await,
        Err(ManagedError::Conflict(_))
    ));
    changed.incoming_revision_id = ResourceRevisionId::generate();
    assert!(matches!(
        f.repo.begin_reconciliation(&f.owner, &changed).await,
        Err(ManagedError::Unavailable)
    ));
    for path in ["added.txt", "run.sh"] {
        f.repo
            .resolve_reconciliation_conflict(
                &f.owner,
                &opened.id,
                &ConflictDecision {
                    path,
                    resolution: ConflictResolution::Candidate,
                    resolved_digest: None,
                },
            )
            .await
            .unwrap();
    }
    f.repo
        .complete_reconciliation(&f.owner, &f.owner, &opened.id, &candidate)
        .await
        .unwrap();
}
