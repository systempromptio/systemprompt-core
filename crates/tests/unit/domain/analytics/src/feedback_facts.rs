use chrono::{Duration, Utc};
use systemprompt_analytics::feedback::{
    BackfillPage, FactsProcessingService, FeedbackFactsRepository,
};
use systemprompt_identifiers::{
    AnalyticsChangeId, AnalyticsFactId, AnalyticsWorkerId, ManagedResourceId, ResourceInvocationId,
    ResourceRevisionId, TaskId, UserId,
};
use systemprompt_models::feedback::analytics::*;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, fixture_db_pool, seed_user_row,
};

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

#[tokio::test]
async fn drain_retries_a_failed_fact_write_without_committing_partial_projection() {
    ensure_test_bootstrap();
    let database = DisposableDb::installed("analytics_drain_fault")
        .await
        .expect("private analytics database");
    let db = database.pool().await.expect("private analytics pool");
    let fixture = Fixture::in_db(&db).await;
    let worker = AnalyticsWorkerId::generate();
    let service = FactsProcessingService::new(fixture.repository.clone());
    let change = invocation("write-fault", 1);
    fixture
        .repository
        .submit(&fixture.owner, &change)
        .await
        .expect("queue change");
    let pool = db.write_pool_arc().expect("private writer");
    sqlx::raw_sql(sqlx::AssertSqlSafe(
        "CREATE FUNCTION reject_analytics_fact_projection() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN RAISE EXCEPTION 'fixture analytics projection rejection'; END $$; \
         CREATE TRIGGER reject_analytics_fact_projection BEFORE INSERT ON analytics_normalized_facts \
         FOR EACH ROW EXECUTE FUNCTION reject_analytics_fact_projection()",
    ))
    .execute(pool.as_ref())
    .await
    .expect("install private projection fault");

    assert_eq!(
        service
            .drain(&fixture.owner, &worker, 1)
            .await
            .expect("the worker records a retry rather than losing the change"),
        0
    );
    let failed: (String, i32, Option<String>, i64, i64) = sqlx::query_as(
        "SELECT state, attempts, last_error, \
         (SELECT count(*) FROM analytics_normalized_facts WHERE owner_id = $1), \
         (SELECT count(*) FROM analytics_fact_deltas WHERE owner_id = $1) \
         FROM analytics_fact_changes WHERE owner_id = $1 AND change_id = $2",
    )
    .bind(fixture.owner.as_str())
    .bind(change.change_id.as_str())
    .fetch_one(pool.as_ref())
    .await
    .expect("failed change diagnostic");
    assert_eq!(failed.0, "pending");
    assert_eq!(failed.1, 1);
    assert_eq!(
        failed.2.as_deref(),
        Some("Fact processing failed; retry scheduled")
    );
    assert_eq!((failed.3, failed.4), (0, 0));
    let generation: i64 =
        sqlx::query_scalar("SELECT generation FROM analytics_fact_checkpoints WHERE owner_id = $1")
            .bind(fixture.owner.as_str())
            .fetch_one(pool.as_ref())
            .await
            .expect("checkpoint after rejected projection");
    assert_eq!(generation, 0, "the failed apply transaction must roll back");

    sqlx::raw_sql(sqlx::AssertSqlSafe(
        "DROP TRIGGER reject_analytics_fact_projection ON analytics_normalized_facts; \
         DROP FUNCTION reject_analytics_fact_projection()",
    ))
    .execute(pool.as_ref())
    .await
    .expect("remove private projection fault");
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert_eq!(
        service
            .drain(&fixture.owner, &worker, 1)
            .await
            .expect("the same queued change resumes after the projection is repaired"),
        1
    );
    assert_eq!(
        service
            .drain(&fixture.owner, &worker, 1)
            .await
            .expect("replay finds no duplicate pending change"),
        0
    );
    let recovered: (String, i32, Option<String>, i64, i64, i64) = sqlx::query_as(
        "SELECT state, attempts, last_error, \
         (SELECT count(*) FROM analytics_normalized_facts WHERE owner_id = $1), \
         (SELECT count(*) FROM analytics_fact_deltas WHERE owner_id = $1), \
         (SELECT generation FROM analytics_fact_checkpoints WHERE owner_id = $1) \
         FROM analytics_fact_changes WHERE owner_id = $1 AND change_id = $2",
    )
    .bind(fixture.owner.as_str())
    .bind(change.change_id.as_str())
    .fetch_one(pool.as_ref())
    .await
    .expect("recovered change diagnostic");
    assert_eq!(recovered.0, "applied");
    assert_eq!(recovered.1, 2);
    assert!(recovered.2.is_none());
    assert_eq!((recovered.3, recovered.4, recovered.5), (1, 1, 1));
    let stored = fixture
        .repository
        .get_fact(&fixture.owner, &change.key)
        .await
        .expect("read recovered fact")
        .expect("the recovered change projects its fact");
    assert_eq!(stored.revision, change.revision as i64);
    let expected_fact = match &change.operation {
        AnalyticsChangeOperation::Replace { fact } => Some(fact.clone()),
        AnalyticsChangeOperation::Tombstone => None,
    };
    assert_eq!(stored.fact, expected_fact);

    pool.close().await;
    drop(db);
    database.drop_now().await;
}


#[path = "feedback_lifecycle.rs"]
mod lifecycle;
#[path = "feedback_validation_atomic.rs"]
mod validation_atomic;

#[path = "feedback_queue.rs"]
mod queue;

#[path = "feedback_backfill.rs"]
mod backfill;

#[path = "feedback_reference.rs"]
mod reference;

#[path = "snapshot_support.rs"]
mod snapshots;
