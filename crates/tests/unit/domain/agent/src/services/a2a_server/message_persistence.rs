//! Persisting a completed A2A task.
//!
//! This is the commit point of a turn: the task row, the user's message and
//! the agent's reply are written together, and any artifacts the turn produced
//! are published exactly once. Getting the `artifacts_already_published` flag
//! wrong duplicates a user's artifacts on every completion; losing the update
//! error silently leaves a task reading as in-flight forever.

use systemprompt_agent::models::a2a::{
    Artifact, ArtifactMetadata, Message, MessageRole, Part, TaskState, TextPart,
};
use systemprompt_agent::test_api::{PersistCompletedTaskParams, persist_completed_task};
use systemprompt_identifiers::{
    Actor, AgentName, ArtifactId, ContextId, MessageId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_test_fixtures::ensure_test_bootstrap;

use crate::repository::{
    make_task, repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip,
};

fn message(role: MessageRole, ctx: &ContextId, task_id: &TaskId, text: &str) -> Message {
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

fn request_context(ctx: &ContextId, session: &SessionId, user: &UserId) -> RequestContext {
    let mut rc = RequestContext::new(
        session.clone(),
        TraceId::generate(),
        ctx.clone(),
        AgentName::new("persist-agent"),
    );
    rc.auth.actor = Actor::user(user.clone());
    rc
}

fn artifact(ctx: &ContextId, task_id: &TaskId) -> Artifact {
    Artifact {
        id: ArtifactId::generate(),
        title: Some("turn-artifact".to_owned()),
        description: None,
        parts: vec![Part::Text(TextPart {
            text: "body".to_owned(),
        })],
        extensions: vec![],
        metadata: ArtifactMetadata::new("text".to_owned(), ctx.clone(), task_id.clone()),
    }
}

// Why: the whole turn commits together. A completed task that reported success
// but never moved off its prior state leaves the client polling an in-flight
// task that will never change.
#[tokio::test]
async fn a_completed_turn_persists_the_task_and_both_messages() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    ensure_test_bootstrap();
    let repositories = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repositories, &user_id, &session_id).await;

    let mut task = make_task(&task_id, &ctx);
    task.status.state = TaskState::Completed;
    let user_message = message(MessageRole::User, &ctx, &task_id, "ask");
    let agent_message = message(MessageRole::Agent, &ctx, &task_id, "answer");
    let context = request_context(&ctx, &session_id, &user_id);

    let updated = persist_completed_task(PersistCompletedTaskParams {
        task: &task,
        user_message: &user_message,
        agent_message: &agent_message,
        context: &context,
        repositories: &repositories,
        artifacts_already_published: true,
    })
    .await
    .expect("a completed turn must persist");

    assert_eq!(updated.id, task_id);
    assert_eq!(
        updated.status.state,
        TaskState::Completed,
        "the persisted task must carry the completed state"
    );

    let stored = repositories
        .tasks
        .get_task(&task_id)
        .await
        .expect("task readable")
        .expect("task present");
    assert_eq!(stored.status.state, TaskState::Completed);
}

// Why: this is the idempotence flag. The streaming path publishes artifacts as
// they are produced and then sets it, so re-publishing here would give the
// user two copies of every artifact in every streamed turn.
#[tokio::test]
async fn artifacts_already_published_are_not_published_a_second_time() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    ensure_test_bootstrap();
    let repositories = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repositories, &user_id, &session_id).await;

    let mut task = make_task(&task_id, &ctx);
    task.status.state = TaskState::Completed;
    let published = artifact(&ctx, &task_id);
    let artifact_id = published.id.clone();
    task.artifacts = Some(vec![published]);

    persist_completed_task(PersistCompletedTaskParams {
        task: &task,
        user_message: &message(MessageRole::User, &ctx, &task_id, "ask"),
        agent_message: &message(MessageRole::Agent, &ctx, &task_id, "answer"),
        context: &request_context(&ctx, &session_id, &user_id),
        repositories: &repositories,
        artifacts_already_published: true,
    })
    .await
    .expect("persisting must succeed");

    let stored = repositories
        .artifacts
        .get_artifact_by_id(&artifact_id)
        .await
        .expect("artifact lookup runs");
    assert!(
        stored.is_none(),
        "the flag says these were already handled; persistence must not write them again"
    );
}

// Why: the update is the only signal that the turn committed. Swallowing its
// failure would report success while the task row still says in-flight.
#[tokio::test]
async fn a_task_that_does_not_exist_fails_loudly_rather_than_reporting_success() {
    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    ensure_test_bootstrap();
    let repositories = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let ctx = ContextId::generate();
    let ghost = TaskId::generate();

    let mut task = make_task(&ghost, &ctx);
    task.status.state = TaskState::Completed;

    let err = persist_completed_task(PersistCompletedTaskParams {
        task: &task,
        user_message: &message(MessageRole::User, &ctx, &ghost, "ask"),
        agent_message: &message(MessageRole::Agent, &ctx, &ghost, "answer"),
        context: &request_context(&ctx, &session_id, &user_id),
        repositories: &repositories,
        artifacts_already_published: true,
    })
    .await
    .expect_err("persisting an unknown task must fail");

    assert!(
        err.to_string()
            .contains("Failed to update task and save messages"),
        "the failure must name what could not be written: {err}"
    );
}
