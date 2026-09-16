use chrono::{Duration, Utc};
use systemprompt_analytics::feedback::{
    BackfillPage, FactsProcessingService, FeedbackFactsRepository,
};
use systemprompt_identifiers::{
    AnalyticsChangeId, AnalyticsFactId, AnalyticsWorkerId, ManagedResourceId, ResourceInvocationId,
    ResourceRevisionId, TaskId, UserId,
};
use systemprompt_models::feedback::analytics::*;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool, seed_user_row};

struct Fixture {
    repository: FeedbackFactsRepository,
    owner: UserId,
    pool: sqlx::PgPool,
}

impl Fixture {
    async fn new() -> Self {
        let bootstrap = ensure_test_bootstrap();
        let db = fixture_db_pool(&bootstrap.database_url)
            .await
            .expect("test database");
        Self::in_db(&db).await
    }

    async fn in_db(db: &systemprompt_database::DbPool) -> Self {
        let pool = db.write_pool_arc().expect("write pool");
        let owner = UserId::new(uuid::Uuid::new_v4().to_string());
        seed_user_row(db, &owner, &format!("{}@facts.invalid", owner.as_str()))
            .await
            .expect("owner");
        Self {
            repository: FeedbackFactsRepository::new(pool.as_ref().clone()),
            owner,
            pool: pool.as_ref().clone(),
        }
    }

    async fn drain(&self) {
        let worker = AnalyticsWorkerId::generate();
        let service = FactsProcessingService::new(self.repository.clone());
        while service
            .drain(&self.owner, &worker, 64)
            .await
            .expect("drain")
            > 0
        {}
    }
}

fn key(kind: AnalyticsFactKind, id: &str) -> AnalyticsFactKey {
    AnalyticsFactKey {
        kind,
        source: "fixture".to_owned(),
        id: AnalyticsFactId::new(id),
    }
}

fn invocation(id: &str, revision: u64) -> AnalyticsChange {
    let now = Utc::now();
    AnalyticsChange {
        change_id: AnalyticsChangeId::generate(),
        key: key(AnalyticsFactKind::Invocation, id),
        revision,
        occurred_at: now,
        recorded_at: now,
        operation: AnalyticsChangeOperation::Replace {
            fact: NormalizedAnalyticsFact::Invocation(NormalizedInvocationFact {
                invocation_id: ResourceInvocationId::new(id),
                occurred_at: now,
                consumer: InvocationConsumerIdentity::HistoricalUnknown,
                attribution: InvocationResourceAttribution::Unknown,
                skill: None,
                succeeded: true,
                latency_micros: Some(100),
            }),
        },
    }
}


#[path = "feedback_lifecycle.rs"]
mod lifecycle;

#[path = "feedback_queue.rs"]
mod queue;

#[path = "feedback_backfill.rs"]
mod backfill;

#[path = "feedback_reference.rs"]
mod reference;

#[path = "snapshot_support.rs"]
mod snapshots;
