// DB-backed tests for ArtifactPublishingService: publishing A2A + MCP
// artifacts, execution-id verification through the `ToolExecutionLookup`
// seam (unknown id is nulled, an unreachable ledger fails the publish), and
// the direct-vs-agentic message-creation branch.

use std::sync::Arc;

use systemprompt_agent::models::a2a::{Artifact, ArtifactMetadata, Part, TextPart};
use systemprompt_agent::repository::A2ARepositories;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::services::SkillService;
use systemprompt_agent::services::artifact_publishing::{
    ArtifactPublishingService, PublishFromMcpParams,
};
use systemprompt_identifiers::{
    Actor, AgentName, ArtifactId, ContextId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::execution::CallSource;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_test_fixtures::{
    ToolExecutionLedger, a2a_dependencies, ensure_test_bootstrap, not_managed_skills,
    tool_execution_ledger,
};
use systemprompt_test_mocks::recording_webhooks;

use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip};

async fn publishing_service(pool: &systemprompt_database::DbPool) -> ArtifactPublishingService {
    service_with_ledger(pool, ToolExecutionLedger::Absent).await
}

async fn service_with_ledger(
    pool: &systemprompt_database::DbPool,
    ledger: ToolExecutionLedger,
) -> ArtifactPublishingService {
    ensure_test_bootstrap();
    let _skills = crate::SKILLS_FIXTURE_LOCK.read().await;
    let mut deps = a2a_dependencies(pool);
    deps.tool_executions = tool_execution_ledger(ledger);
    let repositories = A2ARepositories::new(pool, deps).expect("repositories");
    let steps = Arc::new(ExecutionStepRepository::new(pool).expect("step repo"));
    let skills = Arc::new(
        SkillService::new(not_managed_skills(), steps, recording_webhooks()).expect("skills"),
    );
    ArtifactPublishingService::new(&repositories, skills)
}

fn artifact(
    id: &ArtifactId,
    ctx: &ContextId,
    tid: &TaskId,
    mcp_execution_id: Option<&str>,
) -> Artifact {
    let mut metadata = ArtifactMetadata::new("text".to_owned(), ctx.clone(), tid.clone());
    if let Some(exec) = mcp_execution_id {
        metadata = metadata.with_mcp_execution_id(exec.to_owned());
    }
    Artifact {
        id: id.clone(),
        title: Some("pub-artifact".to_owned()),
        description: None,
        parts: vec![Part::Text(TextPart {
            text: "body".to_owned(),
        })],
        extensions: vec![],
        metadata,
    }
}

fn request_context(ctx: &ContextId, session: &SessionId, user: &UserId) -> RequestContext {
    let mut rc = RequestContext::new(
        session.clone(),
        TraceId::generate(),
        ctx.clone(),
        AgentName::try_new("pub-agent").expect("valid AgentName"),
    );
    rc.auth.actor = Actor::user(user.clone());
    rc
}

#[tokio::test]
async fn publish_from_a2a_persists_artifact() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let svc = publishing_service(&pool).await;
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, tid) = seed_context_and_task(&r, &user_id, &session_id).await;

    let id = ArtifactId::generate();
    let art = artifact(&id, &ctx, &tid, None);
    svc.publish_from_a2a(&art, &tid, &ctx, &user_id)
        .await
        .expect("publish a2a");

    let repo = r.artifacts.clone();
    let fetched = repo
        .get_artifact_by_id(&id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(fetched.id, id);

    r.tasks.delete_task(&tid).await.ok();
}

#[tokio::test]
async fn publish_from_a2a_nulls_unknown_execution_id() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let svc = publishing_service(&pool).await;
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, tid) = seed_context_and_task(&r, &user_id, &session_id).await;

    let id = ArtifactId::generate();
    // The ledger answers "absent" for this execution id, so it is nulled.
    let art = artifact(&id, &ctx, &tid, Some("nonexistent-exec-id"));
    svc.publish_from_a2a(&art, &tid, &ctx, &user_id)
        .await
        .expect("publish");

    let repo = r.artifacts.clone();
    let fetched = repo
        .get_artifact_by_id(&id)
        .await
        .expect("get")
        .expect("present");
    assert!(fetched.metadata.mcp_execution_id.is_none());

    r.tasks.delete_task(&tid).await.ok();
}

#[tokio::test]
async fn publish_from_a2a_keeps_a_known_execution_id() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let svc = service_with_ledger(&pool, ToolExecutionLedger::Exists).await;
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, tid) = seed_context_and_task(&r, &user_id, &session_id).await;

    let exec_id = format!("exec-{}", uuid::Uuid::new_v4().simple());
    let sqlx_pool = pool.pool_arc().expect("sqlx pool");
    sqlx::query(
        "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, started_at, \
         input, user_id) VALUES ($1, 'echo', 'test-server', NOW(), '{}', $2)",
    )
    .bind(&exec_id)
    .bind(user_id.as_str())
    .execute(sqlx_pool.as_ref())
    .await
    .expect("seed execution row");

    let id = ArtifactId::generate();
    let art = artifact(&id, &ctx, &tid, Some(&exec_id));
    svc.publish_from_a2a(&art, &tid, &ctx, &user_id)
        .await
        .expect("publish");

    let fetched = r
        .artifacts
        .get_artifact_by_id(&id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(
        fetched.metadata.mcp_execution_id.as_deref(),
        Some(exec_id.as_str())
    );

    r.tasks.delete_task(&tid).await.ok();
}

#[tokio::test]
async fn an_unreachable_execution_ledger_fails_the_publish_and_keeps_the_id() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let svc = service_with_ledger(&pool, ToolExecutionLedger::Unavailable).await;
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, tid) = seed_context_and_task(&r, &user_id, &session_id).await;

    let id = ArtifactId::generate();
    let art = artifact(&id, &ctx, &tid, Some("exec-while-down"));
    let err = svc
        .publish_from_a2a(&art, &tid, &ctx, &user_id)
        .await
        .expect_err("an unreachable ledger is an error, not an unknown execution");
    assert!(err.to_string().contains("exec-while-down"), "{err}");

    let fetched = r.artifacts.get_artifact_by_id(&id).await.expect("get");
    assert!(
        fetched.is_none(),
        "nothing is persisted with a detached execution id"
    );

    r.tasks.delete_task(&tid).await.ok();
}

#[tokio::test]
async fn publish_from_mcp_agentic_skips_messages() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let svc = publishing_service(&pool).await;
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, tid) = seed_context_and_task(&r, &user_id, &session_id).await;
    let rc = request_context(&ctx, &session_id, &user_id);
    let args = serde_json::json!({"a": 1});

    let id = ArtifactId::generate();
    let art = artifact(&id, &ctx, &tid, None);
    svc.publish_from_mcp(PublishFromMcpParams {
        artifact: &art,
        task_id: &tid,
        context_id: &ctx,
        tool_name: "tool-x",
        tool_args: &args,
        request_context: &rc,
        call_source: CallSource::Agentic,
    })
    .await
    .expect("publish agentic");

    // Agentic path persists the artifact but does NOT create messages.
    let messages = r.tasks.get_messages_by_task(&tid).await.expect("messages");
    assert!(messages.is_empty());

    r.tasks.delete_task(&tid).await.ok();
}

#[tokio::test]
async fn publish_from_mcp_direct_creates_messages() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let svc = publishing_service(&pool).await;
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, tid) = seed_context_and_task(&r, &user_id, &session_id).await;
    let rc = request_context(&ctx, &session_id, &user_id);
    let args = serde_json::json!({"q": "x"});

    let id = ArtifactId::generate();
    let art = artifact(&id, &ctx, &tid, None);
    svc.publish_from_mcp(PublishFromMcpParams {
        artifact: &art,
        task_id: &tid,
        context_id: &ctx,
        tool_name: "tool-direct",
        tool_args: &args,
        request_context: &rc,
        call_source: CallSource::Direct,
    })
    .await
    .expect("publish direct");

    // Direct path creates a synthetic user message + an agent response message.
    let messages = r.tasks.get_messages_by_task(&tid).await.expect("messages");
    assert_eq!(messages.len(), 2);

    r.tasks.delete_task(&tid).await.ok();
}
