//! DB-backed tests for how `ToolUsageReportRepository` treats the execution
//! source: a tool call a host's hook reports against an MCP server is counted
//! like any other, and a host's built-in tool, recorded with the source as its
//! server name, is left out of the tool list and the overview totals.

use chrono::{DateTime, Duration, Utc};
use systemprompt_database::DbPool;
use systemprompt_mcp::repository::ToolUsageReportRepository;
use systemprompt_models::mcp::ExecutionSource;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};
use systemprompt_traits::usage_reports::mcp::{ToolListParams, ToolListSort, ToolUsageReport};
use uuid::Uuid;

async fn insert_execution(pool: &DbPool, tool_name: &str, server_name: &str, source: &str) {
    let user = systemprompt_identifiers::UserId::new(format!("tool-user-{tool_name}"));
    systemprompt_test_fixtures::seed_user_row(
        pool,
        &user,
        &format!("{}@tool-test.invalid", user.as_str()),
    )
    .await
    .expect("retained tool user");
    sqlx::query(
        r"
        INSERT INTO mcp_tool_executions
            (mcp_execution_id, tool_name, server_name, started_at, completed_at,
             execution_time_ms, input, status, user_id, source)
        VALUES ($1, $2, $3, NOW(), NOW(), 10, '{}', 'success', $4, $5)
        ",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(tool_name)
    .bind(server_name)
    .bind(user.as_str())
    .bind(source)
    .execute(pool.write_pool().as_ref())
    .await
    .expect("insert mcp_tool_execution");
}

async fn cleanup(pool: &DbPool, prefix: &str) {
    sqlx::query("DELETE FROM mcp_tool_executions WHERE tool_name LIKE $1")
        .bind(format!("{prefix}%"))
        .execute(pool.write_pool().as_ref())
        .await
        .expect("cleanup");
}

fn window() -> (DateTime<Utc>, DateTime<Utc>) {
    (
        Utc::now() - Duration::hours(1),
        Utc::now() + Duration::hours(1),
    )
}

#[tokio::test]
async fn a_harness_reported_mcp_tool_call_is_counted() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = ToolUsageReportRepository::new(&pool);

    let prefix = format!("tool-{}", Uuid::new_v4());
    let server = format!("srv-{}", Uuid::new_v4());
    let tool = format!("{prefix}-search");
    let (start, end) = window();
    let before = repo
        .overview_tool_metrics(start, end)
        .await
        .expect("overview before");

    insert_execution(&pool, &tool, &server, ExecutionSource::HookHarness.as_str()).await;
    insert_execution(&pool, &tool, &server, ExecutionSource::HookHarness.as_str()).await;

    let rows = repo
        .list_tools(ToolListParams {
            start,
            end,
            limit: 50,
            server_filter: Some(&server),
            sort: ToolListSort::ExecutionCount,
        })
        .await
        .expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tool_name.as_str(), tool);
    assert_eq!(rows[0].execution_count, 2);
    assert_eq!(rows[0].success_count, 2);

    let after = repo
        .overview_tool_metrics(start, end)
        .await
        .expect("overview after");
    assert!(
        after.total >= before.total + 2,
        "the overview counts both harness-reported calls: {} then {}",
        before.total,
        after.total
    );

    cleanup(&pool, &prefix).await;
}

#[tokio::test]
async fn a_built_in_tool_of_any_source_is_left_out_of_the_tool_list() {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let repo = ToolUsageReportRepository::new(&pool);

    let prefix = format!("tool-{}", Uuid::new_v4());
    for source in ExecutionSource::ALL {
        insert_execution(
            &pool,
            &format!("{prefix}-{source}"),
            source.as_str(),
            source.as_str(),
        )
        .await;
    }
    let (start, end) = window();

    let recorded = repo
        .tool_execution_count(&prefix, start, end)
        .await
        .expect("recorded count");
    assert_eq!(
        usize::try_from(recorded).expect("count"),
        ExecutionSource::ALL.len(),
        "every built-in call reached the rollup the list reads"
    );

    for source in ExecutionSource::ALL {
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
                    server_filter: Some(source.as_str()),
                    sort,
                })
                .await
                .expect("list filtered");
            let mine: Vec<_> = rows
                .iter()
                .filter(|r| r.tool_name.as_str().starts_with(&prefix))
                .collect();
            assert!(
                mine.is_empty(),
                "{source} built-ins listed as MCP tools (sort={sort:?}): {}",
                mine.len()
            );
        }
    }

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
        assert!(
            rows.iter()
                .all(|r| !r.tool_name.as_str().starts_with(&prefix)),
            "a built-in tool is listed unfiltered (sort={sort:?})"
        );
    }

    cleanup(&pool, &prefix).await;
}
