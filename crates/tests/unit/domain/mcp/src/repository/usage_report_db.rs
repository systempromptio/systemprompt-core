//! DB-backed tests for `ToolUsageReportRepository`: the leaderboard listing in
//! all sort/filter combinations, and the per-tool detail queries (stats,
//! summary, status breakdown, top errors, trend series, overview totals).
//! Each test seeds `mcp_tool_executions` rows under a unique tool/server name
//! so the ILIKE filters isolate its own data.

use chrono::{DateTime, Duration, Utc};
use systemprompt_database::DbPool;
use systemprompt_mcp::repository::ToolUsageReportRepository;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use systemprompt_traits::usage_reports::mcp::{ToolListParams, ToolListSort, ToolUsageReport};
use uuid::Uuid;

struct ExecutionSeed<'a> {
    tool_name: &'a str,
    server_name: &'a str,
    status: &'a str,
    execution_time_ms: i32,
    error_message: Option<&'a str>,
}

async fn insert_execution(pool: &DbPool, seed: ExecutionSeed<'_>) {
    let user = systemprompt_identifiers::UserId::new(format!("tool-user-{}", seed.tool_name));
    systemprompt_test_fixtures::seed_user_row(
        pool,
        &user,
        &format!("{}@tool-test.invalid", user.as_str()),
    )
    .await
    .expect("retained tool user");
    let p = pool.write_pool();
    sqlx::query(
        r"
        INSERT INTO mcp_tool_executions
            (mcp_execution_id, tool_name, server_name, started_at, completed_at,
             execution_time_ms, input, status, error_message, user_id)
        VALUES ($1, $2, $3, NOW(), NOW(), $4, '{}', $5, $6, $7)
        ",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(seed.tool_name)
    .bind(seed.server_name)
    .bind(seed.execution_time_ms)
    .bind(seed.status)
    .bind(seed.error_message)
    .bind(user.as_str())
    .execute(p.as_ref())
    .await
    .expect("insert mcp_tool_execution");
}

async fn cleanup(pool: &DbPool, prefix: &str) {
    let p = pool.write_pool();
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
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = ToolUsageReportRepository::new(&pool);

    let prefix = format!("tool-{}", Uuid::new_v4());
    let server = format!("srv-{}", Uuid::new_v4());
    seed_two_tools(&pool, &prefix, &server).await;
    let (start, end) = window();

    for sort in [
        ToolListSort::ExecutionCount,
        ToolListSort::SuccessRate,
        ToolListSort::AvgTime,
    ] {
        let rows = repo
            .list_tools(ToolListParams {
                start,
                end,
                limit: 50,
                server_filter: Some(&server),
                sort,
            })
            .await
            .expect("list filtered");
        assert_eq!(rows.len(), 2, "sort={sort:?}");
        assert!(rows.iter().all(|r| r.server_name.as_str() == server));
    }

    let by_count = repo
        .list_tools(ToolListParams {
            start,
            end,
            limit: 50,
            server_filter: Some(&server),
            sort: ToolListSort::ExecutionCount,
        })
        .await
        .expect("list by count");
    assert_eq!(by_count[0].tool_name.as_str(), format!("{prefix}-fast"));
    assert_eq!(by_count[0].execution_count, 2);
    assert_eq!(by_count[0].success_count, 2);

    let by_avg_time = repo
        .list_tools(ToolListParams {
            start,
            end,
            limit: 50,
            server_filter: Some(&server),
            sort: ToolListSort::AvgTime,
        })
        .await
        .expect("list by avg time");
    assert_eq!(by_avg_time[0].tool_name.as_str(), format!("{prefix}-slow"));

    cleanup(&pool, &prefix).await;
}

#[tokio::test]
async fn list_tools_unfiltered_covers_all_sort_orders() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = ToolUsageReportRepository::new(&pool);

    let prefix = format!("tool-{}", Uuid::new_v4());
    let server = format!("srv-{}", Uuid::new_v4());
    seed_two_tools(&pool, &prefix, &server).await;
    let (start, end) = window();

    for sort in [
        ToolListSort::ExecutionCount,
        ToolListSort::SuccessRate,
        ToolListSort::AvgTime,
    ] {
        let rows = repo
            .list_tools(ToolListParams {
                start,
                end,
                limit: 10_000,
                server_filter: None,
                sort,
            })
            .await
            .expect("list unfiltered");
        let mine: Vec<_> = rows
            .iter()
            .filter(|r| r.tool_name.as_str().starts_with(&prefix))
            .collect();
        assert_eq!(mine.len(), 2, "sort={sort:?}");
    }

    cleanup(&pool, &prefix).await;
}

#[tokio::test]
async fn get_stats_and_summary_report_seeded_executions() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = ToolUsageReportRepository::new(&pool);

    let prefix = format!("tool-{}", Uuid::new_v4());
    let server = format!("srv-{}", Uuid::new_v4());
    seed_two_tools(&pool, &prefix, &server).await;
    let (start, end) = window();

    let filtered = repo
        .stats(start, end, Some(&prefix))
        .await
        .expect("filtered stats");
    assert_eq!(filtered.total_tools, 2);
    assert_eq!(filtered.total_executions, 3);
    assert_eq!(filtered.successful, 2);
    assert_eq!(filtered.failed, 1);
    assert_eq!(filtered.timeout, 0);
    assert!(filtered.avg_time > 0.0);

    let unfiltered = repo
        .stats(start, end, None)
        .await
        .expect("unfiltered stats");
    assert!(unfiltered.total_executions >= 3);

    let exists = repo
        .tool_execution_count(&prefix, start, end)
        .await
        .expect("tool exists");
    assert_eq!(exists, 3);

    let summary = repo
        .tool_summary(&format!("{prefix}-fast"), start, end)
        .await
        .expect("summary");
    assert_eq!(summary.total, 2);
    assert_eq!(summary.successful, 2);
    assert!((summary.avg_time - 15.0).abs() < f64::EPSILON);

    cleanup(&pool, &prefix).await;
}

#[tokio::test]
async fn detail_queries_break_down_status_and_errors() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = ToolUsageReportRepository::new(&pool);

    let prefix = format!("tool-{}", Uuid::new_v4());
    let server = format!("srv-{}", Uuid::new_v4());
    seed_two_tools(&pool, &prefix, &server).await;
    let (start, end) = window();

    let breakdown = repo
        .status_breakdown(&prefix, start, end)
        .await
        .expect("breakdown");
    assert_eq!(breakdown.len(), 2);

    let errors = repo
        .top_errors(&format!("{prefix}-slow"), start, end)
        .await
        .expect("errors");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].error_msg.as_deref(), Some("boom"));
    assert_eq!(errors[0].error_count, 1);

    let trends_filtered = repo
        .executions_for_trends(start, end, Some(&prefix))
        .await
        .expect("trends filtered");
    assert_eq!(trends_filtered.len(), 3);

    let trends_all = repo
        .executions_for_trends(start, end, None)
        .await
        .expect("trends unfiltered");
    assert!(trends_all.len() >= 3);

    let overview = repo
        .overview_tool_metrics(start, end)
        .await
        .expect("overview metrics");
    assert!(overview.total >= 3);
    assert!(overview.successful >= 2);

    cleanup(&pool, &prefix).await;
}
