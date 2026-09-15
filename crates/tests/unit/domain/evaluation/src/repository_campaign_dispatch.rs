//! Concurrent campaign dispatch retains one linked matrix and bounded
//! iterations.
use super::*;
use systemprompt_evaluation::campaigns::repository::CampaignAction;
use systemprompt_evaluation::campaigns::{CampaignPolicy, OptimizationObjective};
use systemprompt_evaluation::repository::experiments::CampaignExperiment;

fn policy(f: &Fixture) -> CampaignPolicy {
    CampaignPolicy {
        name: "dispatch acceptance".to_owned(),
        resource_id: f.resource.clone(),
        baseline_revision_id: f.baseline.clone(),
        budget_id: f.budget.clone(),
        objective: OptimizationObjective::Quality,
        minimum_quality_milli: 4000,
        minimum_pairs: 2,
        maximum_iterations: 1,
        automatic: false,
    }
}

#[tokio::test]
async fn concurrent_same_key_dispatch_is_linked_once_and_retry_survives_pause() {
    let pool = runs_pool()
        .await
        .expect("campaign dispatch requires PostgreSQL");
    let f = fixture(&pool).await;
    let campaigns = crate::seams::campaigns(&pool);
    let campaign = campaigns
        .create(&f.owner, &f.owner, "campaign", &policy(&f))
        .await
        .unwrap();
    let input = CampaignExperiment {
        campaign_id: campaign.clone(),
        idempotency_key: "one-operation".to_owned(),
        spec: f.spec(vec![f.case.clone()], f.rubric.clone(), 2),
    };
    let (first, second) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(
            f.experiments
                .create_for_campaign(&f.owner, &f.owner, &input),
            f.experiments
                .create_for_campaign(&f.owner, &f.owner, &input)
        )
    })
    .await
    .expect("bounded concurrent dispatch");
    let id = first.unwrap();
    assert_eq!(second.unwrap(), id);
    assert_eq!(
        campaigns
            .list_experiments(&f.owner, &campaign)
            .await
            .unwrap(),
        vec![id.clone()]
    );
    assert_eq!(
        f.experiments
            .get(&f.owner, &id)
            .await
            .unwrap()
            .executions
            .len(),
        4
    );
    let event_count: i64 = sqlx::query_scalar("SELECT count(*) FROM eval_campaign_events WHERE campaign_id=$1 AND action='experiment_queued'").bind(campaign.as_str()).fetch_one(&pool).await.unwrap();
    assert_eq!(event_count, 1);
    let current = campaigns.get(&f.owner, &campaign).await.unwrap();
    campaigns
        .transition(
            &f.owner,
            &f.owner,
            &campaign,
            (current.generation, CampaignAction::Pause),
        )
        .await
        .unwrap();
    let paused = campaigns.get(&f.owner, &campaign).await.unwrap();
    assert_eq!(
        f.experiments
            .create_for_campaign(&f.owner, &f.owner, &input)
            .await
            .unwrap(),
        id
    );
    assert_eq!(
        campaigns.get(&f.owner, &campaign).await.unwrap().generation,
        paused.generation
    );
    let mut conflicting = input.clone();
    conflicting.spec.name = "changed retained operation".to_owned();
    assert!(
        f.experiments
            .create_for_campaign(&f.owner, &f.owner, &conflicting)
            .await
            .is_err()
    );
    conflicting = input.clone();
    conflicting.idempotency_key = "new-operation".to_owned();
    assert!(
        f.experiments
            .create_for_campaign(&f.owner, &f.owner, &conflicting)
            .await
            .is_err()
    );
    assert_eq!(
        campaigns
            .list_experiments(&f.owner, &campaign)
            .await
            .unwrap(),
        vec![id]
    );
    let budget = crate::seams::budgets(&pool)
        .get(&f.owner, &f.budget)
        .await
        .unwrap();
    assert_eq!((budget.reserved, budget.settled), (0, 0));
}

#[tokio::test]
async fn different_keys_cannot_race_past_the_authorized_iteration_limit() {
    let pool = runs_pool()
        .await
        .expect("campaign dispatch requires PostgreSQL");
    let f = fixture(&pool).await;
    let campaigns = crate::seams::campaigns(&pool);
    let campaign = campaigns
        .create(&f.owner, &f.owner, "campaign", &policy(&f))
        .await
        .unwrap();
    let first = CampaignExperiment {
        campaign_id: campaign.clone(),
        idempotency_key: "first".to_owned(),
        spec: f.spec(vec![f.case.clone()], f.rubric.clone(), 2),
    };
    let second = CampaignExperiment {
        idempotency_key: "second".to_owned(),
        ..first.clone()
    };
    let (one, two) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(
            f.experiments
                .create_for_campaign(&f.owner, &f.owner, &first),
            f.experiments
                .create_for_campaign(&f.owner, &f.owner, &second)
        )
    })
    .await
    .expect("bounded iteration race");
    assert_ne!(one.is_ok(), two.is_ok());
    let linked = campaigns
        .list_experiments(&f.owner, &campaign)
        .await
        .unwrap();
    assert_eq!(linked.len(), 1);
    let (experiments, executions): (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM eval_experiments WHERE owner_id=$1),(SELECT count(*) FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id WHERE e.owner_id=$1)").bind(f.owner.as_str()).fetch_one(&pool).await.unwrap();
    assert_eq!(
        (experiments, executions),
        (1, 4),
        "losing transaction must leave no unlinked matrix"
    );
    let foreign = new_owner(&pool).await;
    assert!(
        f.experiments
            .create_for_campaign(&foreign, &foreign, &first)
            .await
            .is_err()
    );
    assert_eq!(
        campaigns
            .list_experiments(&f.owner, &campaign)
            .await
            .unwrap(),
        linked
    );
}
