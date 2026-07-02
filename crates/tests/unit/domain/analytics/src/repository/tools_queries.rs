//! DB-backed tests for `ToolAnalyticsRepository`: the leaderboard listing in
//! all sort/filter combinations, and the per-tool detail queries (stats,
//! summary, status breakdown, top errors, agent usage, trend series). Each
//! test seeds `mcp_tool_executions` rows under a unique tool/server name so
//! the ILIKE filters isolate its own data.

use chrono::{DateTime, Duration, Utc};
use systemprompt_analytics::{ToolAnalyticsRepository, ToolListParams};
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

struct ExecutionSeed<'a> {
    tool_name: &'a str,
    server_name: &'a str,
    status: &'a str,
    execution_time_ms: i32,
    error_message: Option<&'a str>,
}

async fn insert_execution(pool: &DbPool, seed: ExecutionSeed<'_>) {
    let p = pool.write_pool_arc().expect("write pool");
    sqlx::query(
        r"
        INSERT INTO mcp_tool_executions
            (mcp_execution_id, tool_name, server_name, started_at, completed_at,
             execution_time_ms, input, status, error_message, user_id)
        VALUES ($1, $2, $3, NOW(), NOW(), $4, '{}', $5, $6, 'anon')
        ",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(seed.tool_name)
    .bind(seed.server_name)
    .bind(seed.execution_time_ms)
    .bind(seed.status)
    .bind(seed.error_message)
    .execute(p.as_ref())
    .await
    .expect("insert mcp_tool_execution");
}

async fn cleanup(pool: &DbPool, prefix: &str) {
    let p = pool.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM mcp_tool_executions WHERE tool_name LIKE $1")
        .bind(format!("{prefix}%"))
        .execute(p.as_ref())
        .await
        .ok();
}

fn window() -> (DateTime<Utc>, DateTime<Utc>) {
    (
        Utc::now() - Duration::hours(1),
        Utc::now() + Duration::hours(1),
    )
}

async fn seed_two_tools(pool: &DbPool, prefix: &str, server: &str) {
    let fast = format!("{prefix}-fast");
    let slow = format!("{prefix}-slow");
    insert_execution(
        pool,
        ExecutionSeed {
            tool_name: &fast,
            server_name: server,
            status: "success",
            execution_time_ms: 10,
            error_message: None,
        },
    )
    .await;
    insert_execution(
        pool,
        ExecutionSeed {
            tool_name: &fast,
            server_name: server,
            status: "success",
            execution_time_ms: 20,
            error_message: None,
        },
    )
    .await;
    insert_execution(
        pool,
        ExecutionSeed {
            tool_name: &slow,
            server_name: server,
            status: "failed",
            execution_time_ms: 500,
            error_message: Some("boom"),
        },
    )
    .await;
}

#[tokio::test]
async fn list_tools_filtered_covers_all_sort_orders() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ToolAnalyticsRepository::new(&pool).expect("repo");

    let prefix = format!("tool-{}", Uuid::new_v4());
    let server = format!("srv-{}", Uuid::new_v4());
    seed_two_tools(&pool, &prefix, &server).await;
    let (start, end) = window();

    for sort_order in ["count", "success_rate", "avg_time"] {
        let rows = repo
            .list_tools(ToolListParams {
                start,
                end,
                limit: 50,
                server_filter: Some(&server),
                sort_order,
            })
            .await
            .expect("list filtered");
        assert_eq!(rows.len(), 2, "sort_order={sort_order}");
        assert!(rows.iter().all(|r| r.server_name == server));
    }

    let by_count = repo
        .list_tools(ToolListParams {
            start,
            end,
            limit: 50,
            server_filter: Some(&server),
            sort_order: "count",
        })
        .await
        .expect("list by count");
    assert_eq!(by_count[0].tool_name, format!("{prefix}-fast"));
    assert_eq!(by_count[0].execution_count, 2);
    assert_eq!(by_count[0].success_count, 2);

    let by_avg_time = repo
        .list_tools(ToolListParams {
            start,
            end,
            limit: 50,
            server_filter: Some(&server),
            sort_order: "avg_time",
        })
        .await
        .expect("list by avg time");
    assert_eq!(by_avg_time[0].tool_name, format!("{prefix}-slow"));

    cleanup(&pool, &prefix).await;
}

#[tokio::test]
async fn list_tools_unfiltered_covers_all_sort_orders() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ToolAnalyticsRepository::new(&pool).expect("repo");

    let prefix = format!("tool-{}", Uuid::new_v4());
    let server = format!("srv-{}", Uuid::new_v4());
    seed_two_tools(&pool, &prefix, &server).await;
    let (start, end) = window();

    for sort_order in ["count", "success_rate", "avg_time"] {
        let rows = repo
            .list_tools(ToolListParams {
                start,
                end,
                limit: 10_000,
                server_filter: None,
                sort_order,
            })
            .await
            .expect("list unfiltered");
        let mine: Vec<_> = rows
            .iter()
            .filter(|r| r.tool_name.starts_with(&prefix))
            .collect();
        assert_eq!(mine.len(), 2, "sort_order={sort_order}");
    }

    cleanup(&pool, &prefix).await;
}

#[tokio::test]
async fn get_stats_and_summary_report_seeded_executions() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ToolAnalyticsRepository::new(&pool).expect("repo");

    let prefix = format!("tool-{}", Uuid::new_v4());
    let server = format!("srv-{}", Uuid::new_v4());
    seed_two_tools(&pool, &prefix, &server).await;
    let (start, end) = window();

    let filtered = repo
        .get_stats(start, end, Some(&prefix))
        .await
        .expect("filtered stats");
    assert_eq!(filtered.total_tools, 2);
    assert_eq!(filtered.total_executions, 3);
    assert_eq!(filtered.successful, 2);
    assert_eq!(filtered.failed, 1);
    assert_eq!(filtered.timeout, 0);
    assert!(filtered.avg_time > 0.0);

    let unfiltered = repo
        .get_stats(start, end, None)
        .await
        .expect("unfiltered stats");
    assert!(unfiltered.total_executions >= 3);

    let exists = repo
        .tool_exists(&prefix, start, end)
        .await
        .expect("tool exists");
    assert_eq!(exists, 3);

    let summary = repo
        .get_tool_summary(&format!("{prefix}-fast"), start, end)
        .await
        .expect("summary");
    assert_eq!(summary.total, 2);
    assert_eq!(summary.successful, 2);
    assert!((summary.avg_time - 15.0).abs() < f64::EPSILON);

    cleanup(&pool, &prefix).await;
}

#[tokio::test]
async fn detail_queries_break_down_status_errors_and_agents() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = ToolAnalyticsRepository::new(&pool).expect("repo");

    let prefix = format!("tool-{}", Uuid::new_v4());
    let server = format!("srv-{}", Uuid::new_v4());
    seed_two_tools(&pool, &prefix, &server).await;
    let (start, end) = window();

    let breakdown = repo
        .get_status_breakdown(&prefix, start, end)
        .await
        .expect("breakdown");
    assert_eq!(breakdown.len(), 2);

    let errors = repo
        .get_top_errors(&format!("{prefix}-slow"), start, end)
        .await
        .expect("errors");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].error_msg.as_deref(), Some("boom"));
    assert_eq!(errors[0].error_count, 1);

    let agents = repo
        .get_usage_by_agent(&prefix, start, end)
        .await
        .expect("agents");
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].agent_name.as_deref(), Some("Direct Call"));
    assert_eq!(agents[0].usage_count, 3);

    let trends_filtered = repo
        .get_executions_for_trends(start, end, Some(&prefix))
        .await
        .expect("trends filtered");
    assert_eq!(trends_filtered.len(), 3);

    let trends_all = repo
        .get_executions_for_trends(start, end, None)
        .await
        .expect("trends unfiltered");
    assert!(trends_all.len() >= 3);

    cleanup(&pool, &prefix).await;
}
