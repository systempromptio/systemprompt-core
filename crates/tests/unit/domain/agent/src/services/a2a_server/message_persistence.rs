//! Persisting a completed A2A task.
//!
//! This is the commit point of a turn: the task row, the user's message and
//! the agent's reply are written together, and any artifacts the turn produced
//! are published exactly once. Getting the `artifacts_already_published` flag
//! wrong duplicates a user's artifacts on every completion; losing the update
//! error silently leaves a task reading as in-flight forever. The artifact
//! broadcast is a side channel: its failure is reported in the outcome, never
//! as a persistence error.

use std::sync::Arc;

use systemprompt_agent::models::a2a::{
    Artifact, ArtifactMetadata, Message, MessageRole, Part, TaskState, TextPart,
};
use systemprompt_agent::repository::A2ARepositories;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::services::a2a_server::processing::message::persistence::{
    PersistCompletedTaskParams, persist_completed_task,
};
use systemprompt_agent::services::a2a_server::streaming::webhook_client::DynWebhookBroadcaster;
use systemprompt_agent::services::{ArtifactPublishingService, SkillService};
use systemprompt_identifiers::{
    Actor, AgentName, ArtifactId, ContextId, MessageId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, not_managed_skills};
use systemprompt_test_mocks::{RecordingWebhookBroadcaster, recording_webhooks};

fn publishing(
    pool: &systemprompt_database::DbPool,
    repositories: &A2ARepositories,
    webhooks: DynWebhookBroadcaster,
) -> ArtifactPublishingService {
    let steps = Arc::new(ExecutionStepRepository::new(pool).expect("step repo"));
    let skills =
        Arc::new(SkillService::new(not_managed_skills(), steps, webhooks).expect("skills"));
    ArtifactPublishingService::new(repositories, skills)
}

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
        AgentName::try_new("persist-agent").expect("valid AgentName"),
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
    let webhooks = recording_webhooks();

    let outcome = persist_completed_task(PersistCompletedTaskParams {
        task: &task,
        user_message: &user_message,
        agent_message: &agent_message,
        context: &context,
        repositories: &repositories,
        publishing: &publishing(&pool, &repositories, Arc::clone(&webhooks)),
        webhooks,
        artifacts_already_published: true,
    })
    .await
    .expect("a completed turn must persist");

    let updated = outcome.task;
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

    let webhooks = recording_webhooks();
    persist_completed_task(PersistCompletedTaskParams {
        task: &task,
        user_message: &message(MessageRole::User, &ctx, &task_id, "ask"),
        agent_message: &message(MessageRole::Agent, &ctx, &task_id, "answer"),
        context: &request_context(&ctx, &session_id, &user_id),
        repositories: &repositories,
        publishing: &publishing(&pool, &repositories, Arc::clone(&webhooks)),
        webhooks,
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

    let webhooks = recording_webhooks();
    let err = persist_completed_task(PersistCompletedTaskParams {
        task: &task,
        user_message: &message(MessageRole::User, &ctx, &ghost, "ask"),
        agent_message: &message(MessageRole::Agent, &ctx, &ghost, "answer"),
        context: &request_context(&ctx, &session_id, &user_id),
        repositories: &repositories,
        publishing: &publishing(&pool, &repositories, Arc::clone(&webhooks)),
        webhooks,
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

async fn persist_artifacts(broadcast_ok: bool) {
    let pool = try_pool_or_skip()
        .await
        .expect("persistence coverage requires PostgreSQL");
    ensure_test_bootstrap();
    let rec: Arc<RecordingWebhookBroadcaster> = if broadcast_ok {
        Arc::new(RecordingWebhookBroadcaster::new())
    } else {
        Arc::new(RecordingWebhookBroadcaster::with_lifecycle_down())
    };
    let webhooks: DynWebhookBroadcaster = rec.clone();
    let repositories = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repositories, &user_id, &session_id).await;
    let mut task = make_task(&task_id, &ctx);
    task.status.state = TaskState::Completed;
    let first = artifact(&ctx, &task_id);
    let second = artifact(&ctx, &task_id);
    let ids = [first.id.clone(), second.id.clone()];
    task.artifacts = Some(vec![first, second]);
    let outcome = persist_completed_task(PersistCompletedTaskParams {
        task: &task,
        user_message: &message(MessageRole::User, &ctx, &task_id, "publish these"),
        agent_message: &message(MessageRole::Agent, &ctx, &task_id, "published"),
        context: &request_context(&ctx, &session_id, &user_id),
        repositories: &repositories,
        publishing: &publishing(&pool, &repositories, Arc::clone(&webhooks)),
        webhooks,
        artifacts_already_published: false,
    })
    .await
    .expect("the task and its artifacts commit whether or not the webhook is up");

    assert_eq!(outcome.task.status.state, TaskState::Completed);
    assert_eq!(
        rec.lifecycle_events().len(),
        2,
        "one broadcast per artifact"
    );
    if broadcast_ok {
        assert!(outcome.undelivered_broadcasts.is_empty());
    } else {
        let undelivered: Vec<_> = outcome
            .undelivered_broadcasts
            .iter()
            .map(|(id, _)| id.clone())
            .collect();
        assert_eq!(
            undelivered,
            ids.to_vec(),
            "every failed broadcast is reported"
        );
    }
    for id in ids {
        let stored = repositories
            .artifacts
            .get_artifact_by_id(&id)
            .await
            .unwrap();
        assert!(stored.is_some(), "artifact {id} must be persisted");
    }
    let stored = repositories
        .tasks
        .get_task(&task_id)
        .await
        .expect("task readable")
        .expect("task present");
    assert_eq!(
        stored.status.state,
        TaskState::Completed,
        "a webhook outage never fails a committed task"
    );
}

#[tokio::test]
async fn coverage_unpublished_artifacts_are_saved_with_the_completed_task() {
    persist_artifacts(true).await;
}

#[tokio::test]
async fn a_failed_artifact_broadcast_is_reported_but_the_task_stays_completed() {
    persist_artifacts(false).await;
}
