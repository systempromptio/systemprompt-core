//! Persistence, semantic holdout independence, and actual admission
//! regressions.
use super::*;
use systemprompt_evaluation::campaigns::diagnostics::{DiagnosticCode, DiagnosticStage};
use systemprompt_evaluation::campaigns::repository::CampaignRepository;
use systemprompt_evaluation::campaigns::{CampaignPolicy, OptimizationObjective};
use systemprompt_evaluation::repository::experiments::CampaignExperiment;
use systemprompt_identifiers::{ManagedResourceId, ResourceRevisionId};

fn policy(f: &Fixture) -> CampaignPolicy {
    CampaignPolicy {
        name: "completion".to_owned(),
        resource_id: ManagedResourceId::generate(),
        baseline_revision_id: ResourceRevisionId::generate(),
        budget_id: f.budget.clone(),
        objective: OptimizationObjective::Quality,
        minimum_quality_milli: 4000,
        minimum_pairs: 10,
        maximum_iterations: 3,
        automatic: false,
    }
}
#[tokio::test]
async fn blocked_setup_is_retained_idempotently_and_owner_scoped() {
    let pool = runs_pool().await.expect("fixture database");
    let f = fixture(&pool).await;
    let repo = CampaignRepository::new(pool.clone());
    let mut invalid = policy(&f);
    invalid.minimum_pairs = 1;
    for _ in 0..2 {
        assert!(
            repo.create(&f.owner, &f.owner, "bad-setup", &invalid)
                .await
                .is_err()
        );
    }
    let diagnostics = repo.diagnostics(&f.owner, None, None, 1).await.unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].occurrences, 2);
    assert!(!diagnostics[0].remediation.is_empty());
    let other = new_owner(&pool).await;
    assert!(
        repo.diagnostics(&other, None, None, 100)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(repo.diagnostics(&f.owner, None, None, 101).await.is_err());
    assert!(
        repo.diagnostics(&f.owner, None, Some(&diagnostics[0].id), 1)
            .await
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn holdout_proposal_retries_preserve_digest_and_confirmation_identity() {
    let pool = runs_pool().await.expect("fixture database");
    let f = fixture(&pool).await;
    let repo = CampaignRepository::new(pool.clone());
    let campaign = repo
        .create(&f.owner, &f.owner, "campaign", &policy(&f))
        .await
        .unwrap();
    let spec = f.spec(vec![f.case.clone()], f.rubric.clone(), 1);
    let experiment = f
        .experiments
        .create_for_campaign(
            &f.owner,
            &f.owner,
            &CampaignExperiment {
                campaign_id: campaign.clone(),
                idempotency_key: "development".to_owned(),
                spec: spec.clone(),
            },
        )
        .await
        .unwrap();
    let first = repo
        .propose_holdout(&f.owner, &campaign, &experiment, "review", &spec, (10, 10))
        .await
        .unwrap();
    let retry = repo
        .propose_holdout(&f.owner, &campaign, &experiment, "review", &spec, (10, 10))
        .await
        .unwrap();
    assert_eq!(first.id, retry.id);
    assert!(first.confirmed_by.is_none());
    let mut conflict = spec.clone();
    conflict.name = "changed".to_owned();
    assert!(
        repo.propose_holdout(
            &f.owner,
            &campaign,
            &experiment,
            "review",
            &conflict,
            (10, 10)
        )
        .await
        .is_err()
    );
    assert!(
        repo.confirm_holdout(&f.owner, &f.owner, &campaign, &first.id, "wrong")
            .await
            .is_err()
    );
    assert!(
        sqlx::query(
            "UPDATE eval_campaign_holdout_proposals SET spec_digest='tampered' WHERE id=$1"
        )
        .bind(&first.id)
        .execute(&pool)
        .await
        .is_err(),
        "retained proposal cannot be rewritten after review"
    );
    let confirmed = repo
        .confirm_holdout(&f.owner, &f.owner, &campaign, &first.id, &first.spec_digest)
        .await
        .unwrap();
    assert_eq!(confirmed.confirmed_by, Some(f.owner.clone()));
    assert!(confirmed.confirmed_at.is_some());
    repo.record_diagnostic(
        &f.owner,
        &f.owner,
        Some(&campaign),
        "blocked",
        DiagnosticStage::Holdout,
        DiagnosticCode::UnsupportedCapability,
    )
    .await
    .unwrap();
    assert_eq!(
        repo.diagnostics(&f.owner, Some(&campaign), None, 100)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(f.experiments.execution_availability(&spec).admitted);
    assert!(
        !ExperimentRepository::new(pool)
            .execution_availability(&spec)
            .admitted
    );
}
#[tokio::test]
async fn relabelled_development_content_cannot_be_consumed_as_fresh_holdout() {
    let pool = runs_pool().await.expect("fixture database");
    let f = fixture(&pool).await;
    let revisions = RevisionRepository::new(pool.clone());
    let original = f.spec(vec![f.case.clone()], f.rubric.clone(), 1);
    f.experiments
        .create_with_budget(&f.owner, "development-exposure", &f.budget, &original)
        .await
        .unwrap();
    let ResourceContent::Case(mut content) = revisions.get(&f.owner, &f.case).await.unwrap() else {
        panic!("case")
    };
    content.partition = Partition::Holdout;
    let holdout = revisions
        .create(&f.owner, "renamed-holdout", &ResourceContent::Case(content))
        .await
        .unwrap();
    let dataset = ResourceContent::Dataset(vec![holdout.clone()]);
    let dataset_id = revisions
        .create(&f.owner, "renamed-dataset", &dataset)
        .await
        .unwrap();
    let mut spec = f.spec(vec![holdout], f.rubric.clone(), 1);
    spec.dataset = Some(dataset_id);
    spec.frozen.as_mut().unwrap().dataset_digest = content_digest(&dataset).unwrap();
    spec.claim_independent_improvement = true;
    assert!(
        f.experiments
            .create_with_budget(&f.owner, "holdout-reuse", &f.budget, &spec)
            .await
            .is_err()
    );
    assert_eq!(
        f.experiments.list(&f.owner).await.unwrap().len(),
        1,
        "failed independent admission rolls back its execution matrix"
    );
}
