//! Smoke integration tests covering the remaining repositories:
//! cli_sessions, content_analytics, core_stats, agents, tools, funnel,
//! and per-user / platform cost paths not exercised by `costs.rs`.

use anyhow::Result;
use chrono::{Duration, TimeZone, Utc};
use sqlx::PgPool;
use systemprompt_analytics::{
    AgentAnalyticsRepository, CliSessionAnalyticsRepository, ContentAnalyticsRepository,
    CoreStatsRepository, CostAnalyticsRepository, FunnelRepository, ToolAnalyticsRepository,
    ToolListParams,
};
use systemprompt_database::DbPool;
use systemprompt_models::UserId;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use tokio::sync::{Mutex, MutexGuard, OnceCell};
use uuid::Uuid;

static SERIAL: OnceCell<Mutex<()>> = OnceCell::const_new();

async fn acquire_serial() -> MutexGuard<'static, ()> {
    SERIAL
        .get_or_init(|| async { Mutex::new(()) })
        .await
        .lock()
        .await
}

#[allow(dead_code)]
struct Fixture {
    pool: PgPool,
    db: DbPool,
    user_id: String,
    user_typed: UserId,
    context_id: String,
    tag: String,
    window_start: chrono::DateTime<Utc>,
    window_end: chrono::DateTime<Utc>,
    _guard: MutexGuard<'static, ()>,
}

impl Fixture {
    async fn new() -> Result<Self> {
        let guard = acquire_serial().await;
        let url = fixture_database_url()?;
        let db = fixture_db_pool(&url).await?;
        let pool = db.pool_arc()?.as_ref().clone();
        let tag = Uuid::new_v4().simple().to_string();
        let user_id = format!("ext_u_{tag}");
        let context_id = format!("ext_c_{tag}");

        sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $2, $3)")
            .bind(&user_id)
            .bind(&user_id)
            .bind(format!("{user_id}@test.invalid"))
            .execute(&pool)
            .await?;

        let uuid = Uuid::new_v4();
        let offset_days = i64::from(u32::from_le_bytes(
            uuid.as_bytes()[0..4].try_into().unwrap(),
        ));
        let base = Utc.with_ymd_and_hms(2099, 9, 1, 0, 0, 0).unwrap();
        let window_start = base + Duration::days(offset_days % 1_000_000);
        let window_end = window_start + Duration::days(1);

        sqlx::query(
            "INSERT INTO user_contexts (context_id, user_id, name, created_at, updated_at) VALUES \
             ($1, $2, $3, $4, $4)",
        )
        .bind(&context_id)
        .bind(&user_id)
        .bind(format!("ctx-{tag}"))
        .bind(window_start + Duration::minutes(1))
        .execute(&pool)
        .await?;

        Ok(Self {
            user_typed: UserId::new(&user_id),
            pool,
            db,
            user_id,
            context_id,
            tag,
            window_start,
            window_end,
            _guard: guard,
        })
    }

    async fn insert_session(&self, session_id: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO user_sessions (session_id, user_id, started_at, last_activity_at, \
             country, device_type, browser, user_agent, landing_page, request_count, \
             duration_seconds) VALUES ($1, $2, $3, $3, 'US', 'desktop', 'chrome', 'ua', '/', 7, \
             900)",
        )
        .bind(session_id)
        .bind(&self.user_id)
        .bind(self.window_start + Duration::minutes(1))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn insert_task(&self, agent_name: &str) -> Result<String> {
        let task_id = format!("task_{}_{}", self.tag, Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO agent_tasks (task_id, context_id, agent_name, user_id, status, \
             started_at, execution_time_ms) VALUES ($1, $2, $3, $4, 'TASK_STATE_COMPLETED', $5, \
             250)",
        )
        .bind(&task_id)
        .bind(&self.context_id)
        .bind(agent_name)
        .bind(&self.user_id)
        .bind(self.window_start + Duration::minutes(2))
        .execute(&self.pool)
        .await?;
        Ok(task_id)
    }

    async fn insert_ai_request_for_task(&self, task_id: &str, cost: i64) -> Result<()> {
        let id = format!("req_{}_{}", self.tag, Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO ai_requests (id, request_id, user_id, task_id, provider, model, \
             cost_microdollars, tokens_used, status, created_at, updated_at, actor_kind, \
             actor_id, latency_ms) VALUES ($1, $2, $3, $4, 'prov', 'mod', $5, 100, 'completed', \
             $6, $6, 'user', $3, 50)",
        )
        .bind(&id)
        .bind(&id)
        .bind(&self.user_id)
        .bind(task_id)
        .bind(cost)
        .bind(self.window_start + Duration::minutes(3))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn insert_tool_execution(&self, tool_name: &str, status: &str) -> Result<()> {
        let id = format!("tx_{}_{}", self.tag, Uuid::new_v4().simple());
        let started = self.window_start + Duration::minutes(4);
        sqlx::query(
            "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, status, \
             input, started_at, created_at, user_id) VALUES ($1, $2, 'test-server', $3, '{}', $4, \
             $4, $5)",
        )
        .bind(&id)
        .bind(tool_name)
        .bind(status)
        .bind(started)
        .bind(&self.user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn cleanup(&self) -> Result<()> {
        let _ = sqlx::query("DELETE FROM mcp_tool_executions WHERE user_id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM ai_requests WHERE user_id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM agent_tasks WHERE user_id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM user_sessions WHERE user_id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM user_contexts WHERE user_id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await;
        Ok(())
    }
}

#[tokio::test]
async fn cli_session_repository_smoke() -> Result<()> {
    let fx = Fixture::new().await?;
    fx.insert_session(&format!("cli_s_{}_1", fx.tag)).await?;
    fx.insert_session(&format!("cli_s_{}_2", fx.tag)).await?;

    let repo = CliSessionAnalyticsRepository::new(&fx.db)?;
    let stats = repo.get_stats(fx.window_start, fx.window_end).await?;
    assert!(stats.total_sessions >= 2);
    let active_since = repo.get_active_session_count(fx.window_start).await?;
    assert!(active_since >= 0);
    let live = repo.get_live_sessions(fx.window_start, 50).await?;
    assert!(
        live.iter()
            .any(|s| s.session_id.as_str() == format!("cli_s_{}_1", fx.tag)),
        "seeded live session must surface within its window"
    );
    let active_count = repo.get_active_count(fx.window_start).await?;
    assert!(active_count >= 0);
    let trends = repo
        .get_sessions_for_trends(fx.window_start, fx.window_end)
        .await?;
    assert!(trends.len() >= 2);
    let active_since2 = repo.get_active_count_since(fx.window_start).await?;
    assert!(active_since2 >= 0);
    let total = repo.get_total_count(fx.window_start, fx.window_end).await?;
    assert!(total >= 2);

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn agent_repository_smoke() -> Result<()> {
    let fx = Fixture::new().await?;
    let agent = format!("agent-{}", fx.tag);
    let task_a = fx.insert_task(&agent).await?;
    let task_b = fx.insert_task(&agent).await?;
    fx.insert_ai_request_for_task(&task_a, 500).await?;
    fx.insert_ai_request_for_task(&task_b, 750).await?;

    let repo = AgentAnalyticsRepository::new(&fx.db)?;

    for order in ["", "success_rate", "cost", "last_active", "task_count"] {
        let listed = repo
            .list_agents(fx.window_start, fx.window_end, 10, order)
            .await?;
        assert!(listed.iter().any(|r| r.agent_name == agent));
    }

    let exists = repo
        .agent_exists(&agent, fx.window_start, fx.window_end)
        .await?;
    assert!(exists >= 2);
    let summary = repo
        .get_agent_summary(&agent, fx.window_start, fx.window_end)
        .await?;
    assert!(summary.total_tasks >= 2);
    let _breakdown = repo
        .get_status_breakdown(&agent, fx.window_start, fx.window_end)
        .await?;
    let _errors = repo
        .get_top_errors(&agent, fx.window_start, fx.window_end)
        .await?;
    let _hourly = repo
        .get_hourly_distribution(&agent, fx.window_start, fx.window_end)
        .await?;
    let stats = repo.get_stats(fx.window_start, fx.window_end, None).await?;
    assert!(stats.total_tasks >= 2);
    let _stats_f = repo
        .get_stats(fx.window_start, fx.window_end, Some(&agent))
        .await?;
    let _ai_stats = repo.get_ai_stats(fx.window_start, fx.window_end).await?;
    let _trends = repo
        .get_tasks_for_trends(fx.window_start, fx.window_end, None)
        .await?;
    let _trends_f = repo
        .get_tasks_for_trends(fx.window_start, fx.window_end, Some(&agent))
        .await?;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn tool_repository_smoke() -> Result<()> {
    let fx = Fixture::new().await?;
    let tool = format!("tool-{}", fx.tag);
    fx.insert_tool_execution(&tool, "success").await?;
    fx.insert_tool_execution(&tool, "failed").await?;

    let repo = ToolAnalyticsRepository::new(&fx.db)?;

    for order in ["call_count", "success_rate", "p95_latency", "unknown_sort"] {
        let listed = repo
            .list_tools(ToolListParams {
                start: fx.window_start,
                end: fx.window_end,
                limit: 10,
                server_filter: None,
                sort_order: order,
            })
            .await?;
        assert!(
            listed.iter().any(|r| r.tool_name == tool),
            "seeded tool must surface in list_tools with sort order {order:?}"
        );
    }

    let filtered = repo
        .list_tools(ToolListParams {
            start: fx.window_start,
            end: fx.window_end,
            limit: 10,
            server_filter: Some("test"),
            sort_order: "call_count",
        })
        .await?;
    assert!(filtered.iter().any(|r| r.tool_name == tool));

    let stats = repo.get_stats(fx.window_start, fx.window_end, None).await?;
    assert!(stats.total_executions >= 2);
    let _stats_f = repo
        .get_stats(fx.window_start, fx.window_end, Some("test"))
        .await?;
    let exists = repo
        .tool_exists(&tool, fx.window_start, fx.window_end)
        .await?;
    assert!(exists >= 2);
    let _summary = repo
        .get_tool_summary(&tool, fx.window_start, fx.window_end)
        .await?;
    let _status_break = repo
        .get_status_breakdown(&tool, fx.window_start, fx.window_end)
        .await?;
    let _top_errs = repo
        .get_top_errors(&tool, fx.window_start, fx.window_end)
        .await?;
    let _by_agent = repo
        .get_usage_by_agent(&tool, fx.window_start, fx.window_end)
        .await?;
    let _trend = repo
        .get_executions_for_trends(fx.window_start, fx.window_end, None)
        .await?;
    let _trend_f = repo
        .get_executions_for_trends(fx.window_start, fx.window_end, Some("tool"))
        .await?;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn core_stats_repository_smoke() -> Result<()> {
    let fx = Fixture::new().await?;
    fx.insert_session(&format!("cs_s_{}", fx.tag)).await?;
    fx.insert_ai_request_for_task(&fx.insert_task("agent-a").await?, 500)
        .await?;

    let repo = CoreStatsRepository::new(&fx.db)?;
    let _browsers = repo.get_browser_breakdown(10).await?;
    let _devices = repo.get_device_breakdown(10).await?;
    let _geo = repo.get_geographic_breakdown(10).await?;
    let _bots = repo.get_bot_traffic_stats().await?;
    let _top_users = repo.get_top_users(10).await?;
    let _top_agents = repo.get_top_agents(10).await?;
    let _top_tools = repo.get_top_tools(10).await?;
    let _trend = repo.get_activity_trend(7).await?;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn cost_repository_per_user_paths() -> Result<()> {
    let fx = Fixture::new().await?;
    let task_a = fx.insert_task("agent-a").await?;
    fx.insert_ai_request_for_task(&task_a, 1_000).await?;
    fx.insert_ai_request_for_task(&task_a, 2_000).await?;

    let repo = CostAnalyticsRepository::new(&fx.db)?;
    let summary = repo
        .get_summary_for_user(&fx.user_typed, fx.window_start, fx.window_end)
        .await?;
    assert_eq!(summary.cost, Some(3_000));
    let by_model = repo
        .get_breakdown_by_model_for_user(&fx.user_typed, fx.window_start, fx.window_end, 10)
        .await?;
    assert!(!by_model.is_empty());
    let by_provider = repo
        .get_breakdown_by_provider(fx.window_start, fx.window_end, 10)
        .await?;
    assert!(!by_provider.is_empty());
    let by_model_all = repo
        .get_breakdown_by_model(fx.window_start, fx.window_end, 10)
        .await?;
    assert!(!by_model_all.is_empty());
    let trends = repo
        .get_costs_for_trends(fx.window_start, fx.window_end)
        .await?;
    assert!(!trends.is_empty());
    let prev_cost = repo
        .get_previous_cost(fx.window_start - Duration::days(2), fx.window_start)
        .await?;
    let _ = prev_cost;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn content_analytics_repository_smoke() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = ContentAnalyticsRepository::new(&fx.db)?;
    let _top = repo
        .get_top_content(fx.window_start, fx.window_end, 10)
        .await?;
    let _stats = repo.get_stats(fx.window_start, fx.window_end).await?;
    let _trend = repo
        .get_content_for_trends(fx.window_start, fx.window_end)
        .await?;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn funnel_repository_finder_smoke() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = FunnelRepository::new(&fx.db)?;
    // Empty-state queries: just exercise the SELECT paths
    let _active = repo.list_active().await?;
    let _all = repo.list_all().await?;
    let missing = repo
        .find_by_name(&format!("does-not-exist-{}", fx.tag))
        .await?;
    assert!(missing.is_none());

    fx.cleanup().await?;
    Ok(())
}
