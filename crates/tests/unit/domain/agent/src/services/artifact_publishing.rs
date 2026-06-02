// DB-backed tests for ArtifactPublishingService: publishing A2A + MCP
// artifacts, execution-id FK validation (unknown id is nulled), and the
// direct-vs-agentic message-creation branch.

use systemprompt_agent::models::a2a::{Artifact, ArtifactMetadata, Part, TextPart};
use systemprompt_agent::repository::content::ArtifactRepository;
use systemprompt_agent::services::artifact_publishing::{
    ArtifactPublishingService, PublishFromMcpParams,
};
use systemprompt_identifiers::{
    Actor, AgentName, ArtifactId, ContextId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::execution::CallSource;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_test_fixtures::ensure_test_bootstrap;

use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

async fn publishing_service(
    pool: &systemprompt_database::DbPool,
) -> ArtifactPublishingService {
    ensure_test_bootstrap();
    let _skills = crate::SKILLS_FIXTURE_LOCK.read().await;
    ArtifactPublishingService::new(pool).expect("publishing service")
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
        AgentName::new("pub-agent"),
    );
    rc.auth.actor = Actor::user(user.clone());
    rc
}

#[tokio::test]
async fn publish_from_a2a_persists_artifact() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = publishing_service(&pool).await;
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, tid) = seed_context_and_task(&r, &user_id, &session_id).await;

    let id = ArtifactId::generate();
    let art = artifact(&id, &ctx, &tid, None);
    svc.publish_from_a2a(&art, &tid, &ctx)
        .await
        .expect("publish a2a");

    let repo = ArtifactRepository::new(r.db_pool()).expect("artifact repo");
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
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = publishing_service(&pool).await;
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, tid) = seed_context_and_task(&r, &user_id, &session_id).await;

    let id = ArtifactId::generate();
    // This execution id does not exist in mcp_tool_executions, so it is nulled.
    let art = artifact(&id, &ctx, &tid, Some("nonexistent-exec-id"));
    svc.publish_from_a2a(&art, &tid, &ctx)
        .await
        .expect("publish");

    let repo = ArtifactRepository::new(r.db_pool()).expect("artifact repo");
    let fetched = repo
        .get_artifact_by_id(&id)
        .await
        .expect("get")
        .expect("present");
    assert!(fetched.metadata.mcp_execution_id.is_none());

    r.tasks.delete_task(&tid).await.ok();
}

#[tokio::test]
async fn publish_from_mcp_agentic_skips_messages() {
    let Some(pool) = try_pool().await else {
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
    let messages = r
        .tasks
        .get_messages_by_task(&tid)
        .await
        .expect("messages");
    assert!(messages.is_empty());

    r.tasks.delete_task(&tid).await.ok();
}

#[tokio::test]
async fn publish_from_mcp_direct_creates_messages() {
    let Some(pool) = try_pool().await else {
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
    let messages = r
        .tasks
        .get_messages_by_task(&tid)
        .await
        .expect("messages");
    assert_eq!(messages.len(), 2);

    r.tasks.delete_task(&tid).await.ok();
}

#[tokio::test]
async fn debug_format() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = publishing_service(&pool).await;
    let dbg = format!("{svc:?}");
    assert!(dbg.contains("ArtifactPublishingService"));
}
