//! DB-backed tests for [`McpArtifactRepository`].
//!
//! Every artifact belongs to an execution, so each test records one first
//! through [`ToolUsageRepository`] and hangs the artifact off it.

use chrono::Utc;
use systemprompt_identifiers::{
    Actor, AgentName, AiToolCallId, ArtifactId, ContextId, McpExecutionId, SessionId, TraceId,
    UserId,
};
use systemprompt_mcp::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
use systemprompt_mcp::repository::{
    ArtifactCorrelation, ArtifactShape, CreateMcpArtifact, McpArtifactRepository,
    ToolUsageRepository,
};
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::ExecutionSource;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

async fn db_or_skip() -> Option<systemprompt_database::DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

pub async fn seed_execution(db: &systemprompt_database::DbPool, server: &str) -> McpExecutionId {
    let ctx = RequestContext::new(
        SessionId::new(unique("sess")),
        TraceId::new(unique("trace")),
        ContextId::generate(),
        AgentName::try_new("artifact-tests").expect("valid AgentName"),
    )
    .with_actor(Actor::user(UserId::new(
        "11111111-1111-4111-8111-111111111abc",
    )));
    let started_at = Utc::now();
    let request = ToolExecutionRequest {
        tool_name: "seed_tool".to_owned(),
        server_name: server.to_owned(),
        input: serde_json::json!({}),
        started_at,
        context: ctx,
        request_method: Some("mcp".to_owned()),
        request_source: Some(server.to_owned()),
        ai_tool_call_id: None,
        source: ExecutionSource::InProcess,
    };
    let result = ToolExecutionResult {
        output: None,
        output_schema: None,
        status: ExecutionStatus::Success.as_str().to_owned(),
        error_message: None,
        started_at,
        completed_at: Some(Utc::now()),
    };
    ToolUsageRepository::new(db)
        .expect("tool usage repo")
        .log_execution_sync(&request, &result)
        .await
        .expect("seed execution")
}

async fn full_artifact(
    db: &systemprompt_database::DbPool,
    id: &ArtifactId,
    server: &str,
) -> CreateMcpArtifact {
    let exec = seed_execution(db, server).await;
    let mut create = CreateMcpArtifact::new(
        id.clone(),
        exec,
        server,
        "document",
        serde_json::json!({"body": "hello"}),
    );
    create.context_id =
        Some(ContextId::try_new("00000000-0000-4000-8000-000000000abc").expect("valid ContextId"));
    create.user_id = Some(UserId::new("11111111-1111-4111-8111-111111111abc"));
    create.title = Some("Report".to_owned());
    create.metadata = Some(serde_json::json!({"k": "v"}));
    create.shape = ArtifactShape {
        is_structured: true,
        ..ArtifactShape::default()
    };
    create
}

#[tokio::test]
async fn repository_new_succeeds() {
    let Some(db) = db_or_skip().await else { return };
    drop(McpArtifactRepository::new(&db).expect("ctor"));
}

#[tokio::test]
async fn find_by_id_random_returns_none() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(unique("art"));
    assert!(repo.find_by_id(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn list_by_server_returns_vec() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let r = repo.list_by_server(&unique("none"), 10).await.unwrap();
    assert!(r.is_empty());
}

#[tokio::test]
async fn delete_random_returns_false() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    assert!(!repo.delete(&ArtifactId::new(unique("art"))).await.unwrap());
}

#[tokio::test]
async fn cleanup_expired_reaps_a_past_due_artifact() {
    use chrono::Duration;

    let Some(db) = db_or_skip().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(unique("art"));
    let mut create = full_artifact(&db, &id, "art-cleanup").await;
    create.expires_at = Some(Utc::now() - Duration::hours(1));
    repo.save(&create).await.unwrap();

    let reaped = repo.cleanup_expired().await.unwrap();
    assert!(
        reaped >= 1,
        "cleanup_expired deletes at least the seeded past-due artifact"
    );
    assert!(repo.find_by_id(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn save_then_find_round_trips_all_fields() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(unique("art"));
    let server = unique("srv");
    let create = full_artifact(&db, &id, &server).await;
    repo.save(&create).await.unwrap();

    let found = repo
        .find_by_id(&id)
        .await
        .unwrap()
        .expect("saved artifact is found");
    assert_eq!(found.artifact_id, id);
    assert_eq!(found.mcp_execution_id, create.mcp_execution_id);
    assert_eq!(found.server_name, server);
    assert_eq!(found.artifact_type, "document");
    assert_eq!(found.title.as_deref(), Some("Report"));
    assert_eq!(found.data, serde_json::json!({"body": "hello"}));
    assert_eq!(found.metadata, Some(serde_json::json!({"k": "v"})));
    assert_eq!(found.source(), ExecutionSource::InProcess);
    assert!(found.is_structured);
    assert!(found.expires_at.is_none());
}

#[tokio::test]
async fn save_on_conflict_updates_mutable_fields() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(unique("art"));
    let server = unique("srv");
    let create = full_artifact(&db, &id, &server).await;
    repo.save(&create).await.unwrap();

    let mut updated = create.clone();
    updated.title = Some("Revised".to_owned());
    updated.data = serde_json::json!({"body": "world"});
    updated.metadata = Some(serde_json::json!({"k": "v2"}));
    repo.save(&updated).await.unwrap();

    let found = repo.find_by_id(&id).await.unwrap().expect("still present");
    assert_eq!(found.title.as_deref(), Some("Revised"));
    assert_eq!(found.data, serde_json::json!({"body": "world"}));
    assert_eq!(found.metadata, Some(serde_json::json!({"k": "v2"})));
}

#[tokio::test]
async fn one_artifact_per_execution_is_enforced() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let server = unique("srv");
    let first = full_artifact(&db, &ArtifactId::new(unique("art")), &server).await;
    repo.save(&first).await.unwrap();

    let mut second = first.clone();
    second.artifact_id = ArtifactId::new(unique("art"));
    assert!(
        repo.save(&second).await.is_err(),
        "a second artifact for the same execution is refused"
    );
}

#[tokio::test]
async fn find_by_execution_and_ai_tool_call_id() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(unique("art"));
    let call = AiToolCallId::new(unique("toolu"));
    let mut create = full_artifact(&db, &id, &unique("srv")).await;
    create.ai_tool_call_id = Some(call.clone());
    repo.save(&create).await.unwrap();

    let by_exec = repo
        .find_by_execution_id(&create.mcp_execution_id)
        .await
        .unwrap()
        .expect("found by execution");
    assert_eq!(by_exec.artifact_id, id);
    let by_call = repo
        .find_by_ai_tool_call_id(&call)
        .await
        .unwrap()
        .expect("found by client call id");
    assert_eq!(by_call.artifact_id, id);
}

#[tokio::test]
async fn enrich_correlation_fills_only_missing_keys() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(unique("art"));
    let mut create = full_artifact(&db, &id, &unique("srv")).await;
    create.session_id = Some(SessionId::new("original-session"));
    repo.save(&create).await.unwrap();

    let call = AiToolCallId::new(unique("toolu"));
    repo.enrich_correlation(
        &id,
        &ArtifactCorrelation {
            session_id: Some(SessionId::new("later-session")),
            trace_id: Some(TraceId::new("later-trace")),
            ai_tool_call_id: Some(call.clone()),
            last_seen_source: Some(ExecutionSource::HookClaudeCode),
        },
    )
    .await
    .unwrap();

    let found = repo.find_by_id(&id).await.unwrap().expect("present");
    assert_eq!(
        found.session_id.as_ref().map(SessionId::as_str),
        Some("original-session")
    );
    assert_eq!(
        found.trace_id.as_ref().map(TraceId::as_str),
        Some("later-trace")
    );
    assert_eq!(found.ai_tool_call_id, Some(call));
    assert_eq!(found.last_seen_source.as_deref(), Some("hook_claude_code"));
    assert_eq!(found.source(), ExecutionSource::InProcess);
}

#[tokio::test]
async fn list_by_server_returns_saved_rows() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let server = unique("srv");
    let id_a = ArtifactId::new(unique("art"));
    let id_b = ArtifactId::new(unique("art"));
    repo.save(&full_artifact(&db, &id_a, &server).await)
        .await
        .unwrap();
    repo.save(&full_artifact(&db, &id_b, &server).await)
        .await
        .unwrap();

    let rows = repo.list_by_server(&server, 10).await.unwrap();
    assert_eq!(rows.len(), 2);
    let ids: Vec<&ArtifactId> = rows.iter().map(|r| &r.artifact_id).collect();
    assert!(ids.contains(&&id_a));
    assert!(ids.contains(&&id_b));
}

#[tokio::test]
async fn delete_returns_true_for_existing_artifact() {
    let Some(db) = db_or_skip().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(unique("art"));
    repo.save(&full_artifact(&db, &id, &unique("srv")).await)
        .await
        .unwrap();

    assert!(repo.delete(&id).await.unwrap());
    assert!(repo.find_by_id(&id).await.unwrap().is_none());
}

#[tokio::test]
async fn find_by_id_hides_expired_artifact() {
    use chrono::Duration;

    let Some(db) = db_or_skip().await else { return };
    let repo = McpArtifactRepository::new(&db).unwrap();
    let id = ArtifactId::new(unique("art"));
    let mut create = full_artifact(&db, &id, &unique("srv")).await;
    create.expires_at = Some(Utc::now() - Duration::hours(1));
    repo.save(&create).await.unwrap();

    assert!(repo.find_by_id(&id).await.unwrap().is_none());
}
