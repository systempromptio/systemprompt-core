use crate::source_sync_fixture::{Fixture, files};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use systemprompt_marketplace::managed::{
    CapturedGitSource, GitCaptureRequest, GitSourceCapture, GitSyncResult,
    GitSynchronizationService, ManagedError, ResourceKind, Result, RevisionFiles, WithdrawalStatus,
};

struct RecoveringCapture(AtomicU8);

impl RecoveringCapture {
    fn new(phase: u8) -> Self {
        Self(AtomicU8::new(phase))
    }
    fn set(&self, phase: u8) {
        self.0.store(phase, Ordering::SeqCst);
    }
}

impl GitSourceCapture for RecoveringCapture {
    fn capture(&self, request: &GitCaptureRequest<'_>) -> Result<CapturedGitSource> {
        assert_eq!(
            request.repository,
            "https://git.example.com/organization.git"
        );
        assert_eq!(request.reference, "refs/heads/main");
        assert_eq!(request.subdirectory, Some("catalog"));
        assert_eq!(request.root, "alpha");
        assert_eq!(request.credential, Some("scoped-source-secret"));
        match self.0.load(Ordering::SeqCst) {
            0 => panic!("fixture capture task panicked"),
            1 => Err(ManagedError::Unavailable),
            2 => Ok(CapturedGitSource {
                commit: "not-a-commit".to_owned(),
                files: files("untrusted invalid capture"),
            }),
            3 => Ok(CapturedGitSource {
                commit: "d".repeat(40),
                files: RevisionFiles::default(),
            }),
            _ => Ok(CapturedGitSource {
                commit: "e".repeat(40),
                files: files("recovered trusted capture"),
            }),
        }
    }
}

async fn durable_capture_counts(fixture: &Fixture) -> (i64, i64) {
    let pool = fixture.db.write_pool_arc().expect("source-sync write pool");
    let snapshots = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM managed_source_snapshots WHERE owner_id = $1 AND source_id = $2",
    )
    .bind(fixture.owner.as_str())
    .bind(fixture.request.source_id.as_str())
    .fetch_one(pool.as_ref())
    .await
    .expect("source-scoped snapshot count");
    let reconciliations = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM managed_reconciliations WHERE owner_id = $1 AND resource_id = $2",
    )
    .bind(fixture.owner.as_str())
    .bind(fixture.request.resource_id.as_str())
    .fetch_one(pool.as_ref())
    .await
    .expect("resource-scoped reconciliation count");
    (snapshots, reconciliations)
}

async fn revision_count(fixture: &Fixture) -> usize {
    fixture
        .repo
        .list_revisions(&fixture.owner, &fixture.request.resource_id, 0)
        .await
        .expect("list retained revisions")
        .items
        .len()
}

#[tokio::test]
async fn panicked_capture_is_atomic_and_a_later_capture_recovers() {
    let mut fixture = Fixture::new().await;
    let GitSyncResult::Incoming {
        revision_id: base, ..
    } = fixture.sync().await.expect("seed trusted revision")
    else {
        panic!("seed capture must be incoming");
    };
    fixture.publish(base.clone()).await;
    fixture.request.upstream_base_revision_id = Some(base);
    let published = fixture
        .repo
        .resolve_managed(&fixture.owner, ResourceKind::Skill, "alpha")
        .await
        .expect("published selection");
    let capture = Arc::new(RecoveringCapture::new(0));
    let service = GitSynchronizationService::new(fixture.repo.clone(), capture.clone());
    let durable_before = durable_capture_counts(&fixture).await;

    let error = service
        .sync(
            &fixture.owner,
            &fixture.request,
            Some("scoped-source-secret"),
        )
        .await
        .expect_err("capture panic is contained");
    assert!(matches!(error, ManagedError::Integrity));
    assert_eq!(revision_count(&fixture).await, 1);
    assert_eq!(durable_capture_counts(&fixture).await, durable_before);
    assert!(
        fixture
            .repo
            .list_withdrawal_proposals(&fixture.owner)
            .await
            .expect("list withdrawal proposals")
            .is_empty()
    );

    capture.set(2);
    let invalid = service
        .sync(
            &fixture.owner,
            &fixture.request,
            Some("scoped-source-secret"),
        )
        .await
        .expect_err("invalid captured commit must not replace the selected revision");
    assert!(matches!(invalid, ManagedError::Integrity));
    assert_eq!(revision_count(&fixture).await, 1);
    assert_eq!(durable_capture_counts(&fixture).await, durable_before);
    assert_eq!(
        fixture
            .repo
            .resolve_managed(&fixture.owner, ResourceKind::Skill, "alpha")
            .await
            .expect("selection after rejected captured commit"),
        published
    );
    assert!(
        fixture
            .repo
            .list_withdrawal_proposals(&fixture.owner)
            .await
            .expect("proposals after rejected captured commit")
            .is_empty()
    );

    capture.set(4);
    let GitSyncResult::Incoming {
        snapshot_id,
        revision_id,
        commit,
        reconciliation_id,
    } = service
        .sync(
            &fixture.owner,
            &fixture.request,
            Some("scoped-source-secret"),
        )
        .await
        .expect("capture recovers after joined task failure")
    else {
        panic!("recovery must retain an incoming revision");
    };
    assert_eq!(commit, "e".repeat(40));
    assert!(reconciliation_id.is_none());
    assert_eq!(revision_count(&fixture).await, 2);
    assert_eq!(
        fixture
            .repo
            .resolve_managed(&fixture.owner, ResourceKind::Skill, "alpha")
            .await
            .expect("publication retained across refresh"),
        published
    );
    assert!(
        fixture
            .repo
            .get_revision_files(&fixture.owner, &revision_id)
            .await
            .expect("recovered revision files")
            .same_content(&files("recovered trusted capture"))
    );
    assert_eq!(
        fixture
            .repo
            .snapshot_provenance(&fixture.owner, &snapshot_id)
            .await
            .expect("recovered provenance")
            .commit
            .as_deref(),
        Some(commit.as_str())
    );
}

#[tokio::test]
async fn rejected_capture_outputs_write_nothing_before_valid_recovery() {
    let fixture = Fixture::new().await;
    let capture = Arc::new(RecoveringCapture::new(1));
    let service = GitSynchronizationService::new(fixture.repo.clone(), capture.clone());

    for phase in [1, 2] {
        capture.set(phase);
        service
            .sync(
                &fixture.owner,
                &fixture.request,
                Some("scoped-source-secret"),
            )
            .await
            .expect_err("unavailable and invalid captures are rejected");
        assert_eq!(revision_count(&fixture).await, 0);
        assert_eq!(durable_capture_counts(&fixture).await, (0, 0));
        assert!(
            fixture
                .repo
                .list_withdrawal_proposals(&fixture.owner)
                .await
                .expect("list proposals after rejected capture")
                .is_empty()
        );
    }

    capture.set(4);
    let result = service
        .sync(
            &fixture.owner,
            &fixture.request,
            Some("scoped-source-secret"),
        )
        .await
        .expect("valid capture follows rejected output");
    assert!(matches!(result, GitSyncResult::Incoming { .. }));
    assert_eq!(revision_count(&fixture).await, 1);
}

#[tokio::test]
async fn repeated_upstream_removal_preserves_rejected_evidence_and_requires_fresh_review() {
    let fixture = Fixture::new().await;
    let service =
        GitSynchronizationService::new(fixture.repo.clone(), Arc::new(RecoveringCapture::new(3)));
    let GitSyncResult::WithdrawalProposed {
        proposal_id: first,
        snapshot_id: first_snapshot,
        ..
    } = service
        .sync(
            &fixture.owner,
            &fixture.request,
            Some("scoped-source-secret"),
        )
        .await
        .expect("first removal capture")
    else {
        panic!("empty capture requires review");
    };
    fixture
        .repo
        .decide_withdrawal_proposal(&fixture.owner, &fixture.owner, &first, false)
        .await
        .expect("reject first removal");

    let GitSyncResult::WithdrawalProposed {
        proposal_id: second,
        snapshot_id: second_snapshot,
        ..
    } = service
        .sync(
            &fixture.owner,
            &fixture.request,
            Some("scoped-source-secret"),
        )
        .await
        .expect("repeat removal capture")
    else {
        panic!("repeat empty capture requires a fresh review");
    };
    assert_ne!(first_snapshot, second_snapshot);
    assert_ne!(first, second);
    assert_eq!(revision_count(&fixture).await, 0);
    let proposals = fixture
        .repo
        .list_withdrawal_proposals(&fixture.owner)
        .await
        .expect("retained withdrawal history");
    assert_eq!(proposals.len(), 2);
    assert_eq!(
        proposals
            .iter()
            .find(|p| p.id == first)
            .expect("first retained")
            .status,
        WithdrawalStatus::Rejected
    );
    assert_eq!(
        proposals
            .iter()
            .find(|p| p.id == second)
            .expect("second retained")
            .status,
        WithdrawalStatus::Pending
    );
}
