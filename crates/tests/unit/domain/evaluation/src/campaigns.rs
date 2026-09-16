//! Campaign contract not covered by the ownership and dispatch suites: the
//! one-past page the listing returns and the single wire name a measurement
//! accepts for its deterministic checks.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_evaluation::campaigns::repository::CampaignRepository;
use systemprompt_evaluation::campaigns::{CampaignPolicy, OptimizationObjective};
use systemprompt_evaluation::repository::experiments::DeterministicMeasurement;
use systemprompt_identifiers::{EvalBudgetId, ManagedResourceId, ResourceRevisionId, UserId};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use systemprompt_traits::{ManagedRevisionOwnership, ManagedSkillResolverError};
use uuid::Uuid;

struct HeldRevision {
    owner: UserId,
    revision: ResourceRevisionId,
    resource: ManagedResourceId,
}

#[async_trait::async_trait]
impl ManagedRevisionOwnership for HeldRevision {
    async fn revision_resource(
        &self,
        owner: &UserId,
        revision: &ResourceRevisionId,
    ) -> Result<Option<ManagedResourceId>, ManagedSkillResolverError> {
        Ok((*owner == self.owner && *revision == self.revision).then(|| self.resource.clone()))
    }
}

struct Fixture {
    pg: PgPool,
    campaigns: CampaignRepository,
    owner: UserId,
    policy: CampaignPolicy,
}

async fn fixture() -> Option<Fixture> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let pg = (*pool.write_pool_arc().expect("write pool")).clone();
    let owner = unique_user_id("eval-campaign-page");
    seed_user_row(&pool, &owner, &format!("{}@example.test", owner.as_str()))
        .await
        .expect("seed owner");
    let resource = ManagedResourceId::new(format!("resource-{}", Uuid::new_v4()));
    let revision = ResourceRevisionId::new(format!("revision-{}", Uuid::new_v4()));
    let budget: EvalBudgetId = crate::seams::budgets(&pg)
        .create_shared(&owner, &format!("budget-{}", Uuid::new_v4()), 5_000_000)
        .await
        .expect("budget");
    let campaigns = CampaignRepository::new(
        pg.clone(),
        Arc::new(HeldRevision {
            owner: owner.clone(),
            revision: revision.clone(),
            resource: resource.clone(),
        }),
    );
    let policy = CampaignPolicy {
        name: "page".to_owned(),
        resource_id: resource,
        baseline_revision_id: revision,
        budget_id: budget,
        objective: OptimizationObjective::Quality,
        minimum_quality_milli: 4000,
        minimum_pairs: 2,
        maximum_iterations: 3,
        automatic: false,
    };
    Some(Fixture {
        pg,
        campaigns,
        owner,
        policy,
    })
}

impl Fixture {
    async fn cleanup(&self) {
        for statement in [
            "DELETE FROM eval_campaign_events WHERE campaign_id IN (SELECT id FROM eval_campaigns \
             WHERE owner_id = $1)",
            "DELETE FROM eval_campaigns WHERE owner_id = $1",
            "DELETE FROM eval_budget_accounts WHERE owner_id = $1",
        ] {
            sqlx::query(statement)
                .bind(self.owner.as_str())
                .execute(&self.pg)
                .await
                .expect("cleanup");
        }
    }
}

#[tokio::test]
async fn listing_returns_one_row_past_the_page_size() {
    let Some(f) = fixture().await else {
        return;
    };
    for _ in 0..=CampaignRepository::PAGE_SIZE {
        f.campaigns
            .create(
                &f.owner,
                &f.owner,
                &format!("campaign-{}", Uuid::new_v4()),
                &f.policy,
            )
            .await
            .expect("campaign");
    }
    let page = f.campaigns.list(&f.owner, None).await.expect("list");
    assert_eq!(page.len(), CampaignRepository::PAGE_SIZE + 1);
    let after = &page[CampaignRepository::PAGE_SIZE - 1].id;
    let rest = f
        .campaigns
        .list(&f.owner, Some(after))
        .await
        .expect("list after");
    assert_eq!(rest.len(), 1);
    assert!(rest.iter().all(|row| row.id > *after));
    f.cleanup().await;
}

#[test]
fn a_measurement_has_one_wire_name_for_its_checks() {
    let checks = serde_json::json!({
        "arithmetic": true, "permissions": true, "evidence_references": true,
        "install_integrity": true, "write_readbacks": true
    });
    let body = |name: &str| {
        serde_json::json!({
            "hard_failures": [], name: checks, "judgment": null, "quality_milli": null,
            "latency_ms": 10, "input_tokens": null, "output_tokens": null, "tool_calls": 0,
            "attempted_cost_microdollars": 0, "accounting_status": "complete",
            "verified_success": false
        })
    };
    assert!(serde_json::from_value::<DeterministicMeasurement>(body("checks")).is_ok());
    assert!(
        serde_json::from_value::<DeterministicMeasurement>(body("deterministic_checks")).is_err(),
        "the storage column name is not an accepted wire name"
    );
}
