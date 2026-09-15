//! Persistence, semantic holdout independence, and actual admission
//! regressions.
use super::*;
use systemprompt_evaluation::campaigns::diagnostics::{
    DiagnosticCode, DiagnosticRecord, DiagnosticStage,
};
use systemprompt_evaluation::campaigns::holdout::{HoldoutConfirmation, HoldoutProposalRequest};
use systemprompt_evaluation::campaigns::{CampaignPolicy, OptimizationObjective};
use systemprompt_evaluation::repository::experiments::CampaignExperiment;
use systemprompt_identifiers::{EvalCampaignId, EvalExperimentId, EvalHoldoutProposalId};

fn proposal<'a>(
    campaign: &'a EvalCampaignId,
    development: &'a EvalExperimentId,
    key: &'a str,
    spec: &'a ExperimentSpec,
) -> HoldoutProposalRequest<'a> {
    HoldoutProposalRequest {
        campaign,
        development,
        key,
        spec,
        counts: (10, 10),
    }
}

fn confirmation<'a>(
    actor: &'a UserId,
    campaign: &'a EvalCampaignId,
    id: &'a EvalHoldoutProposalId,
    digest: &'a str,
) -> HoldoutConfirmation<'a> {
    HoldoutConfirmation {
        actor,
        campaign,
        id,
        digest,
    }
}

fn policy(f: &Fixture) -> CampaignPolicy {
    CampaignPolicy {
        name: "completion".to_owned(),
        resource_id: f.resource.clone(),
        baseline_revision_id: f.baseline.clone(),
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
    let repo = crate::seams::campaigns(&pool);
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
    let repo = crate::seams::campaigns(&pool);
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
        .propose_holdout(&f.owner, proposal(&campaign, &experiment, "review", &spec))
        .await
        .unwrap();
    let retry = repo
        .propose_holdout(&f.owner, proposal(&campaign, &experiment, "review", &spec))
        .await
        .unwrap();
    assert_eq!(first.id, retry.id);
    assert!(first.confirmed_by.is_none());
    let mut conflict = spec.clone();
    conflict.name = "changed".to_owned();
    assert!(
        repo.propose_holdout(
            &f.owner,
            proposal(&campaign, &experiment, "review", &conflict)
        )
        .await
        .is_err()
    );
    assert!(
        repo.confirm_holdout(
            &f.owner,
            confirmation(&f.owner, &campaign, &first.id, "wrong")
        )
        .await
        .is_err()
    );
    assert!(
        sqlx::query(
            "UPDATE eval_campaign_holdout_proposals SET spec_digest='tampered' WHERE id=$1"
        )
        .bind(first.id.as_str())
        .execute(&pool)
        .await
        .is_err(),
        "retained proposal cannot be rewritten after review"
    );
    let confirmed = repo
        .confirm_holdout(
            &f.owner,
            confirmation(&f.owner, &campaign, &first.id, &first.spec_digest),
        )
        .await
        .unwrap();
    assert_eq!(confirmed.confirmed_by, Some(f.owner.clone()));
    assert!(confirmed.confirmed_at.is_some());
    let unconfirmed = repo
        .propose_holdout(
            &f.owner,
            proposal(&campaign, &experiment, "unconfirmed", &spec),
        )
        .await
        .unwrap();
    assert!(
        repo.attach_holdout_run(&f.owner, &campaign, &unconfirmed.id, &experiment)
            .await
            .is_err(),
        "an unconfirmed proposal does not attach a run"
    );
    let attached = repo
        .attach_holdout_run(&f.owner, &campaign, &first.id, &experiment)
        .await
        .unwrap();
    assert_eq!(attached.experiment_id, Some(experiment.clone()));
    assert!(
        repo.attach_holdout_run(&f.owner, &campaign, &first.id, &experiment)
            .await
            .is_ok(),
        "re-attaching the same run is idempotent"
    );
    assert!(
        repo.attach_holdout_run(
            &f.owner,
            &campaign,
            &first.id,
            &EvalExperimentId::generate()
        )
        .await
        .is_err(),
        "a proposal bound to one run refuses another"
    );
    repo.record_diagnostic(
        &f.owner,
        DiagnosticRecord {
            actor: &f.owner,
            campaign: Some(&campaign),
            operation: "blocked",
            stage: DiagnosticStage::Holdout,
            code: DiagnosticCode::UnsupportedCapability,
        },
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
        !crate::seams::experiments(&pool, crate::seams::verified_admission())
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

#[tokio::test]
async fn transition_retries_acknowledge_retained_actions_without_new_generations() {
    use systemprompt_evaluation::campaigns::repository::{CampaignAction, CampaignTransition};
    use systemprompt_evaluation::models::CampaignStatus;
    let step = |expected_generation: i64, action: CampaignAction| CampaignTransition {
        expected_generation,
        action,
    };
    let pool = runs_pool().await.expect("fixture database");
    let f = fixture(&pool).await;
    let repo = crate::seams::campaigns(&pool);
    let campaign = repo
        .create(&f.owner, &f.owner, "transitions", &policy(&f))
        .await
        .unwrap();
    let (first, retry) = tokio::join!(
        repo.transition(
            &f.owner,
            &f.owner,
            &campaign,
            step(0, CampaignAction::Pause)
        ),
        repo.transition(
            &f.owner,
            &f.owner,
            &campaign,
            step(0, CampaignAction::Pause)
        )
    );
    first.unwrap();
    retry.unwrap();
    assert_eq!(repo.get(&f.owner, &campaign).await.unwrap().generation, 1);
    assert!(
        repo.transition(
            &f.owner,
            &f.owner,
            &campaign,
            step(0, CampaignAction::Cancel)
        )
        .await
        .is_err()
    );
    repo.transition(
        &f.owner,
        &f.owner,
        &campaign,
        step(1, CampaignAction::Resume),
    )
    .await
    .unwrap();
    repo.transition(
        &f.owner,
        &f.owner,
        &campaign,
        step(0, CampaignAction::Pause),
    )
    .await
    .unwrap();
    let final_state = repo.get(&f.owner, &campaign).await.unwrap();
    assert_eq!(final_state.generation, 2);
    assert_eq!(final_state.status, CampaignStatus::Active);
}

#[path = "campaign_comparison.rs"]
mod comparison;
#[path = "campaign_report.rs"]
mod report;
