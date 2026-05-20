use anyhow::Result;
use chrono::{Duration, TimeZone, Utc};
use std::env;
use std::sync::Arc;
use systemprompt_analytics::CostAnalyticsRepository;
use systemprompt_database::{Database, DbPool};
use systemprompt_models::UserId;
use tokio::sync::{Mutex, MutexGuard, OnceCell};
use uuid::Uuid;

// Serialise this module's tests against a single in-process gate. Each test
// here creates its own 50-connection sqlx pool against Postgres; running N of
// them in parallel exhausts `max_connections=100`. Tests in this module are
// fast (~150ms each) so the wall-clock cost of serialising is negligible.
static SERIAL: OnceCell<Mutex<()>> = OnceCell::const_new();

async fn acquire_serial() -> MutexGuard<'static, ()> {
    SERIAL
        .get_or_init(|| async { Mutex::new(()) })
        .await
        .lock()
        .await
}

struct Fixture {
    pool: sqlx::PgPool,
    db: DbPool,
    user_id: String,
    context_id: String,
    window_start: chrono::DateTime<Utc>,
    window_end: chrono::DateTime<Utc>,
    tag: String,
    _guard: MutexGuard<'static, ()>,
}

impl Fixture {
    async fn new() -> Result<Self> {
        let guard = acquire_serial().await;
        let url = env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set for cost reconciliation tests");
        let db = Database::new_postgres(&url).await?;
        let pool = db.pool_arc()?.as_ref().clone();
        let db = Arc::new(db);
        let tag = Uuid::new_v4().simple().to_string();
        let user_id = format!("test_user_{tag}");
        let context_id = format!("test_ctx_{tag}");

        sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $2, $3)")
            .bind(&user_id)
            .bind(&user_id)
            .bind(format!("{user_id}@test.invalid"))
            .execute(&pool)
            .await?;

        sqlx::query("INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)")
            .bind(&context_id)
            .bind(&user_id)
            .bind(format!("ctx-{tag}"))
            .execute(&pool)
            .await?;

        let uuid = Uuid::new_v4();
        let offset_days = i64::from(u32::from_le_bytes(
            uuid.as_bytes()[0..4].try_into().unwrap(),
        ));
        let base = Utc.with_ymd_and_hms(2100, 1, 1, 0, 0, 0).unwrap();
        let window_start = base + Duration::days(offset_days % 1_000_000);
        let window_end = window_start + Duration::days(1);

        Ok(Self {
            pool,
            db,
            user_id,
            context_id,
            window_start,
            window_end,
            tag,
            _guard: guard,
        })
    }

    async fn insert_task(&self, agent_name: Option<&str>) -> Result<String> {
        let task_id = format!("task_{}_{}", self.tag, Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO agent_tasks (task_id, context_id, agent_name, user_id) VALUES ($1, $2, \
             $3, $4)",
        )
        .bind(&task_id)
        .bind(&self.context_id)
        .bind(agent_name)
        .bind(&self.user_id)
        .execute(&self.pool)
        .await?;
        Ok(task_id)
    }

    async fn insert_ai_request(
        &self,
        task_id: Option<&str>,
        cost_microdollars: i64,
        tokens: i32,
        offset_minutes: i64,
    ) -> Result<()> {
        let id = format!("req_{}_{}", self.tag, Uuid::new_v4().simple());
        let created_at = self.window_start + Duration::minutes(offset_minutes);
        sqlx::query(
            "INSERT INTO ai_requests (id, request_id, user_id, task_id, provider, model, \
             cost_microdollars, tokens_used, status, created_at, updated_at, actor_kind, \
             actor_id) VALUES ($1, $2, $3, $4, 'test-provider', 'test-model', $5, $6, \
             'completed', $7, $7, 'user', $3)",
        )
        .bind(&id)
        .bind(&id)
        .bind(&self.user_id)
        .bind(task_id)
        .bind(cost_microdollars)
        .bind(tokens)
        .bind(created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    fn repo(&self) -> Result<CostAnalyticsRepository> {
        Ok(CostAnalyticsRepository::new(&self.db)?)
    }

    async fn cleanup(&self) -> Result<()> {
        sqlx::query("DELETE FROM ai_requests WHERE user_id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[tokio::test]
async fn breakdown_reconciles_with_summary_when_all_attributed() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_a = fx.insert_task(Some(&format!("agent-a-{}", fx.tag))).await?;
    let task_b = fx.insert_task(Some(&format!("agent-b-{}", fx.tag))).await?;
    for i in 0..5 {
        fx.insert_ai_request(Some(&task_a), 1_000 * (i + 1), 100, i)
            .await?;
    }
    for i in 0..5 {
        fx.insert_ai_request(Some(&task_b), 500 * (i + 1), 50, i + 5)
            .await?;
    }

    let repo = fx.repo()?;
    let summary = repo.get_summary(fx.window_start, fx.window_end).await?;
    let breakdown = repo
        .get_breakdown_by_agent(fx.window_start, fx.window_end, 20)
        .await?;

    let breakdown_sum: i64 = breakdown.iter().map(|r| r.cost).sum();
    let expected_total: i64 = (1_000 * (1 + 2 + 3 + 4 + 5)) + (500 * (1 + 2 + 3 + 4 + 5));

    assert_eq!(summary.cost, Some(expected_total));
    assert_eq!(breakdown_sum, expected_total);
    assert!(
        breakdown.iter().all(|r| r.name != "unattributed"),
        "no unattributed row expected when every request is attributed, got: {:?}",
        breakdown.iter().map(|r| &r.name).collect::<Vec<_>>()
    );

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn breakdown_reconciles_with_summary_with_null_task_ids() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_a = fx.insert_task(Some(&format!("agent-a-{}", fx.tag))).await?;
    for i in 0..5 {
        fx.insert_ai_request(Some(&task_a), 2_000, 100, i).await?;
    }
    for i in 0..5 {
        fx.insert_ai_request(None, 1_500, 75, i + 5).await?;
    }

    let repo = fx.repo()?;
    let summary = repo.get_summary(fx.window_start, fx.window_end).await?;
    let breakdown = repo
        .get_breakdown_by_agent(fx.window_start, fx.window_end, 20)
        .await?;

    let breakdown_sum: i64 = breakdown.iter().map(|r| r.cost).sum();
    let expected_total: i64 = (5 * 2_000) + (5 * 1_500);

    assert_eq!(summary.cost, Some(expected_total));
    assert_eq!(breakdown_sum, expected_total);

    let unattributed = breakdown
        .iter()
        .find(|r| r.name == "unattributed")
        .expect("unattributed row must be present when NULL task_ids exist");
    assert_eq!(unattributed.cost, 5 * 1_500);
    assert_eq!(unattributed.requests, 5);

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn unattributed_row_survives_limit() -> Result<()> {
    let fx = Fixture::new().await?;
    let mut tasks = Vec::new();
    for i in 0..6 {
        let t = fx
            .insert_task(Some(&format!("agent-{i}-{}", fx.tag)))
            .await?;
        tasks.push(t);
    }
    for (i, task) in tasks.iter().enumerate() {
        fx.insert_ai_request(Some(task), 10_000 * (i as i64 + 1), 100, i as i64)
            .await?;
    }
    fx.insert_ai_request(None, 50, 1, 20).await?;

    let repo = fx.repo()?;
    let breakdown = repo
        .get_breakdown_by_agent(fx.window_start, fx.window_end, 2)
        .await?;

    assert!(
        breakdown.iter().any(|r| r.name == "unattributed"),
        "unattributed row must survive LIMIT truncation, got: {:?}",
        breakdown.iter().map(|r| &r.name).collect::<Vec<_>>()
    );
    let attributed_count = breakdown
        .iter()
        .filter(|r| r.name != "unattributed")
        .count();
    assert_eq!(
        attributed_count, 2,
        "LIMIT should bound attributed rows to 2"
    );

    fx.cleanup().await?;
    Ok(())
}

async fn make_other_user(fx: &Fixture) -> Result<String> {
    let other_id = format!("test_user_other_{}", fx.tag);
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $2, $3)")
        .bind(&other_id)
        .bind(&other_id)
        .bind(format!("{other_id}@test.invalid"))
        .execute(&fx.pool)
        .await?;
    let other_ctx = format!("test_ctx_other_{}", fx.tag);
    sqlx::query("INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)")
        .bind(&other_ctx)
        .bind(&other_id)
        .bind(format!("ctx-other-{}", fx.tag))
        .execute(&fx.pool)
        .await?;
    let id = format!("req_other_{}", Uuid::new_v4().simple());
    let created_at = fx.window_start + Duration::minutes(2);
    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, context_id, provider, model, \
         cost_microdollars, tokens_used, status, created_at, updated_at, actor_kind, actor_id) \
         VALUES ($1, $2, $3, $4, 'test-provider', 'other-model', 99_999_999, 9_999, 'completed', \
         $5, $5, 'user', $3)",
    )
    .bind(&id)
    .bind(&id)
    .bind(&other_id)
    .bind(&other_ctx)
    .bind(created_at)
    .execute(&fx.pool)
    .await?;
    Ok(other_id)
}

async fn cleanup_other(fx: &Fixture, other_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM ai_requests WHERE user_id = $1")
        .bind(other_id)
        .execute(&fx.pool)
        .await?;
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(other_id)
        .execute(&fx.pool)
        .await?;
    Ok(())
}

async fn insert_request_with_context(
    fx: &Fixture,
    context_id: &str,
    task_id: Option<&str>,
    model: &str,
    cost: i64,
    tokens: i32,
    offset_minutes: i64,
) -> Result<()> {
    let id = format!("req_{}_{}", fx.tag, Uuid::new_v4().simple());
    let created_at = fx.window_start + Duration::minutes(offset_minutes);
    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, context_id, task_id, provider, model, \
         cost_microdollars, tokens_used, status, created_at, updated_at, actor_kind, actor_id) \
         VALUES ($1, $2, $3, $4, $5, 'test-provider', $6, $7, $8, 'completed', $9, $9, 'user', \
         $3)",
    )
    .bind(&id)
    .bind(&id)
    .bind(&fx.user_id)
    .bind(context_id)
    .bind(task_id)
    .bind(model)
    .bind(cost)
    .bind(tokens)
    .bind(created_at)
    .execute(&fx.pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn summary_for_user_isolates_by_user_id() -> Result<()> {
    let fx = Fixture::new().await?;
    let other = make_other_user(&fx).await?;
    fx.insert_ai_request(None, 1_000, 100, 0).await?;
    fx.insert_ai_request(None, 2_500, 250, 1).await?;

    let repo = fx.repo()?;
    let summary = repo
        .get_summary_for_user(&UserId::new(&fx.user_id), fx.window_start, fx.window_end)
        .await?;
    assert_eq!(summary.requests, 2);
    assert_eq!(summary.cost, Some(3_500));

    let other_summary = repo
        .get_summary_for_user(&UserId::new(&other), fx.window_start, fx.window_end)
        .await?;
    assert_eq!(other_summary.requests, 1);
    assert_eq!(other_summary.cost, Some(99_999_999));

    cleanup_other(&fx, &other).await?;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn breakdown_by_model_for_user_only_includes_self() -> Result<()> {
    let fx = Fixture::new().await?;
    let other = make_other_user(&fx).await?;
    insert_request_with_context(&fx, &fx.context_id, None, "model-x", 1_000, 100, 0).await?;
    insert_request_with_context(&fx, &fx.context_id, None, "model-y", 5_000, 500, 1).await?;

    let repo = fx.repo()?;
    let rows = repo
        .get_breakdown_by_model_for_user(
            &UserId::new(&fx.user_id),
            fx.window_start,
            fx.window_end,
            10,
        )
        .await?;
    let names: std::collections::HashSet<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains("model-x"));
    assert!(names.contains("model-y"));
    assert!(
        !names.contains("other-model"),
        "must not leak rows from another user"
    );

    cleanup_other(&fx, &other).await?;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn context_summary_counts_distinct_contexts() -> Result<()> {
    let fx = Fixture::new().await?;
    let ctx2 = format!("test_ctx2_{}", fx.tag);
    sqlx::query("INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)")
        .bind(&ctx2)
        .bind(&fx.user_id)
        .bind("ctx2")
        .execute(&fx.pool)
        .await?;
    insert_request_with_context(&fx, &fx.context_id, None, "model-a", 100, 10, 0).await?;
    insert_request_with_context(&fx, &fx.context_id, None, "model-a", 100, 10, 1).await?;
    insert_request_with_context(&fx, &ctx2, None, "model-a", 100, 10, 2).await?;

    let repo = fx.repo()?;
    let summary = repo
        .get_context_summary_for_user(&UserId::new(&fx.user_id), fx.window_start, fx.window_end)
        .await?;
    assert_eq!(summary.conversations, 2);
    assert_eq!(summary.ai_requests, 3);

    sqlx::query("DELETE FROM user_contexts WHERE context_id = $1")
        .bind(&ctx2)
        .execute(&fx.pool)
        .await?;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn contexts_by_agent_groups_by_agent_name() -> Result<()> {
    let fx = Fixture::new().await?;
    let agent_a = format!("agent-a-{}", fx.tag);
    let agent_b = format!("agent-b-{}", fx.tag);
    let task_a = fx.insert_task(Some(&agent_a)).await?;
    let task_b = fx.insert_task(Some(&agent_b)).await?;
    insert_request_with_context(&fx, &fx.context_id, Some(&task_a), "m", 100, 10, 0).await?;
    insert_request_with_context(&fx, &fx.context_id, Some(&task_a), "m", 100, 10, 1).await?;
    insert_request_with_context(&fx, &fx.context_id, Some(&task_b), "m", 100, 10, 2).await?;

    let repo = fx.repo()?;
    let rows = repo
        .get_contexts_by_agent_for_user(
            &UserId::new(&fx.user_id),
            fx.window_start,
            fx.window_end,
            10,
        )
        .await?;
    let by_name: std::collections::HashMap<&str, &_> =
        rows.iter().map(|r| (r.name.as_str(), r)).collect();
    assert!(by_name.contains_key(agent_a.as_str()));
    assert!(by_name.contains_key(agent_b.as_str()));

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn empty_window_returns_no_rows() -> Result<()> {
    let fx = Fixture::new().await?;

    let repo = fx.repo()?;
    let breakdown = repo
        .get_breakdown_by_agent(fx.window_start, fx.window_end, 20)
        .await?;

    assert!(
        breakdown.is_empty(),
        "empty window must not surface a zero-cost unattributed row, got: {breakdown:?}"
    );

    fx.cleanup().await?;
    Ok(())
}
