//! DB-backed tests for [`ToolUsageRepository`].
//!
//! Most queries return empty datasets when invoked with random IDs. We use
//! the per-track Postgres database to exercise the read-only branches and the
//! `list_tool_stats` aggregator.

use systemprompt_identifiers::{AiToolCallId, ContextId, McpExecutionId};
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn repository_new_succeeds() {
    let Some(db) = db().await else { return };
    let _ = ToolUsageRepository::new(&db).expect("ctor");
}

#[tokio::test]
async fn find_by_id_random_returns_none() {
    let Some(db) = db().await else { return };
    let repo = ToolUsageRepository::new(&db).unwrap();
    let id = McpExecutionId::new(format!("none-{}", uuid::Uuid::new_v4().simple()));
    let r = repo.find_by_id(&id).await.unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn find_by_ai_call_id_random_returns_none() {
    let Some(db) = db().await else { return };
    let repo = ToolUsageRepository::new(&db).unwrap();
    let id = AiToolCallId::new(format!("none-{}", uuid::Uuid::new_v4().simple()));
    let r = repo.find_by_ai_call_id(&id).await.unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn find_context_id_random_returns_none() {
    let Some(db) = db().await else { return };
    let repo = ToolUsageRepository::new(&db).unwrap();
    let id = McpExecutionId::new(format!("none-{}", uuid::Uuid::new_v4().simple()));
    let r = repo.find_context_id(&id).await.unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn list_tool_stats_returns_vec() {
    let Some(db) = db().await else { return };
    let repo = ToolUsageRepository::new(&db).unwrap();
    let r = repo.list_tool_stats(10).await.unwrap();
    let _ = r.len();
}

#[tokio::test]
async fn update_context_timestamp_on_missing_context_does_not_panic() {
    let Some(db) = db().await else { return };
    let repo = ToolUsageRepository::new(&db).unwrap();
    let ctx = ContextId::generate();
    repo.update_context_timestamp(&ctx).await.unwrap();
}

#[tokio::test]
async fn start_and_complete_execution_roundtrip() {
    use chrono::Utc;
    use serde_json::json;
    use systemprompt_identifiers::{AgentName, SessionId, TraceId, UserId};
    use systemprompt_mcp::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
    use systemprompt_models::RequestContext;

    let Some(db) = db().await else { return };
    let repo = ToolUsageRepository::new(&db).unwrap();
    let ctx = RequestContext::new(
        SessionId::new("s1"),
        TraceId::new("t1"),
        ContextId::generate(),
        AgentName::new("test-agent"),
    )
    .with_actor(systemprompt_identifiers::Actor::user(UserId::new("test-user")));

    let started_at = Utc::now();
    let request = ToolExecutionRequest {
        tool_name: "tool-x".to_owned(),
        server_name: "srv-x".to_owned(),
        input: json!({"a":1}),
        started_at,
        context: ctx,
        request_method: Some("mcp".to_owned()),
        request_source: Some("srv-x".to_owned()),
        ai_tool_call_id: None,
    };

    let exec_id = repo.start_execution(&request).await.unwrap();

    let fetched = repo.find_by_id(&exec_id).await.unwrap();
    assert!(fetched.is_some());

    let completed_at = Utc::now();
    let result = ToolExecutionResult {
        output: Some(json!({"ok":true})),
        output_schema: None,
        status: ExecutionStatus::Success.as_str().to_owned(),
        error_message: None,
        started_at,
        completed_at,
    };
    repo.complete_execution(&exec_id, &result).await.unwrap();

    let final_row = repo.find_by_id(&exec_id).await.unwrap().unwrap();
    assert_eq!(final_row.status, "success");
}

#[tokio::test]
async fn log_execution_sync_writes_row() {
    use chrono::Utc;
    use serde_json::json;
    use systemprompt_identifiers::{AgentName, SessionId, TraceId, UserId};
    use systemprompt_mcp::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
    use systemprompt_models::RequestContext;

    let Some(db) = db().await else { return };
    let repo = ToolUsageRepository::new(&db).unwrap();
    let ctx = RequestContext::new(
        SessionId::new("s2"),
        TraceId::new("t2"),
        ContextId::generate(),
        AgentName::new("agent-sync"),
    )
    .with_actor(systemprompt_identifiers::Actor::user(UserId::new("u2")));

    let started_at = Utc::now();
    let request = ToolExecutionRequest {
        tool_name: "sync-tool".to_owned(),
        server_name: "sync-srv".to_owned(),
        input: json!({}),
        started_at,
        context: ctx,
        request_method: Some("mcp".to_owned()),
        request_source: Some("sync-srv".to_owned()),
        ai_tool_call_id: None,
    };
    let result = ToolExecutionResult {
        output: None,
        output_schema: None,
        status: ExecutionStatus::Failed.as_str().to_owned(),
        error_message: Some("boom".to_owned()),
        started_at,
        completed_at: Utc::now(),
    };
    let id = repo.log_execution_sync(&request, &result).await.unwrap();
    let row = repo.find_by_id(&id).await.unwrap().unwrap();
    assert_eq!(row.error_message.as_deref(), Some("boom"));
}
