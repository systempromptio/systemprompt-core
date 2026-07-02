//! DB-backed tests for [`TraceQueryService`] and [`AiTraceService`].
//!
//! Most query methods return empty datasets when invoked with random
//! trace/request ids. We exercise them on a fresh per-track DB to drive line
//! coverage through the (zero-result) happy paths, plus a handful of filter
//! combinations.

use chrono::{Duration as ChronoDuration, Utc};
use systemprompt_identifiers::{AiRequestId, TraceId};
use systemprompt_logging::trace::{
    AiRequestFilter, LogSearchFilter, ToolExecutionFilter, TraceListFilter,
};
use systemprompt_logging::{AiTraceService, TraceQueryService};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn pool_arc() -> Option<std::sync::Arc<sqlx::PgPool>> {
    let url = fixture_database_url().ok()?;
    let db = fixture_db_pool(&url).await.ok()?;
    db.pool_arc().ok()
}

#[tokio::test]
async fn trace_service_get_methods_on_empty_trace_id() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);
    let trace_id = TraceId::new(format!("no-such-{}", uuid::Uuid::new_v4().simple()));

    let _ = svc.get_log_events(&trace_id).await.unwrap();
    let _ = svc.get_ai_request_summary(&trace_id).await.unwrap();
    let _ = svc.get_ai_request_events(&trace_id).await.unwrap();
    let _ = svc.get_mcp_execution_summary(&trace_id).await.unwrap();
    let _ = svc.get_mcp_execution_events(&trace_id).await.unwrap();
    let _ = svc.get_task_id(&trace_id).await.unwrap();
    let _ = svc.get_execution_step_summary(&trace_id).await.unwrap();
    let _ = svc.get_execution_step_events(&trace_id).await.unwrap();
    let _ = svc.get_all_trace_data(&trace_id).await.unwrap();
    let _ = svc.find_logs_by_trace_id(&trace_id).await.unwrap();
}

#[tokio::test]
async fn trace_service_list_traces_various_filters() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);

    let f = TraceListFilter::new(5);
    let _ = svc.list_traces(&f).await.unwrap();

    let f = TraceListFilter::new(5)
        .with_since(Utc::now() - ChronoDuration::days(7))
        .with_has_mcp(true)
        .with_include_system(true);
    let _ = svc.list_traces(&f).await.unwrap();

    let f = TraceListFilter::new(5)
        .with_agent("agent-x".to_owned())
        .with_status("completed".to_owned())
        .with_tool("tool-y".to_owned());
    let _ = svc.list_traces(&f).await.unwrap();
}

#[tokio::test]
async fn trace_service_list_tool_executions() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);
    let f = ToolExecutionFilter::new(10);
    let _ = svc.list_tool_executions(&f).await.unwrap();
    let f = ToolExecutionFilter::new(10)
        .with_name("tool-x".to_owned())
        .with_server("srv".to_owned())
        .with_status("ok".to_owned())
        .with_since(Utc::now() - ChronoDuration::hours(1));
    let _ = svc.list_tool_executions(&f).await.unwrap();
}

#[tokio::test]
async fn trace_service_search_logs_branches() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);

    let _ = svc.search_logs("nothing", None, None, 10).await.unwrap();
    let _ = svc
        .search_logs(
            "nothing",
            Some(Utc::now() - ChronoDuration::hours(1)),
            Some("INFO"),
            10,
        )
        .await
        .unwrap();
    let _ = svc
        .search_tool_executions("nothing", None, 10)
        .await
        .unwrap();
    let _ = svc
        .search_tool_executions("nothing", Some(Utc::now() - ChronoDuration::hours(1)), 5)
        .await
        .unwrap();
}

#[tokio::test]
async fn trace_service_ai_request_branches() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);

    let _ = svc.list_ai_requests(None, None, None, 10).await.unwrap();
    let _ = svc
        .list_ai_requests(
            Some(Utc::now() - ChronoDuration::hours(1)),
            Some("gpt-4"),
            Some("openai"),
            5,
        )
        .await
        .unwrap();
    let _ = svc.get_ai_request_stats(None).await.unwrap();
    let _ = svc
        .get_ai_request_stats(Some(Utc::now() - ChronoDuration::days(1)))
        .await
        .unwrap();
    let _ = svc.find_ai_request_detail("no-such").await.unwrap();
    let _ = svc.find_ai_request_for_audit("no-such").await.unwrap();
    let _ = svc
        .list_audit_messages(&AiRequestId::new("no-such"))
        .await
        .unwrap();
    let _ = svc
        .list_audit_tool_calls(&AiRequestId::new("no-such"))
        .await
        .unwrap();
    let _ = svc
        .list_linked_mcp_calls(&AiRequestId::new("no-such"))
        .await
        .unwrap();
}

#[tokio::test]
async fn trace_service_log_lookup_branches() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);

    let _ = svc.find_log_by_id("no-such").await.unwrap();
    let _ = svc.find_log_by_partial_id("abc").await.unwrap();
    let _ = svc.list_logs_filtered(None, None, 10).await.unwrap();
    let _ = svc
        .list_logs_filtered(Some(Utc::now() - ChronoDuration::days(1)), Some("ERROR"), 5)
        .await
        .unwrap();
}

#[tokio::test]
async fn trace_service_log_summary_branches() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);

    let _ = svc.count_logs_by_level(None).await.unwrap();
    let _ = svc
        .count_logs_by_level(Some(Utc::now() - ChronoDuration::days(1)))
        .await
        .unwrap();
    let _ = svc.top_modules(None, 5).await.unwrap();
    let _ = svc
        .top_modules(Some(Utc::now() - ChronoDuration::days(1)), 5)
        .await
        .unwrap();
    let _ = svc.log_time_range(None).await.unwrap();
    let _ = svc
        .log_time_range(Some(Utc::now() - ChronoDuration::days(1)))
        .await
        .unwrap();
    let _ = svc.total_log_count().await.unwrap();
}

#[tokio::test]
async fn ai_trace_service_methods_with_random_ids() {
    let Some(pool) = pool_arc().await else { return };
    let svc = AiTraceService::new(pool);
    let task_id =
        systemprompt_identifiers::TaskId::new(format!("task-{}", uuid::Uuid::new_v4().simple()));
    let ctx_id = systemprompt_identifiers::ContextId::generate();
    let _ = svc.get_task_info(&task_id).await;
    let _ = svc.get_user_input(&task_id).await.unwrap();
    let _ = svc.get_agent_response(&task_id).await.unwrap();
    let _ = svc.get_execution_steps(&task_id).await.unwrap();
    let _ = svc.get_ai_requests(&task_id).await.unwrap();
    let _ = svc
        .get_system_prompt(&AiRequestId::new("no-such"))
        .await
        .unwrap();
    let _ = svc
        .get_conversation_messages(&AiRequestId::new("no-such"))
        .await
        .unwrap();
    let _ = svc.get_mcp_executions(&task_id, &ctx_id).await.unwrap();
    let _ = svc.resolve_task_id("no-such-prefix").await;
}

#[test]
fn filter_value_types() {
    let _ = ToolExecutionFilter::new(10);
    let _ = AiRequestFilter::new(10)
        .with_model("m".to_owned())
        .with_provider("p".to_owned());
    let _ = LogSearchFilter::new("p".to_owned(), 10)
        .with_level("WARN".to_owned())
        .with_since(Utc::now() - ChronoDuration::hours(1));
}

async fn insert_ai_request(pool: &sqlx::PgPool, trace_id: &str, status: &str) {
    let id = uuid::Uuid::new_v4().simple().to_string();
    sqlx::query!(
        "INSERT INTO ai_requests \
         (id, request_id, user_id, provider, model, actor_kind, actor_id, trace_id, status) \
         VALUES ($1, $2, 'u', 'test', 'test', 'user', 'u', $3, $4)",
        id.as_str(),
        id.as_str(),
        trace_id,
        status,
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_mcp_execution(pool: &sqlx::PgPool, trace_id: &str, status: &str) {
    let id = uuid::Uuid::new_v4().simple().to_string();
    sqlx::query!(
        "INSERT INTO mcp_tool_executions \
         (mcp_execution_id, tool_name, server_name, started_at, input, user_id, trace_id, status) \
         VALUES ($1, 't', 's', now(), '{}', 'u', $2, $3)",
        id.as_str(),
        trace_id,
        status,
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_log(pool: &sqlx::PgPool, trace_id: &str, level: &str) {
    sqlx::query!(
        "INSERT INTO logs (level, module, message, trace_id, user_id, session_id) \
         VALUES ($1, 'm', 'msg', $2, 'test-user', 'test-session')",
        level,
        trace_id,
    )
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn list_traces_derives_status_for_non_agent_traces() {
    let Some(pool) = pool_arc().await else { return };

    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let tid = |label: &str| format!("t-{label}-{suffix}");

    let ai_completed = tid("ai-done");
    let ai_failed = tid("ai-fail");
    let ai_pending = tid("ai-run");
    let mcp_ok = tid("mcp-ok");
    let mcp_timeout = tid("mcp-timeout");
    let log_err = tid("log-err");
    let log_info = tid("log-info");

    insert_ai_request(&pool, &ai_completed, "completed").await;
    insert_ai_request(&pool, &ai_failed, "failed").await;
    insert_ai_request(&pool, &ai_pending, "pending").await;
    insert_mcp_execution(&pool, &mcp_ok, "success").await;
    insert_mcp_execution(&pool, &mcp_timeout, "timeout").await;
    insert_log(&pool, &log_err, "ERROR").await;
    insert_log(&pool, &log_info, "INFO").await;

    let svc = TraceQueryService::new(std::sync::Arc::clone(&pool));
    let items = svc.list_traces(&TraceListFilter::new(1000)).await.unwrap();

    let by_id: std::collections::HashMap<String, String> = items
        .into_iter()
        .map(|i| (i.trace_id.as_str().to_owned(), i.status))
        .collect();

    let status_of = |id: &str| by_id.get(id).map(String::as_str);
    assert_eq!(status_of(&ai_completed), Some("completed"));
    assert_eq!(status_of(&ai_failed), Some("failed"));
    assert_eq!(status_of(&ai_pending), Some("running"));
    assert_eq!(status_of(&mcp_ok), Some("completed"));
    assert_eq!(status_of(&mcp_timeout), Some("failed"));
    assert_eq!(status_of(&log_err), Some("failed"));
    assert_eq!(status_of(&log_info), Some("completed"));

    assert!(
        !by_id.values().any(|s| s == "unknown"),
        "trace status must never fall back to the 'unknown' sentinel"
    );
}
