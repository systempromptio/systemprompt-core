use crate::consumer_fixture::{Fixture, fixture};
use systemprompt_identifiers::{ConsumerInstallationId, SessionId};
use systemprompt_marketplace::managed::{
    AssetDigest, ClientEvidence, DistributionState, InstallationReceiptRequest, InstalledFile,
    ManagedError,
};

fn evidence(f: &Fixture, session: &str) -> ClientEvidence {
    ClientEvidence {
        session_id: SessionId::new(session),
        owner_id: f.owner.clone(),
        recorded: std::collections::BTreeMap::new(),
    }
}

fn receipt(f: &Fixture) -> InstallationReceiptRequest {
    InstallationReceiptRequest {
        installation_id: ConsumerInstallationId::new("retained-install"),
        publication_id: f.request.publication_id.clone(),
        resource_id: f.request.resource_id.clone(),
        generation: f.request.generation,
        bundle_digest: AssetDigest::try_from(f.request.bundle_digest.as_str().to_owned()).unwrap(),
        files: f
            .request
            .files
            .iter()
            .map(|file| InstalledFile {
                revision_id: file.revision_id.clone(),
                path: file.path.clone(),
                digest: AssetDigest::try_from(file.digest.as_str().to_owned()).unwrap(),
                bytes: file.bytes,
                executable: file.executable,
            })
            .collect(),
        client_evidence: evidence(f, "retained-session"),
    }
}

async fn deliver(f: &Fixture) {
    let claim = f
        .repo
        .claim_distribution(&f.owner, "retained-delivery")
        .await
        .unwrap()
        .unwrap();
    f.repo
        .complete_distribution(&f.owner, &claim, true, None)
        .await
        .unwrap();
}

#[tokio::test]
async fn delivery_retry_retains_identity_failure_and_terminal_fencing() {
    let f = fixture().await;
    assert!(
        f.repo
            .claim_distribution(&f.consumer, "other-owner")
            .await
            .unwrap()
            .is_none()
    );
    for invalid in [" ".to_owned(), "x".repeat(201)] {
        assert!(f.repo.claim_distribution(&f.owner, &invalid).await.is_err());
    }
    let claim = f
        .repo
        .claim_distribution(&f.owner, "delivery")
        .await
        .unwrap()
        .unwrap();
    let retry = f
        .repo
        .claim_distribution(&f.owner, "delivery")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claim.id, retry.id);
    assert_eq!(claim.payload, retry.payload);
    assert!(
        f.repo
            .complete_distribution(&f.consumer, &claim, true, None)
            .await
            .is_err()
    );
    let mut forged = claim.clone();
    forged.claim_token = "another-token".to_owned();
    assert!(
        f.repo
            .complete_distribution(&f.owner, &forged, true, None)
            .await
            .is_err()
    );
    f.repo
        .complete_distribution(&f.owner, &claim, false, Some("offline"))
        .await
        .unwrap();
    f.repo
        .complete_distribution(&f.owner, &claim, false, Some("offline"))
        .await
        .unwrap();
    let failed = f.repo.list_distribution_status(&f.owner).await.unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].status, DistributionState::Failed);
    assert_eq!(failed[0].error.as_deref(), Some("offline"));
    assert!(failed[0].delivered_at.is_none());
    f.repo
        .complete_distribution(&f.owner, &claim, true, None)
        .await
        .unwrap();
    f.repo
        .complete_distribution(&f.owner, &claim, true, None)
        .await
        .unwrap();
    assert!(
        f.repo
            .complete_distribution(&f.owner, &claim, false, Some("late failure"))
            .await
            .is_err()
    );
    assert!(
        f.repo
            .claim_distribution(&f.owner, "next-delivery")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        f.repo
            .list_distribution_status(&f.consumer)
            .await
            .unwrap()
            .is_empty()
    );
    let completed = f.repo.list_distribution_status(&f.owner).await.unwrap();
    assert_eq!(completed[0].status, DistributionState::Distributed);
    assert!(completed[0].delivered_at.is_some());
}

#[tokio::test]
async fn completion_rejects_mutated_retained_claim_before_acknowledging_outbox() {
    let f = fixture().await;
    let claim = f
        .repo
        .claim_distribution(&f.owner, "identity")
        .await
        .unwrap()
        .unwrap();
    for change in 0..3 {
        let mut forged = claim.clone();
        match change {
            0 => forged.outbox_id = systemprompt_identifiers::EventOutboxId::new("another-outbox"),
            1 => forged.publication_id = systemprompt_identifiers::PublicationId::generate(),
            _ => forged.generation += 1,
        }
        assert!(matches!(
            f.repo
                .complete_distribution(&f.owner, &forged, true, None)
                .await,
            Err(ManagedError::Conflict(_))
        ));
    }
    assert_eq!(
        f.repo.list_distribution_status(&f.owner).await.unwrap()[0].status,
        DistributionState::Claimed
    );
    f.repo
        .complete_distribution(&f.owner, &claim, true, None)
        .await
        .unwrap();
}

#[tokio::test]
async fn historical_readback_rejects_undelivered_partial_duplicate_and_mode_tampering() {
    let f = fixture().await;
    let request = receipt(&f);
    assert!(
        f.repo
            .record_installation(&f.owner, &request)
            .await
            .is_err()
    );
    deliver(&f).await;
    for mutation in 0..5 {
        let mut changed = request.clone();
        match mutation {
            0 => {
                changed.files.pop();
            },
            1 => changed.files.push(changed.files[0].clone()),
            2 => changed.files[0].executable = !changed.files[0].executable,
            3 => changed.files[0].bytes += 1,
            _ => changed.files[0].digest = AssetDigest::of(b"tampered"),
        }
        assert!(matches!(
            f.repo.record_installation(&f.owner, &changed).await,
            Err(ManagedError::Integrity)
        ));
    }
    assert!(
        f.repo
            .list_installation_receipts(&f.owner, None)
            .await
            .unwrap()
            .is_empty()
    );
    let first = f
        .repo
        .record_installation(&f.owner, &request)
        .await
        .unwrap();
    let retry = f
        .repo
        .record_installation(&f.owner, &request)
        .await
        .unwrap();
    assert_eq!(first.id, retry.id);
    assert_eq!(first.verified_at, retry.verified_at);
    let mut conflict = request.clone();
    conflict.client_evidence.session_id = SessionId::new("different-session");
    assert!(matches!(
        f.repo.record_installation(&f.owner, &conflict).await,
        Err(ManagedError::Conflict(_))
    ));
    let retained = f
        .repo
        .list_installation_receipts(&f.owner, Some(&f.request.resource_id))
        .await
        .unwrap();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].installed_manifest, request.files);
    assert!(
        f.repo
            .list_installation_receipts(&f.consumer, None)
            .await
            .unwrap()
            .is_empty()
    );
    let missing = systemprompt_identifiers::ManagedResourceId::generate();
    assert!(
        f.repo
            .list_installation_receipts(&f.owner, Some(&missing))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn historical_evidence_rejects_forged_owner_missing_session_and_oversize_payload() {
    let f = fixture().await;
    let request = receipt(&f);
    for mutation in 0..6 {
        let mut changed = request.clone();
        match mutation {
            0 => changed.installation_id = ConsumerInstallationId::new(""),
            1 => changed.installation_id = ConsumerInstallationId::new("x".repeat(201)),
            2 => changed.generation = 0,
            3 => changed.client_evidence.session_id = SessionId::new(""),
            4 => changed.client_evidence.owner_id = f.consumer.clone(),
            _ => {
                changed
                    .client_evidence
                    .recorded
                    .insert("oversize".to_owned(), "x".repeat(65_537).into());
            },
        }
        assert!(
            f.repo
                .record_installation(&f.owner, &changed)
                .await
                .is_err()
        );
    }
    assert!(
        f.repo
            .list_installation_receipts(&f.owner, None)
            .await
            .unwrap()
            .is_empty()
    );
}
