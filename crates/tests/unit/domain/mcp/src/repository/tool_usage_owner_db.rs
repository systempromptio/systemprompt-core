//! `ToolUsageRepository::find_visible` reads an execution through its owner:
//! another user's execution answers exactly as an unknown id, and the
//! unscoped read (an admin's) still sees it.

use chrono::Utc;
use serde_json::json;
use systemprompt_identifiers::{
    AgentName, ContextId, McpExecutionId, McpServerId, McpToolName, SessionId, TraceId, UserId,
};
use systemprompt_mcp::models::{McpToolExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::ExecutionSource;
use systemprompt_test_fixtures::test_db_pool;

async fn logged_by(repo: &ToolUsageRepository, owner: &UserId) -> McpExecutionId {
    let server_name = format!("owner-srv-{}", uuid::Uuid::new_v4().simple());
    let ctx = RequestContext::new(
        SessionId::new("owner-s"),
        TraceId::new("owner-t"),
        ContextId::generate(),
        AgentName::try_new("owner-agent").expect("valid AgentName"),
        systemprompt_identifiers::Actor::user(owner.clone()),
    );
    let started_at = Utc::now();
    let request = ToolExecutionRequest {
        tool_name: McpToolName::new("owner-tool"),
        server_name: McpServerId::new(server_name.as_str()),
        input: json!({}),
        started_at,
        context: ctx,
        request_method: Some("mcp".to_owned()),
        request_source: Some(server_name.clone()),
        ai_tool_call_id: None,
        source: ExecutionSource::InProcess,
    };
    let result = ToolExecutionResult {
        output: Some(json!({"ok": true})),
        output_schema: None,
        status: McpToolExecutionStatus::Success,
        error_message: None,
        started_at,
        completed_at: Some(Utc::now()),
    };
    repo.log_execution_sync(&request, &result)
        .await
        .expect("log")
}

#[tokio::test]
async fn a_foreign_execution_answers_as_an_unknown_one() {
    let db = test_db_pool().await;
    let repo = ToolUsageRepository::new(&db);
    let owner = UserId::new(format!("exec-owner-{}", uuid::Uuid::new_v4().simple()));
    let stranger = UserId::new(format!("exec-stranger-{}", uuid::Uuid::new_v4().simple()));
    let id = logged_by(&repo, &owner).await;

    let own = repo
        .find_visible(&id, Some(&owner))
        .await
        .expect("own read");
    assert_eq!(own.expect("own execution").user_id, owner);

    let foreign = repo
        .find_visible(&id, Some(&stranger))
        .await
        .expect("foreign read");
    let unknown_id = McpExecutionId::new(format!("none-{}", uuid::Uuid::new_v4().simple()));
    let unknown = repo
        .find_visible(&unknown_id, Some(&stranger))
        .await
        .expect("unknown read");
    assert!(foreign.is_none());
    assert_eq!(foreign.is_none(), unknown.is_none());

    assert!(
        repo.find_visible(&id, None)
            .await
            .expect("admin read")
            .is_some()
    );
}
