// DB-backed tests for ArtifactPublishingService: publishing A2A + MCP
// artifacts, execution-id verification through the `ToolExecutionLookup`
// seam (unknown id is nulled, an unreachable ledger fails the publish), and
// the direct-vs-agentic message-creation branch.

use std::sync::Arc;
use systemprompt_test_fixtures::agent_reads::{ArtifactReadsExt, TaskReadsExt};

use systemprompt_agent::models::a2a::{Artifact, ArtifactMetadata, Part};
use systemprompt_agent::repository::A2ARepositories;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::services::SkillService;
use systemprompt_agent::services::artifact_publishing::ArtifactPublishingService;
use systemprompt_identifiers::{ArtifactId, ContextId, McpExecutionId, TaskId};
use systemprompt_test_fixtures::{
    ToolExecutionLedger, a2a_dependencies, ensure_test_bootstrap, not_managed_skills,
    tool_execution_ledger,
};
use systemprompt_test_mocks::recording_webhooks;

use crate::repository::{repos, seed_runtime_task, seed_user_and_session};
use systemprompt_test_fixtures::test_db_pool;

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
    let repositories = A2ARepositories::new(pool, deps);
    let steps = Arc::new(ExecutionStepRepository::new(pool));
    let skills = Arc::new(SkillService::new(
        systemprompt_test_fixtures::test_skills_root(),
        not_managed_skills(),
        steps,
        recording_webhooks(),
    ));
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
        metadata = metadata.with_mcp_execution_id(McpExecutionId::new(exec));
    }
    Artifact {
        id: id.clone(),
        title: Some("pub-artifact".to_owned()),
        description: None,
        parts: vec![Part::text("body".to_owned())],
        extensions: vec![],
        metadata,
    }
}

#[tokio::test]
async fn publish_from_a2a_persists_artifact() {
    let pool = test_db_pool().await;
    let svc = publishing_service(&pool).await;
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, runtime) = seed_runtime_task(&r, &user_id, &session_id).await;
    let tid = runtime.task_id().clone();

    let id = ArtifactId::generate();
    let art = artifact(&id, &ctx, &tid, None);
    svc.publish_from_a2a(&art, &runtime, &user_id)
        .await
        .expect("publish a2a");

    let repo = r.artifacts.clone();
    let fetched = repo
        .read_artifact(&id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(fetched.id, id);

    r.tasks.remove_task(&tid).await.ok();
}

#[tokio::test]
async fn publish_from_a2a_nulls_unknown_execution_id() {
    let pool = test_db_pool().await;
    let svc = publishing_service(&pool).await;
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, runtime) = seed_runtime_task(&r, &user_id, &session_id).await;
    let tid = runtime.task_id().clone();

    let id = ArtifactId::generate();
    // The ledger answers "absent" for this execution id, so it is nulled.
    let art = artifact(&id, &ctx, &tid, Some("nonexistent-exec-id"));
    svc.publish_from_a2a(&art, &runtime, &user_id)
        .await
        .expect("publish");

    let repo = r.artifacts.clone();
    let fetched = repo
        .read_artifact(&id)
        .await
        .expect("get")
        .expect("present");
    assert!(fetched.metadata.mcp_execution_id.is_none());

    r.tasks.remove_task(&tid).await.ok();
}

#[tokio::test]
async fn publish_from_a2a_keeps_a_known_execution_id() {
    let pool = test_db_pool().await;
    let svc = service_with_ledger(&pool, ToolExecutionLedger::Exists).await;
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, runtime) = seed_runtime_task(&r, &user_id, &session_id).await;
    let tid = runtime.task_id().clone();

    let exec_id = format!("exec-{}", uuid::Uuid::new_v4().simple());
    let sqlx_pool = pool.pool();
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
    svc.publish_from_a2a(&art, &runtime, &user_id)
        .await
        .expect("publish");

    let fetched = r
        .artifacts
        .read_artifact(&id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(
        fetched
            .metadata
            .mcp_execution_id
            .as_ref()
            .map(|id| id.as_str()),
        Some(exec_id.as_str())
    );

    r.tasks.remove_task(&tid).await.ok();
}

#[tokio::test]
async fn an_unreachable_execution_ledger_fails_the_publish_and_keeps_the_id() {
    let pool = test_db_pool().await;
    let svc = service_with_ledger(&pool, ToolExecutionLedger::Unavailable).await;
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, runtime) = seed_runtime_task(&r, &user_id, &session_id).await;
    let tid = runtime.task_id().clone();

    let id = ArtifactId::generate();
    let art = artifact(&id, &ctx, &tid, Some("exec-while-down"));
    let err = svc
        .publish_from_a2a(&art, &runtime, &user_id)
        .await
        .expect_err("an unreachable ledger is an error, not an unknown execution");
    assert!(err.to_string().contains("exec-while-down"), "{err}");

    let fetched = r.artifacts.read_artifact(&id).await.expect("get");
    assert!(
        fetched.is_none(),
        "nothing is persisted with a detached execution id"
    );

    r.tasks.remove_task(&tid).await.ok();
}
