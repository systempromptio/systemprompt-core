//! DB-backed tests for [`TraceQueryService`] and [`AiTraceService`].
//!
//! Asserts the zero-result contract: query methods invoked with random
//! trace/request/task ids must return empty collections, `None`, or
//! zero-valued summaries — never rows belonging to other traces. Filter
//! combinations that cannot match seeded data must also come back empty.
//! Seeded-row mapping is covered in `seeded_queries.rs`.

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

fn nonexistent_tag() -> String {
    format!("no-such-{}", uuid::Uuid::new_v4().simple())
}

#[tokio::test]
async fn trace_service_get_methods_on_empty_trace_id() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);
    let trace_id = TraceId::new(nonexistent_tag());

    assert!(svc.get_log_events(&trace_id).await.unwrap().is_empty());
    let ai_summary = svc.get_ai_request_summary(&trace_id).await.unwrap();
    assert_eq!(ai_summary.request_count, 0);
    assert_eq!(ai_summary.total_tokens, 0);
    assert_eq!(ai_summary.total_cost_microdollars, 0);
    assert!(
        svc.get_ai_request_events(&trace_id)
            .await
            .unwrap()
            .is_empty()
    );
    let mcp_summary = svc.get_mcp_execution_summary(&trace_id).await.unwrap();
    assert_eq!(mcp_summary.execution_count, 0);
    assert_eq!(mcp_summary.total_execution_time_ms, 0);
    assert!(
        svc.get_mcp_execution_events(&trace_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(svc.get_task_id(&trace_id).await.unwrap(), None);
    let step_summary = svc.get_execution_step_summary(&trace_id).await.unwrap();
    assert_eq!(step_summary.total, 0);
    assert_eq!(step_summary.completed, 0);
    assert_eq!(step_summary.failed, 0);
    assert_eq!(step_summary.pending, 0);
    assert!(
        svc.get_execution_step_events(&trace_id)
            .await
            .unwrap()
            .is_empty()
    );
    let (logs, ai_events, mcp_events, step_events, ai_sum, mcp_sum, step_sum, task_id) =
        svc.get_all_trace_data(&trace_id).await.unwrap();
    assert!(logs.is_empty());
    assert!(ai_events.is_empty());
    assert!(mcp_events.is_empty());
    assert!(step_events.is_empty());
    assert_eq!(ai_sum.request_count, 0);
    assert_eq!(mcp_sum.execution_count, 0);
    assert_eq!(step_sum.total, 0);
    assert_eq!(task_id, None);
    assert!(
        svc.find_logs_by_trace_id(&trace_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn trace_service_list_traces_no_match_filters_yield_empty() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);

    let f = TraceListFilter::new(5)
        .with_agent(nonexistent_tag())
        .with_since(Utc::now() - ChronoDuration::days(7))
        .with_has_mcp(true)
        .with_include_system(true);
    assert!(svc.list_traces(&f).await.unwrap().is_empty());

    let f = TraceListFilter::new(5)
        .with_agent(nonexistent_tag())
        .with_status("completed".to_owned())
        .with_tool(nonexistent_tag());
    assert!(svc.list_traces(&f).await.unwrap().is_empty());
}

#[tokio::test]
async fn trace_service_list_tool_executions_no_match_filters_yield_empty() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);
    let f = ToolExecutionFilter::new(10)
        .with_name(nonexistent_tag())
        .with_server(nonexistent_tag())
        .with_status("ok".to_owned())
        .with_since(Utc::now() - ChronoDuration::hours(1));
    assert!(svc.list_tool_executions(&f).await.unwrap().is_empty());
}

#[tokio::test]
async fn trace_service_search_finds_nothing_for_random_pattern() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);
    let pattern = nonexistent_tag();

    assert!(
        svc.search_logs(&pattern, None, None, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        svc.search_logs(
            &pattern,
            Some(Utc::now() - ChronoDuration::hours(1)),
            Some("INFO"),
            10,
        )
        .await
        .unwrap()
        .is_empty()
    );
    assert!(
        svc.search_tool_executions(&pattern, None, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        svc.search_tool_executions(&pattern, Some(Utc::now() - ChronoDuration::hours(1)), 5)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn trace_service_ai_request_lookups_on_random_ids() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);
    let missing = nonexistent_tag();

    assert!(
        svc.list_ai_requests(None, Some(missing.as_str()), None, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        svc.list_ai_requests(
            Some(Utc::now() + ChronoDuration::days(1)),
            Some("gpt-4"),
            Some("openai"),
            5,
        )
        .await
        .unwrap()
        .is_empty()
    );
    let stats = svc
        .get_ai_request_stats(Some(Utc::now() + ChronoDuration::days(1)))
        .await
        .unwrap();
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.total_cost_microdollars, 0);
    assert!(stats.by_provider.is_empty());
    assert!(stats.by_model.is_empty());
    let unbounded = svc.get_ai_request_stats(None).await.unwrap();
    assert!(
        unbounded.total_requests >= stats.total_requests,
        "unbounded stats must cover at least the future-bounded window"
    );
    assert!(
        svc.find_ai_request_detail(&missing)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        svc.find_ai_request_for_audit(&missing)
            .await
            .unwrap()
            .is_none()
    );
    let request_id = AiRequestId::new(missing.as_str());
    assert!(
        svc.list_audit_messages(&request_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        svc.list_audit_tool_calls(&request_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        svc.list_linked_mcp_calls(&request_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn trace_service_log_lookups_on_random_ids() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);
    let missing = nonexistent_tag();

    assert!(svc.find_log_by_id(&missing).await.unwrap().is_none());
    assert!(
        svc.find_log_by_partial_id(&missing)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        svc.list_logs_filtered(Some(Utc::now() + ChronoDuration::days(1)), Some("ERROR"), 5)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        svc.list_logs_filtered(None, None, 10).await.unwrap().len() <= 10,
        "limit must be respected"
    );
}

#[tokio::test]
async fn trace_service_log_summaries_respect_future_since_bound() {
    let Some(pool) = pool_arc().await else { return };
    let svc = TraceQueryService::new(pool);
    let future = Utc::now() + ChronoDuration::days(1);

    assert!(
        svc.count_logs_by_level(Some(future))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(svc.top_modules(Some(future), 5).await.unwrap().is_empty());
    let range = svc.log_time_range(Some(future)).await.unwrap();
    assert_eq!(range.earliest, None);
    assert_eq!(range.latest, None);

    assert!(svc.total_log_count().await.unwrap() >= 0);
    assert!(
        svc.count_logs_by_level(None)
            .await
            .unwrap()
            .iter()
            .all(|c| c.count >= 0)
    );
    assert!(svc.top_modules(None, 5).await.unwrap().len() <= 5);
    let unbounded_range = svc.log_time_range(None).await.unwrap();
    assert_eq!(
        unbounded_range.earliest.is_some(),
        unbounded_range.latest.is_some()
    );
    if let (Some(earliest), Some(latest)) = (unbounded_range.earliest, unbounded_range.latest) {
        assert!(earliest <= latest);
    }
}

#[tokio::test]
async fn ai_trace_service_methods_with_random_ids() {
    let Some(pool) = pool_arc().await else { return };
    let svc = AiTraceService::new(pool);
    let task_id =
        systemprompt_identifiers::TaskId::new(format!("task-{}", uuid::Uuid::new_v4().simple()));
    let ctx_id = systemprompt_identifiers::ContextId::generate();

    assert!(svc.get_task_info(&task_id).await.is_err());
    assert_eq!(svc.get_user_input(&task_id).await.unwrap(), None);
    assert_eq!(svc.get_agent_response(&task_id).await.unwrap(), None);
    assert!(svc.get_execution_steps(&task_id).await.unwrap().is_empty());
    assert!(svc.get_ai_requests(&task_id).await.unwrap().is_empty());
    let request_id = AiRequestId::new(nonexistent_tag());
    assert_eq!(svc.get_system_prompt(&request_id).await.unwrap(), None);
    assert!(
        svc.get_conversation_messages(&request_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        svc.get_mcp_executions(&task_id, &ctx_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(svc.resolve_task_id(&nonexistent_tag()).await.is_err());
}

#[test]
fn filter_builders_retain_limits() {
    assert_eq!(ToolExecutionFilter::new(10).limit, 10);
    let ai = AiRequestFilter::new(10)
        .with_model("m".to_owned())
        .with_provider("p".to_owned());
    assert_eq!(ai.model.as_deref(), Some("m"));
    assert_eq!(ai.provider.as_deref(), Some("p"));
    let since = Utc::now() - ChronoDuration::hours(1);
    let log = LogSearchFilter::new("p".to_owned(), 10)
        .with_level("WARN".to_owned())
        .with_since(since);
    assert_eq!(log.level.as_deref(), Some("WARN"));
    assert_eq!(log.since, Some(since));
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
