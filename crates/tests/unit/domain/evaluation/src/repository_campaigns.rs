//! DB-backed tests for the campaign repository's ownership invariant: the
//! baseline revision must be held by the owner and belong to the campaign's
//! resource, verified through `ManagedRevisionOwnership` before anything is
//! persisted.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_evaluation::EvaluationError;
use systemprompt_evaluation::campaigns::repository::CampaignRepository;
use systemprompt_evaluation::campaigns::{CampaignPolicy, OptimizationObjective};
use systemprompt_identifiers::{EvalBudgetId, ManagedResourceId, ResourceRevisionId, UserId};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use systemprompt_traits::{ManagedRevisionOwnership, ManagedSkillResolverError};
use uuid::Uuid;

struct KnownRevision {
    owner: UserId,
    revision: ResourceRevisionId,
    resource: ManagedResourceId,
}

#[async_trait::async_trait]
impl ManagedRevisionOwnership for KnownRevision {
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
    budget: EvalBudgetId,
    resource: ManagedResourceId,
    revision: ResourceRevisionId,
}

async fn fixture() -> Option<Fixture> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let pg = (*pool.write_pool_arc().expect("write pool")).clone();
    let owner = unique_user_id("eval-campaign");
    seed_user_row(&pool, &owner, &format!("{}@example.test", owner.as_str()))
        .await
        .expect("seed owner");
    let resource = ManagedResourceId::new(format!("resource-{}", Uuid::new_v4()));
    let revision = ResourceRevisionId::new(format!("revision-{}", Uuid::new_v4()));
    let budget = crate::seams::budgets(&pg)
        .create_shared(&owner, &format!("budget-{}", Uuid::new_v4()), 1_000_000)
        .await
        .expect("budget");
    let campaigns = CampaignRepository::new(
        pg.clone(),
        Arc::new(KnownRevision {
            owner: owner.clone(),
            revision: revision.clone(),
            resource: resource.clone(),
        }),
    );
    Some(Fixture {
        pg,
        campaigns,
        owner,
        budget,
        resource,
        revision,
    })
}

fn policy(
    fixture: &Fixture,
    resource: ManagedResourceId,
    revision: ResourceRevisionId,
) -> CampaignPolicy {
    CampaignPolicy {
        name: "tighten the skill".to_owned(),
        resource_id: resource,
        baseline_revision_id: revision,
        budget_id: fixture.budget.clone(),
        objective: OptimizationObjective::Quality,
        minimum_quality_milli: 3000,
        minimum_pairs: 2,
        maximum_iterations: 3,
        automatic: false,
    }
}

async fn campaign_count(pg: &PgPool, owner: &UserId) -> i64 {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM eval_campaigns WHERE owner_id=$1"#,
        owner.as_str()
    )
    .fetch_one(pg)
    .await
    .expect("campaign count")
}

#[tokio::test]
async fn a_campaign_adopts_only_a_baseline_the_owner_holds_on_its_resource() {
    let Some(fixture) = fixture().await else {
        return;
    };
    let foreign_revision = ResourceRevisionId::new(format!("foreign-{}", Uuid::new_v4()));
    assert!(
        matches!(
            fixture
                .campaigns
                .create(
                    &fixture.owner,
                    &fixture.owner,
                    "key-foreign",
                    &policy(&fixture, fixture.resource.clone(), foreign_revision),
                )
                .await,
            Err(EvaluationError::ResourceNotFound(_))
        ),
        "a revision the owner does not hold is refused"
    );
    let other_resource = ManagedResourceId::new(format!("other-{}", Uuid::new_v4()));
    assert!(
        matches!(
            fixture
                .campaigns
                .create(
                    &fixture.owner,
                    &fixture.owner,
                    "key-mismatch",
                    &policy(&fixture, other_resource, fixture.revision.clone()),
                )
                .await,
            Err(EvaluationError::InvalidSpec(_))
        ),
        "a baseline from another resource is refused"
    );
    assert_eq!(campaign_count(&fixture.pg, &fixture.owner).await, 0);

    let id = fixture
        .campaigns
        .create(
            &fixture.owner,
            &fixture.owner,
            "key-ok",
            &policy(&fixture, fixture.resource.clone(), fixture.revision.clone()),
        )
        .await
        .expect("a held baseline on the campaign resource is accepted");
    let campaign = fixture
        .campaigns
        .get(&fixture.owner, &id)
        .await
        .expect("get campaign");
    assert_eq!(campaign.policy.baseline_revision_id, fixture.revision);
    assert_eq!(campaign_count(&fixture.pg, &fixture.owner).await, 1);
}
