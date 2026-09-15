//! DB-backed tests for [`ToolUsageRepository`].
//!
//! Most queries return empty datasets when invoked with random IDs. We use
//! the per-track Postgres database to exercise the read-only branches and the
//! `list_tool_stats` aggregator.

use systemprompt_identifiers::{AiToolCallId, ContextId, McpExecutionId};
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};
use systemprompt_traits::ToolExecutionLookup;

async fn db_or_skip() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

#[tokio::test]
async fn repository_new_succeeds() {
    let Some(db) = db_or_skip().await else { return };
    drop(ToolUsageRepository::new(&db).expect("ctor"));
}

#[tokio::test]
async fn find_by_id_random_returns_none() {
    let Some(db) = db_or_skip().await else { return };
    let repo = ToolUsageRepository::new(&db).unwrap();
    let id = McpExecutionId::new(format!("none-{}", uuid::Uuid::new_v4().simple()));
    let r = repo.find_by_id(&id).await.unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn find_by_ai_call_id_random_returns_none() {
    let Some(db) = db_or_skip().await else { return };
    let repo = ToolUsageRepository::new(&db).unwrap();
    let id = AiToolCallId::new(format!("none-{}", uuid::Uuid::new_v4().simple()));
    let r = repo.find_by_ai_call_id(&id).await.unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn execution_exists_answers_through_the_shared_lookup_seam() {
    use chrono::Utc;
    use serde_json::json;
    use systemprompt_identifiers::{AgentName, SessionId, TraceId, UserId};
    use systemprompt_mcp::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
    use systemprompt_models::RequestContext;

    let Some(db) = db_or_skip().await else { return };
    let repo = ToolUsageRepository::new(&db).unwrap();

    let tool_name = format!("stats-tool-{}", uuid::Uuid::new_v4().simple());
    let server_name = format!("stats-srv-{}", uuid::Uuid::new_v4().simple());
    let ctx = RequestContext::new(
        SessionId::new("stats-s"),
        TraceId::new("stats-t"),
        ContextId::generate(),
        AgentName::try_new("stats-agent").expect("valid AgentName"),
    )
    .with_actor(systemprompt_identifiers::Actor::user(UserId::new(
        "stats-u",
    )));

    let started_at = Utc::now();
    let request = ToolExecutionRequest {
        tool_name: tool_name.clone(),
        server_name: server_name.clone(),
        input: json!({}),
        started_at,
        context: ctx,
        request_method: Some("mcp".to_owned()),
        request_source: Some(server_name.clone()),
        ai_tool_call_id: None,
    };
    let result = ToolExecutionResult {
        output: Some(json!({"ok": true})),
        output_schema: None,
        status: ExecutionStatus::Success.as_str().to_owned(),
        error_message: None,
        started_at,
        completed_at: Utc::now(),
    };
    let execution_id = repo.log_execution_sync(&request, &result).await.unwrap();

    let lookup: &dyn ToolExecutionLookup = &repo;
    assert!(
        lookup.execution_exists(&execution_id).await.unwrap(),
        "a logged execution is visible through the seam"
    );
    let unknown = McpExecutionId::new(format!("none-{}", uuid::Uuid::new_v4().simple()));
    assert!(!lookup.execution_exists(&unknown).await.unwrap());
}

#[tokio::test]
async fn start_and_complete_execution_roundtrip() {
    use chrono::Utc;
    use serde_json::json;
    use systemprompt_identifiers::{AgentName, SessionId, TraceId, UserId};
    use systemprompt_mcp::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
    use systemprompt_models::RequestContext;

    let Some(db) = db_or_skip().await else { return };
    let repo = ToolUsageRepository::new(&db).unwrap();
    let ctx = RequestContext::new(
        SessionId::new("s1"),
        TraceId::new("t1"),
        ContextId::generate(),
        AgentName::try_new("test-agent").expect("valid AgentName"),
    )
    .with_actor(systemprompt_identifiers::Actor::user(UserId::new(
        "test-user",
    )));

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

    let fetched = repo
        .find_by_id(&exec_id)
        .await
        .unwrap()
        .expect("started execution is retrievable");
    assert_eq!(fetched.tool_name, "tool-x");
    assert_eq!(fetched.server_name, "srv-x");

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

    let (actor_kind, actor_id, request_method, request_source): (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT actor_kind, actor_id, request_method, request_source \
         FROM mcp_tool_executions WHERE mcp_execution_id = $1",
    )
    .bind(exec_id.as_str())
    .fetch_one(&*db.write_pool_arc().unwrap())
    .await
    .unwrap();
    assert_eq!(actor_kind.as_deref(), Some("user"));
    assert_eq!(actor_id.as_deref(), Some("test-user"));
    assert_eq!(request_method.as_deref(), Some("mcp"));
    assert_eq!(request_source.as_deref(), Some("srv-x"));
}

#[tokio::test]
async fn log_execution_sync_writes_row() {
    use chrono::Utc;
    use serde_json::json;
    use systemprompt_identifiers::{AgentName, SessionId, TraceId, UserId};
    use systemprompt_mcp::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
    use systemprompt_models::RequestContext;

    let Some(db) = db_or_skip().await else { return };
    let repo = ToolUsageRepository::new(&db).unwrap();
    let ctx = RequestContext::new(
        SessionId::new("s2"),
        TraceId::new("t2"),
        ContextId::generate(),
        AgentName::try_new("agent-sync").expect("valid AgentName"),
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
