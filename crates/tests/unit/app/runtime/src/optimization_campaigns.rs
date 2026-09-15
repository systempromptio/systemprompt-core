//! Runtime admission and recovery preserve retained campaign evidence and
//! budgets.
use super::optimization_fixture::{Fixture, resource};
use systemprompt_evaluation::campaigns::diagnostics::{DiagnosticCode, DiagnosticStage};
use systemprompt_evaluation::campaigns::repository::{CampaignAction, CampaignTransition};
use systemprompt_evaluation::repository::experiments::CampaignExperiment;
use systemprompt_runtime::optimization::SkillOptimizationOrchestrator;

#[tokio::test]
async fn invalid_baseline_retry_is_durable_then_corrected_launch_resolves_and_is_idempotent() {
    let f = Fixture::new().await;
    let mut input = CampaignExperiment {
        campaign_id: f.campaign.clone(),
        idempotency_key: "recover-this-launch".to_owned(),
        spec: f.spec.clone(),
    };
    input.spec.variants.swap(0, 1);
    for _ in 0..2 {
        assert!(
            f.runtime
                .launch(&f.owner, &f.owner, &input)
                .await
                .expect_err("wrong retained baseline")
                .to_string()
                .contains("retained baseline")
        );
    }
    let diagnostics = f
        .repositories
        .campaigns
        .diagnostics(&f.owner, Some(&f.campaign), None, 100)
        .await
        .unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].occurrences, 2);
    assert!(matches!(diagnostics[0].stage, DiagnosticStage::Launch));
    assert!(diagnostics[0].resolved_at.is_none());
    assert_eq!(
        f.repositories
            .budgets
            .get(&f.owner, &f.policy.budget_id)
            .await
            .unwrap()
            .reserved,
        0
    );
    assert!(
        f.repositories
            .campaigns
            .list_experiments(&f.owner, &f.campaign)
            .await
            .unwrap()
            .is_empty()
    );
    input.spec = f.spec.clone();
    let id = f
        .runtime
        .launch(&f.owner, &f.owner, &input)
        .await
        .expect("corrected launch");
    let reserved = f
        .repositories
        .budgets
        .get(&f.owner, &f.policy.budget_id)
        .await
        .unwrap()
        .reserved;
    assert_eq!(
        f.runtime.launch(&f.owner, &f.owner, &input).await.unwrap(),
        id
    );
    assert_eq!(
        f.repositories
            .budgets
            .get(&f.owner, &f.policy.budget_id)
            .await
            .unwrap()
            .reserved,
        reserved
    );
    assert_eq!(
        f.repositories
            .campaigns
            .list_experiments(&f.owner, &f.campaign)
            .await
            .unwrap(),
        vec![id]
    );
    assert!(
        f.repositories
            .campaigns
            .diagnostics(&f.owner, Some(&f.campaign), None, 100)
            .await
            .unwrap()[0]
            .resolved_at
            .is_some()
    );
}

#[tokio::test]
async fn foreign_resource_candidate_and_canonical_unsupported_admission_leave_budget_untouched() {
    let f = Fixture::new().await;
    let (_, foreign) = resource(&f.managed, &f.owner, "other-resource").await;
    let digest = f
        .runtime
        .register_workspace(&f.owner, &foreign)
        .await
        .unwrap();
    assert_eq!(
        f.runtime
            .register_workspace(&f.owner, &foreign)
            .await
            .unwrap(),
        digest
    );
    let mut input = CampaignExperiment {
        campaign_id: f.campaign.clone(),
        idempotency_key: "foreign".to_owned(),
        spec: f.spec.clone(),
    };
    input.spec.variants[1].skill_bundle_digest = digest;
    assert!(
        f.runtime
            .launch(&f.owner, &f.owner, &input)
            .await
            .expect_err("resource mismatch")
            .to_string()
            .contains("campaign resource")
    );
    let canonical = systemprompt_test_fixtures::fixture_evaluation_repositories(&f.db)
        .expect("evaluation repositories");
    let runtime = SkillOptimizationOrchestrator::new(
        f.managed.clone(),
        canonical.clone(),
        canonical.revisions.clone(),
    );
    input.spec = f.spec.clone();
    input.idempotency_key = "unverified-native".to_owned();
    assert!(
        !canonical
            .experiments
            .execution_availability(&input.spec)
            .admitted
    );
    assert!(runtime.launch(&f.owner, &f.owner, &input).await.is_err());
    let budget = f
        .repositories
        .budgets
        .get(&f.owner, &f.policy.budget_id)
        .await
        .unwrap();
    assert_eq!((budget.reserved, budget.settled), (0, 0));
    assert!(
        f.repositories
            .campaigns
            .list_experiments(&f.owner, &f.campaign)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn automatic_missing_template_survives_restart_and_paused_campaign_does_not_retry() {
    let f = Fixture::new().await;
    assert!(
        f.runtime
            .advance(&f.owner, &f.owner, &f.campaign)
            .await
            .unwrap()
            .is_none()
    );
    let restarted = SkillOptimizationOrchestrator::new(
        f.managed.clone(),
        f.repositories.clone(),
        f.repositories.revisions.clone(),
    );
    assert!(
        restarted
            .advance(&f.owner, &f.owner, &f.campaign)
            .await
            .unwrap()
            .is_none()
    );
    let diagnostics = f
        .repositories
        .campaigns
        .diagnostics(&f.owner, Some(&f.campaign), None, 100)
        .await
        .unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert!(matches!(
        diagnostics[0].code,
        DiagnosticCode::MissingTemplate
    ));
    assert!(matches!(
        diagnostics[0].stage,
        DiagnosticStage::AutomaticFollowup
    ));
    assert_eq!(diagnostics[0].occurrences, 2);
    let campaign = f
        .repositories
        .campaigns
        .get(&f.owner, &f.campaign)
        .await
        .unwrap();
    f.repositories
        .campaigns
        .transition(
            &f.owner,
            &f.owner,
            &f.campaign,
            CampaignTransition {
                expected_generation: campaign.generation,
                action: CampaignAction::Pause,
            },
        )
        .await
        .unwrap();
    assert!(
        restarted
            .advance(&f.owner, &f.owner, &f.campaign)
            .await
            .unwrap()
            .is_none()
    );
    let after = f
        .repositories
        .campaigns
        .diagnostics(&f.owner, Some(&f.campaign), None, 100)
        .await
        .unwrap();
    assert_eq!(after[0].occurrences, 2);
    assert!(after[0].resolved_at.is_none());
    assert_eq!(
        f.repositories
            .budgets
            .get(&f.owner, &f.policy.budget_id)
            .await
            .unwrap()
            .reserved,
        0
    );
}

#[tokio::test]
async fn attach_revalidates_candidate_resource_and_iteration_limit_survives_restart() {
    let f = Fixture::new().await;
    let (_, revision) = resource(&f.managed, &f.owner, "foreign-attach").await;
    let mut foreign_spec = f.spec.clone();
    foreign_spec.variants[1].skill_bundle_digest = f
        .runtime
        .register_workspace(&f.owner, &revision)
        .await
        .unwrap();
    let foreign_run = f
        .repositories
        .experiments
        .create_with_budget(
            &f.owner,
            "standalone-foreign",
            &f.policy.budget_id,
            &foreign_spec,
        )
        .await
        .unwrap();
    assert!(
        f.runtime
            .attach(&f.owner, &f.owner, &f.campaign, &foreign_run)
            .await
            .expect_err("cannot attach another resource's candidate")
            .to_string()
            .contains("campaign resource")
    );
    assert!(
        f.repositories
            .campaigns
            .list_experiments(&f.owner, &f.campaign)
            .await
            .unwrap()
            .is_empty()
    );
    for key in ["first", "second"] {
        f.runtime
            .launch(
                &f.owner,
                &f.owner,
                &CampaignExperiment {
                    campaign_id: f.campaign.clone(),
                    idempotency_key: key.to_owned(),
                    spec: f.spec.clone(),
                },
            )
            .await
            .unwrap();
    }
    let before = f
        .repositories
        .budgets
        .get(&f.owner, &f.policy.budget_id)
        .await
        .unwrap();
    let restarted = SkillOptimizationOrchestrator::new(
        f.managed.clone(),
        f.repositories.clone(),
        f.repositories.revisions.clone(),
    );
    assert!(
        restarted
            .advance(&f.owner, &f.owner, &f.campaign)
            .await
            .unwrap()
            .is_none()
    );
    let diagnostics = f
        .repositories
        .campaigns
        .diagnostics(&f.owner, Some(&f.campaign), None, 100)
        .await
        .unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|item| matches!(item.code, DiagnosticCode::IterationLimit)
                && item.resolved_at.is_none())
    );
    assert_eq!(
        f.repositories
            .campaigns
            .list_experiments(&f.owner, &f.campaign)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        f.repositories
            .budgets
            .get(&f.owner, &f.policy.budget_id)
            .await
            .unwrap()
            .reserved,
        before.reserved
    );
}
