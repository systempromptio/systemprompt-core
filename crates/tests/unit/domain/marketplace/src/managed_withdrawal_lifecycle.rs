//! Approval records intent; only an explicit reviewed publication changes
//! selection.

use crate::source_sync_fixture::{Fixture, files};
use systemprompt_marketplace::managed::{
    ComparisonEvidence, GitSyncResult, ManagedResolution, PublicationAction, PublicationRequest,
    ResourceKind, RevisionFiles, WithdrawalStatus,
};

#[tokio::test]
async fn approved_removal_keeps_content_until_explicit_withdrawal_and_retains_both_reviews() {
    let f = Fixture::new().await;
    let GitSyncResult::Incoming { revision_id, .. } = f.sync().await.unwrap() else {
        panic!("initial retained revision");
    };
    f.publish(revision_id.clone()).await;
    let initial = f
        .repo
        .resolve_managed(&f.owner, ResourceKind::Skill, "alpha")
        .await
        .unwrap();
    f.output('b', RevisionFiles::default());
    let GitSyncResult::WithdrawalProposed { proposal_id, .. } = f.sync().await.unwrap() else {
        panic!("removal review required");
    };
    f.repo
        .decide_withdrawal_proposal(&f.owner, &f.owner, &proposal_id, true)
        .await
        .unwrap();
    let reviewed = f.repo.list_withdrawal_proposals(&f.owner).await.unwrap();
    assert_eq!(reviewed.len(), 1);
    assert_eq!(reviewed[0].status, WithdrawalStatus::Approved);
    assert_eq!(reviewed[0].decided_by.as_ref(), Some(&f.owner));
    assert!(reviewed[0].decided_at.is_some());
    assert_eq!(
        f.repo
            .resolve_managed(&f.owner, ResourceKind::Skill, "alpha")
            .await
            .unwrap(),
        initial
    );
    assert!(
        f.repo
            .decide_withdrawal_proposal(&f.owner, &f.owner, &proposal_id, false)
            .await
            .is_err()
    );
    let withdrawal = f
        .repo
        .review_and_publish(
            &f.owner,
            &f.owner,
            &PublicationRequest {
                resource_id: f.request.resource_id.clone(),
                revision_id: None,
                action: PublicationAction::Withdraw,
                expected_generation: 1,
                operation_key: "approved-source-removal".into(),
                comparison_evidence: ComparisonEvidence {
                    recorded: std::collections::BTreeMap::from([(
                        "withdrawal_proposal".to_owned(),
                        serde_json::to_value(&proposal_id).unwrap(),
                    )]),
                },
                limitations: "source removed; retained evidence remains available".into(),
            },
        )
        .await
        .unwrap();
    assert!(
        matches!(f.repo.resolve_managed(&f.owner, ResourceKind::Skill, "alpha").await.unwrap(),
        ManagedResolution::Withdrawn { generation: 2, publication_id, .. } if publication_id == withdrawal.publication_id)
    );
    assert!(
        f.repo
            .get_revision_files(&f.owner, &revision_id)
            .await
            .unwrap()
            .same_content(&files("base"))
    );
    let inventory = f.repo.list_resources(&f.owner, 0).await.unwrap();
    assert_eq!(inventory.items.len(), 1);
    assert_eq!(inventory.items[0].revision_count, 1);
    let history = f
        .repo
        .list_publication_history(&f.owner, &f.request.resource_id)
        .await
        .unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].decision, withdrawal);
    assert_eq!(history[1].decision.revision_id.as_ref(), Some(&revision_id));
    assert_eq!(
        history[0].comparison_evidence.recorded["withdrawal_proposal"],
        serde_json::to_value(&proposal_id).unwrap()
    );
    assert_eq!(
        f.repo.list_withdrawal_proposals(&f.owner).await.unwrap()[0].status,
        WithdrawalStatus::Approved
    );
}
