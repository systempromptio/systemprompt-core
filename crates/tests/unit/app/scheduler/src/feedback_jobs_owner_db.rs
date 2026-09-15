//! The feedback snapshot and facts jobs process the system admin's queues —
//! the owner every analytics route reads under — whatever actor the schedule
//! was configured with.

use std::sync::Arc;

use chrono::Utc;
use systemprompt_analytics::feedback::FeedbackFactsRepository;
use systemprompt_identifiers::{
    Actor, AnalyticsChangeId, AnalyticsFactId, ResourceInvocationId, UserId,
};
use systemprompt_models::feedback::analytics::{
    AnalyticsChange, AnalyticsChangeOperation, AnalyticsFactKey, AnalyticsFactKind,
    InvocationConsumerIdentity, InvocationResourceAttribution, NormalizedAnalyticsFact,
    NormalizedInvocationFact,
};
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::jobs::{FeedbackFactsJob, FeedbackSnapshotsJob};
use systemprompt_test_fixtures::{fixture_app_context, seed_user_row};
use systemprompt_traits::{Job, JobContext};

struct Harness {
    app: Arc<AppContext>,
    ctx: JobContext,
    configured_owner: UserId,
}

async fn harness(pool: &systemprompt_database::DbPool, url: &str) -> Harness {
    let app = fixture_app_context(pool, url).expect("fixture AppContext");
    let admin = app.system_admin().id().clone();
    seed_user_row(pool, &admin, &format!("{admin}@feedback-jobs.invalid"))
        .await
        .expect("system admin row");
    let configured_owner = UserId::new(format!("analytics-bot-{}", uuid::Uuid::new_v4().simple()));
    seed_user_row(
        pool,
        &configured_owner,
        &format!("{configured_owner}@feedback-jobs.invalid"),
    )
    .await
    .expect("configured owner row");

    let app_paths_any: Arc<dyn std::any::Any + Send + Sync> =
        Arc::new(Arc::clone(app.app_paths_arc()));
    let db_pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(Arc::clone(pool));
    let app_context_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(Arc::clone(&app));
    let actor = Actor::job(configured_owner.clone(), "feedback-owner-test".to_string());
    Harness {
        app,
        ctx: JobContext::new(actor, db_pool_any, app_context_any, app_paths_any),
        configured_owner,
    }
}

fn invocation_change(id: &str) -> AnalyticsChange {
    let now = Utc::now();
    AnalyticsChange {
        change_id: AnalyticsChangeId::generate(),
        key: AnalyticsFactKey {
            kind: AnalyticsFactKind::Invocation,
            source: "fixture".to_owned(),
            id: AnalyticsFactId::new(id),
        },
        revision: 1,
        occurred_at: now,
        recorded_at: now,
        operation: AnalyticsChangeOperation::Replace {
            fact: NormalizedAnalyticsFact::Invocation(NormalizedInvocationFact {
                invocation_id: ResourceInvocationId::new(id),
                occurred_at: now,
                consumer: InvocationConsumerIdentity::HistoricalUnknown,
                attribution: InvocationResourceAttribution::Unknown,
                succeeded: true,
                latency_micros: Some(100),
            }),
        },
    }
}

#[tokio::test]
async fn snapshot_job_initialises_the_system_admin_queue_not_the_configured_owner() {
    let (pool, url) = systemprompt_test_fixtures::db_pool_or_skip!();
    let h = harness(&pool, &url).await;
    let admin = h.app.system_admin().id().clone();
    let repository = h.app.feedback_snapshots_repository();

    let result = FeedbackSnapshotsJob
        .execute(&h.ctx)
        .await
        .expect("the snapshot job runs against an empty queue");
    assert!(result.success);

    let admin_health = repository.health(&admin).await.expect("admin health");
    assert_eq!(
        admin_health.last_error, None,
        "the queue the analytics routes read under is initialised"
    );
    let configured_health = repository
        .health(&h.configured_owner)
        .await
        .expect("configured owner health");
    assert_eq!(
        configured_health.last_error.as_deref(),
        Some("Snapshots have not been initialized"),
        "nothing is keyed by the schedule's actor"
    );
}

#[tokio::test]
async fn facts_job_drains_the_system_admin_queue_not_the_configured_owner() {
    let (pool, url) = systemprompt_test_fixtures::db_pool_or_skip!();
    let h = harness(&pool, &url).await;
    let admin = h.app.system_admin().id().clone();
    let facts: &FeedbackFactsRepository = h.app.feedback_facts_repository();
    let id = format!("inv-{}", uuid::Uuid::new_v4().simple());
    facts
        .submit(&admin, &invocation_change(&id))
        .await
        .expect("submit a change under the system admin");
    assert_eq!(facts.health(&admin).await.expect("health").pending, 1);

    let result = FeedbackFactsJob
        .execute(&h.ctx)
        .await
        .expect("the facts job runs");
    assert!(result.success);
    assert_eq!(
        result.items_processed,
        Some(1),
        "the job applied the system admin's pending change"
    );
    let health = facts.health(&admin).await.expect("health");
    assert_eq!(health.pending, 0);
    assert_eq!(health.generation, 1);
    assert_eq!(
        facts
            .health(&h.configured_owner)
            .await
            .expect("configured owner health")
            .generation,
        0,
        "no checkpoint is created for the schedule's actor"
    );
}
