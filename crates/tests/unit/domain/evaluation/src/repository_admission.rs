use super::*;
use systemprompt_evaluation::repository::experiments::CampaignExperiment;
use systemprompt_identifiers::EvalCampaignId;

#[tokio::test]
async fn unsupported_native_targets_leave_budget_and_campaign_state_unchanged() {
    let pool = runs_pool()
        .await
        .expect("admission regression requires the fixture database");
    let f = fixture(&pool).await;
    let budgets = BudgetRepository::new(pool.clone());
    let production = ExperimentRepository::new(pool.clone());
    let before = budgets
        .get(&f.owner, &f.budget)
        .await
        .expect("budget before");
    for client in [
        ClientKind::ClaudeCode,
        ClientKind::Opencode,
        ClientKind::Codex,
        ClientKind::Hermes,
    ] {
        let mut spec = f.spec(vec![f.case.clone()], f.rubric.clone(), 1);
        for variant in &mut spec.variants {
            variant.client = client;
        }
        let error = production
            .create_with_budget(&f.owner, "unsupported", &f.budget, &spec)
            .await
            .expect_err("unverified client must not launch");
        assert!(matches!(error, EvaluationError::InvalidSpec(_)));
        let error = production
            .create_for_campaign(
                &f.owner,
                &f.owner,
                &CampaignExperiment {
                    campaign_id: EvalCampaignId::generate(),
                    idempotency_key: "follow-up".to_owned(),
                    spec,
                },
            )
            .await
            .expect_err("automatic campaign follow-ups must use the same admission");
        assert!(
            matches!(error, EvaluationError::InvalidSpec(_)),
            "admission precedes campaign lookup"
        );
    }
    let after = budgets
        .get(&f.owner, &f.budget)
        .await
        .expect("budget after");
    assert_eq!(
        (after.cap, after.reserved, after.settled, after.frozen),
        (before.cap, before.reserved, before.settled, before.frozen)
    );
    assert!(
        production
            .list(&f.owner)
            .await
            .expect("experiments")
            .is_empty()
    );
    let reservations: i64 =
        sqlx::query_scalar("SELECT count(*) FROM eval_budget_reservations WHERE account_id=$1")
            .bind(f.budget.as_str())
            .fetch_one(&pool)
            .await
            .expect("reservations");
    assert_eq!(reservations, 0);
}

#[tokio::test]
async fn production_claim_rejects_a_retained_unverified_fixture_without_advancing_its_fence() {
    let pool = runs_pool()
        .await
        .expect("admission regression requires the fixture database");
    let f = fixture(&pool).await;
    let spec = f.spec(vec![f.case.clone()], f.rubric.clone(), 1);
    let experiment = f
        .experiments
        .create_with_budget(&f.owner, "retained", &f.budget, &spec)
        .await
        .expect("trusted fixture composition");
    let production = ExperimentRepository::new(pool.clone());
    assert!(matches!(
        production.claim(&f.owner, &EvalWorkerId::generate()).await,
        Err(EvaluationError::InvalidSpec(_))
    ));
    let detail = production
        .get(&f.owner, &experiment)
        .await
        .expect("retained matrix");
    assert_eq!(detail.experiment.status, ExperimentStatus::Queued);
    assert_eq!(detail.experiment.accounting.reserved, 0);
    assert!(detail.executions.iter().all(
        |execution| execution.status == ExecutionStatus::Queued && execution.fencing_token == 0
    ));
}
