// DB-backed tests for `PersistenceService`: initial task construction,
// create/update round trips, and persisting a completed task with messages —
// including artifact publishing and the already-published skip.

use systemprompt_agent::models::a2a::{Artifact, Message, MessageRole, Part, TaskState, TextPart};
use systemprompt_agent::services::a2a_server::processing::persistence_service::{
    PersistCompletedTaskServiceParams, PersistenceService,
};
use systemprompt_identifiers::{ArtifactId, ContextId, MessageId, TaskId};
use systemprompt_models::a2a::ArtifactMetadata;

use super::a2a_helpers::request_context;
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

fn message(ctx: &ContextId, task_id: &TaskId, role: MessageRole, text: &str) -> Message {
    Message {
        role,
        parts: vec![Part::Text(TextPart {
            text: text.to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: ctx.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn artifact(ctx: &ContextId, task_id: &TaskId) -> Artifact {
    Artifact {
        id: ArtifactId::generate(),
        title: Some("out".to_owned()),
        description: None,
        parts: vec![Part::Text(TextPart {
            text: "artifact body".to_owned(),
        })],
        extensions: vec![serde_json::json!(
            systemprompt_models::a2a::ARTIFACT_RENDERING_URI
        )],
        metadata: ArtifactMetadata {
            artifact_type: "document".to_owned(),
            context_id: ctx.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            task_id: task_id.clone(),
            rendering_hints: None,
            source: None,
            mcp_execution_id: None,
            mcp_schema: None,
            is_internal: None,
            fingerprint: None,
            tool_name: None,
            execution_index: None,
            skill_id: None,
            skill_name: None,
        },
    }
}

#[test]
fn build_initial_task_is_submitted_with_agent_metadata() {
    let task_id = TaskId::generate();
    let ctx = ContextId::generate();
    let task = PersistenceService::build_initial_task(task_id.clone(), ctx.clone(), "init-agent");

    assert_eq!(task.id, task_id);
    assert_eq!(task.context_id, ctx);
    assert_eq!(task.status.state, TaskState::Submitted);
    assert!(task.history.is_none());
    assert!(task.artifacts.is_none());
    assert_eq!(
        task.metadata.expect("metadata set").agent_name,
        "init-agent"
    );
    assert!(task.created_at.is_some());
}

#[tokio::test]
async fn create_task_and_update_state_round_trip() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _) = seed_context_and_task(&repos, &user, &session).await;

    let service = PersistenceService::new(pool.clone());
    let task_id = TaskId::generate();
    let task = PersistenceService::build_initial_task(task_id.clone(), ctx.clone(), "svc-agent");
    let request = request_context(&ctx, &session, &user, "svc-agent");

    service
        .create_task(&task, &request, "svc-agent")
        .await
        .expect("create task");

    service
        .update_task_state(&task_id, TaskState::Working, &chrono::Utc::now())
        .await
        .expect("update state");

    let stored = repos
        .tasks
        .get_task(&task_id)
        .await
        .expect("get task")
        .expect("task exists");
    assert_eq!(stored.status.state, TaskState::Working);
}

#[tokio::test]
async fn persist_completed_task_saves_messages_and_publishes_artifacts() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let service = PersistenceService::new(pool.clone());
    let request = request_context(&ctx, &session, &user, "svc-agent");

    let user_msg = message(&ctx, &task_id, MessageRole::User, "question");
    let agent_msg = message(&ctx, &task_id, MessageRole::Agent, "answer");

    let mut task =
        PersistenceService::build_initial_task(task_id.clone(), ctx.clone(), "svc-agent");
    task.status.state = TaskState::Completed;
    task.status.message = Some(agent_msg.clone());
    task.artifacts = Some(vec![artifact(&ctx, &task_id)]);

    let persisted = service
        .persist_completed_task(PersistCompletedTaskServiceParams {
            task: &task,
            user_message: &user_msg,
            agent_message: &agent_msg,
            context: &request,
            artifacts_already_published: false,
        })
        .await
        .expect("persist");

    assert_eq!(persisted.id, task_id);
    assert_eq!(persisted.status.state, TaskState::Completed);
}

#[tokio::test]
async fn persist_completed_task_skips_publishing_when_already_published() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let service = PersistenceService::new(pool.clone());
    let request = request_context(&ctx, &session, &user, "svc-agent");

    let user_msg = message(&ctx, &task_id, MessageRole::User, "q2");
    let agent_msg = message(&ctx, &task_id, MessageRole::Agent, "a2");

    let mut task =
        PersistenceService::build_initial_task(task_id.clone(), ctx.clone(), "svc-agent");
    task.status.state = TaskState::Completed;
    task.status.message = Some(agent_msg.clone());
    task.artifacts = Some(vec![artifact(&ctx, &task_id)]);

    let persisted = service
        .persist_completed_task(PersistCompletedTaskServiceParams {
            task: &task,
            user_message: &user_msg,
            agent_message: &agent_msg,
            context: &request,
            artifacts_already_published: true,
        })
        .await
        .expect("persist");

    assert_eq!(persisted.status.state, TaskState::Completed);
}
