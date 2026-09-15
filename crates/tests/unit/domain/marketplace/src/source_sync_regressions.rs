//! Retained sync contracts use a trusted capture fixture, not network
//! acceptance.

use crate::source_sync_fixture::{Fixture, files};
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use systemprompt_marketplace::managed::{
    GitSyncResult, ManagedResolution, NewRevision, ResourceKind, RevisionFiles, SourceSpec,
    WithdrawalStatus,
};

#[tokio::test]
async fn source_sync_preserves_published_revision_and_candidate_with_three_way_incoming() {
    let mut f = Fixture::new().await;
    let GitSyncResult::Incoming {
        snapshot_id,
        revision_id: base,
        commit,
        reconciliation_id,
    } = f.sync().await.unwrap()
    else {
        panic!("incoming base");
    };
    assert!(reconciliation_id.is_none());
    assert_eq!(commit, "a".repeat(40));
    let provenance = f
        .repo
        .snapshot_provenance(&f.owner, &snapshot_id)
        .await
        .unwrap();
    assert_eq!(provenance.source_kind, "git");
    assert_eq!(provenance.commit.as_deref(), Some(commit.as_str()));
    assert_eq!(provenance.importer_version, "managed-git-v1");
    f.publish(base.clone()).await;
    let published = f
        .repo
        .resolve_managed(&f.owner, ResourceKind::Skill, "alpha")
        .await
        .unwrap();
    let candidate_files = files("local candidate");
    let candidate = f
        .repo
        .create_revision(
            &f.owner,
            &NewRevision {
                resource_id: f.request.resource_id.clone(),
                snapshot_id,
                parent_id: Some(base.clone()),
                files: candidate_files.clone(),
                dependencies: BTreeMap::new(),
                rationale: "local reviewed work remains a candidate".into(),
            },
        )
        .await
        .unwrap();
    f.request.upstream_base_revision_id = Some(base.clone());
    f.output('b', files("upstream changed same file"));
    let GitSyncResult::Incoming {
        revision_id: incoming,
        reconciliation_id,
        ..
    } = f.sync().await.unwrap()
    else {
        panic!("incoming update");
    };
    assert!(
        reconciliation_id.is_some(),
        "divergent local candidate enters three-way reconciliation"
    );
    assert_ne!(incoming, candidate);
    assert!(
        f.repo
            .get_revision_files(&f.owner, &candidate)
            .await
            .unwrap()
            .same_content(&candidate_files)
    );
    assert!(
        f.repo
            .get_revision_files(&f.owner, &base)
            .await
            .unwrap()
            .same_content(&files("base"))
    );
    assert_eq!(
        f.repo
            .resolve_managed(&f.owner, ResourceKind::Skill, "alpha")
            .await
            .unwrap(),
        published
    );
    assert!(
        matches!(published, ManagedResolution::Published { revision_id, generation: 1, .. } if revision_id == base)
    );
}

#[tokio::test]
async fn upstream_removal_retains_publication_and_requires_owner_review_without_auto_withdrawal() {
    let f = Fixture::new().await;
    let GitSyncResult::Incoming { revision_id, .. } = f.sync().await.unwrap() else {
        panic!("initial revision");
    };
    f.publish(revision_id).await;
    let published = f
        .repo
        .resolve_managed(&f.owner, ResourceKind::Skill, "alpha")
        .await
        .unwrap();
    f.output('c', RevisionFiles::default());
    let GitSyncResult::WithdrawalProposed {
        proposal_id,
        commit,
        ..
    } = f.sync().await.unwrap()
    else {
        panic!("retained withdrawal proposal");
    };
    assert_eq!(commit, "c".repeat(40));
    assert_eq!(
        f.repo
            .resolve_managed(&f.owner, ResourceKind::Skill, "alpha")
            .await
            .unwrap(),
        published
    );
    let stranger = systemprompt_identifiers::UserId::new("other-source-owner");
    assert!(
        f.repo
            .list_withdrawal_proposals(&stranger)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        f.repo
            .decide_withdrawal_proposal(&stranger, &f.owner, &proposal_id, true)
            .await
            .is_err()
    );
    f.repo
        .decide_withdrawal_proposal(&f.owner, &f.owner, &proposal_id, false)
        .await
        .unwrap();
    assert!(
        f.repo
            .decide_withdrawal_proposal(&f.owner, &f.owner, &proposal_id, true)
            .await
            .is_err()
    );
    let proposals = f.repo.list_withdrawal_proposals(&f.owner).await.unwrap();
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, WithdrawalStatus::Rejected);
    assert_eq!(proposals[0].decided_by.as_ref(), Some(&f.owner));
    assert_eq!(
        f.repo
            .resolve_managed(&f.owner, ResourceKind::Skill, "alpha")
            .await
            .unwrap(),
        published
    );
}

#[tokio::test]
async fn source_sync_validates_owner_root_and_required_credentials_before_capture() {
    let mut f = Fixture::new().await;
    assert!(f.service.sync(&f.owner, &f.request, None).await.is_err());
    assert!(
        f.service
            .sync(&f.owner, &f.request, Some(""))
            .await
            .is_err()
    );
    let stranger = systemprompt_identifiers::UserId::new("stranger-source-owner");
    assert!(
        f.service
            .sync(&stranger, &f.request, Some("scoped-source-secret"))
            .await
            .is_err()
    );
    f.request.upstream_root = "../alpha".into();
    assert!(f.sync().await.is_err());
    f.request.upstream_root = "alpha".into();
    assert_eq!(f.capture.calls.load(Ordering::SeqCst), 0);
    assert!(
        f.service
            .sync(&f.owner, &f.request, Some("wrong-source-secret"))
            .await
            .is_err()
    );
    assert_eq!(f.capture.calls.load(Ordering::SeqCst), 1);
    *f.capture.output.lock().unwrap() = ("INVALID".into(), files("invalid provenance"));
    assert!(f.sync().await.is_err());
    let mut invalid_files = files("invalid path");
    invalid_files
        .0
        .insert("../escape".into(), invalid_files.0["index.md"].clone());
    f.output('d', invalid_files);
    assert!(f.sync().await.is_err());
    f.request.source_id = f
        .repo
        .register_source(&f.owner, "managed-not-git", &SourceSpec::Managed)
        .await
        .unwrap();
    assert!(f.sync().await.is_err());
}

#[tokio::test]
async fn trusted_capture_cannot_bypass_private_network_source_rejection() {
    let mut f = Fixture::new().await;
    let source = f
        .repo
        .register_source(
            &f.owner,
            "blocked-metadata-source",
            &SourceSpec::Git {
                repository: "https://169.254.169.254/private.git".into(),
                reference: "a".repeat(40),
                subdirectory: None,
                credential_reference: Some("private-source-key".into()),
            },
        )
        .await;
    if let Ok(source) = source {
        f.request.source_id = source;
        assert!(f.sync().await.is_err());
    }
    assert_eq!(f.capture.calls.load(Ordering::SeqCst), 0);
}
