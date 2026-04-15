use anyhow::Result;
use chrono::{Duration, TimeZone, Utc};
use std::env;
use std::sync::Arc;
use systemprompt_analytics::CostAnalyticsRepository;
use systemprompt_database::{Database, DbPool};
use uuid::Uuid;

struct Fixture {
    pool: sqlx::PgPool,
    db: DbPool,
    user_id: String,
    context_id: String,
    window_start: chrono::DateTime<Utc>,
    window_end: chrono::DateTime<Utc>,
    tag: String,
}

impl Fixture {
    async fn new() -> Result<Self> {
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

        sqlx::query(
            "INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)",
        )
        .bind(&context_id)
        .bind(&user_id)
        .bind(format!("ctx-{tag}"))
        .execute(&pool)
        .await?;

        let window_start = Utc.with_ymd_and_hms(2099, 1, 1, 0, 0, 0).unwrap();
        let window_end = window_start + Duration::days(1);

        Ok(Self {
            pool,
            db,
            user_id,
            context_id,
            window_start,
            window_end,
            tag,
        })
    }

    async fn insert_task(&self, agent_name: Option<&str>) -> Result<String> {
        let task_id = format!("task_{}_{}", self.tag, Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO agent_tasks (task_id, context_id, agent_name, user_id) \
             VALUES ($1, $2, $3, $4)",
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
            "INSERT INTO ai_requests \
             (id, request_id, user_id, task_id, provider, model, \
              cost_microdollars, tokens_used, status, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, 'test-provider', 'test-model', \
                     $5, $6, 'completed', $7, $7)",
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
        CostAnalyticsRepository::new(&self.db)
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
        fx.insert_ai_request(Some(&task_a), 1_000 * (i + 1), 100, i).await?;
    }
    for i in 0..5 {
        fx.insert_ai_request(Some(&task_b), 500 * (i + 1), 50, i + 5).await?;
    }

    let repo = fx.repo()?;
    let summary = repo.get_summary(fx.window_start, fx.window_end).await?;
    let breakdown = repo
        .get_breakdown_by_agent(fx.window_start, fx.window_end, 20)
        .await?;

    let breakdown_sum: i64 = breakdown.iter().map(|r| r.cost).sum();
    let expected_total: i64 = (1_000 * (1 + 2 + 3 + 4 + 5)) + (500 * (1 + 2 + 3 + 4 + 5));

    assert_eq!(summary.total_cost, Some(expected_total));
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

    assert_eq!(summary.total_cost, Some(expected_total));
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
    let attributed_count = breakdown.iter().filter(|r| r.name != "unattributed").count();
    assert_eq!(attributed_count, 2, "LIMIT should bound attributed rows to 2");

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
