use super::*;
use systemprompt_evaluation::repository::experiments::CampaignExperiment;
use systemprompt_identifiers::EvalCampaignId;

#[tokio::test]
async fn unsupported_native_targets_leave_budget_and_campaign_state_unchanged() {
    let pool = runs_pool()
        .await
        .expect("admission regression requires the fixture database");
    let f = fixture(&pool).await;
    let budgets = crate::seams::budgets(&pool);
    let production = crate::seams::experiments(&pool, crate::seams::verified_admission());
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
    let production = crate::seams::experiments(&pool, crate::seams::verified_admission());
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

#[derive(Debug)]
struct WrongPlatformAdmission(systemprompt_evaluation::capabilities::VerifiedNativeTarget);

impl systemprompt_evaluation::capabilities::ExecutionAdmission for WrongPlatformAdmission {
    fn admit(&self, spec: &ExperimentSpec) -> systemprompt_evaluation::Result<()> {
        for variant in &spec.variants {
            if !self
                .0
                .matches(variant, std::env::consts::OS, std::env::consts::ARCH)
            {
                return Err(EvaluationError::InvalidSpec(
                    "Native target platform mismatch".to_owned(),
                ));
            }
        }
        Err(EvaluationError::InvalidSpec(
            "Fixture cannot enable native execution".to_owned(),
        ))
    }
}

#[tokio::test]
async fn wrong_platform_target_rejection_precedes_any_budget_reservation() {
    let pool = runs_pool().await.expect("fixture database");
    let f = fixture(&pool).await;
    let spec = f.spec(vec![f.case.clone()], f.rubric.clone(), 1);
    let variant = &spec.variants[0];
    let target = systemprompt_evaluation::capabilities::VerifiedNativeTarget {
        client: variant.client,
        platform: "unsupported-fixture-platform".to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        client_version: variant.client_version.clone(),
        adapter_version: "fixture-platform-denial".to_owned(),
        image_digest: variant.worker_image_digest.clone(),
        executable_digest: "a".repeat(64),
        native_isolation_evidence_digest: "b".repeat(64),
        native_metering_evidence_digest: "c".repeat(64),
    };
    assert!(target.matches(
        variant,
        "unsupported-fixture-platform",
        std::env::consts::ARCH
    ));
    let repository =
        crate::seams::experiments(&pool, std::sync::Arc::new(WrongPlatformAdmission(target)));
    let budgets = crate::seams::budgets(&pool);
    let before = budgets.get(&f.owner, &f.budget).await.unwrap();
    let error = repository
        .create_with_budget(&f.owner, "wrong-platform", &f.budget, &spec)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("platform mismatch"));
    let after = budgets.get(&f.owner, &f.budget).await.unwrap();
    assert_eq!(
        (before.reserved, before.settled, before.frozen),
        (after.reserved, after.settled, after.frozen)
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM eval_budget_reservations WHERE account_id=$1")
            .bind(f.budget.as_str())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
}
