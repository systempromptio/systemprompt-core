//! Webhook broadcast surface: `broadcast_handlers` (`/a2a`, `/agui`) and
//! `context_broadcast` (`/broadcast`) plus the `event_loader` dispatch.
//!
//! `/a2a` and `/agui` are driven for both the user-match happy path and the
//! mismatch 403. `/broadcast` seeds a user-owned context (and, per event type,
//! a task or artifact) so `load_event_data` resolves each `event_type` arm:
//! `context_updated`, `task_completed`, `artifact_created`, `message_received`
//! (not-found), `execution_step`, `task_created`, and the unknown-type
//! bad-request branch. `EventRouter` returns zero broadcasts with no
//! subscribers, so every arm runs to completion.

use axum::Extension;
use systemprompt_api::routes::webhook_router;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    Actor, AgentName, ArtifactId, ContextId, MessageId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::a2a::{
    Message, MessageRole, Part, Task, TaskState as ModelsTaskState, TextPart,
};
use systemprompt_models::{
    A2AEventBuilder, AgUiEventBuilder, ExecutionStep, RequestContext, StepContent,
};
use systemprompt_test_fixtures::{seed_user_row, seed_user_session, unique_user_id};
use tower::ServiceExt;

use super::common::{json_post, setup_ctx};

struct Seeded {
    user_id: UserId,
    context_id: ContextId,
}

async fn seed_context(pool: &DbPool) -> anyhow::Result<Seeded> {
    let user_id = unique_user_id("wh");
    let session_id = SessionId::generate();
    let email = format!("{}@wh.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await?;
    seed_user_session(pool, &user_id, &session_id).await?;

    let context_id = ContextId::generate();
    let handle = pool.pool_arc()?;
    sqlx::query(
        "INSERT INTO user_contexts (context_id, user_id, session_id, name) VALUES ($1, $2, $3, $4)",
    )
    .bind(context_id.as_str())
    .bind(user_id.as_str())
    .bind(session_id.as_str())
    .bind("wh-context")
    .execute(handle.as_ref())
    .await?;
    Ok(Seeded {
        user_id,
        context_id,
    })
}

async fn seed_task(pool: &DbPool, s: &Seeded) -> anyhow::Result<TaskId> {
    let task_id = TaskId::generate();
    let handle = pool.pool_arc()?;
    sqlx::query(
        "INSERT INTO agent_tasks (task_id, context_id, status, status_timestamp, user_id, \
         agent_name) VALUES ($1, $2, 'TASK_STATE_WORKING', now(), $3, 'wh-agent')",
    )
    .bind(task_id.as_str())
    .bind(s.context_id.as_str())
    .bind(s.user_id.as_str())
    .execute(handle.as_ref())
    .await?;
    Ok(task_id)
}

async fn seed_artifact(pool: &DbPool, s: &Seeded, task_id: &TaskId) -> anyhow::Result<ArtifactId> {
    let artifact_id = ArtifactId::generate();
    let handle = pool.pool_arc()?;
    sqlx::query(
        "INSERT INTO task_artifacts (task_id, context_id, artifact_id, name, artifact_type) \
         VALUES ($1, $2, $3, $4, 'table')",
    )
    .bind(task_id.as_str())
    .bind(s.context_id.as_str())
    .bind(artifact_id.as_str())
    .bind("wh artifact")
    .execute(handle.as_ref())
    .await?;
    Ok(artifact_id)
}

fn request_context_for(user: &UserId) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("wh-agent"),
    )
    .with_actor(Actor::user(user.clone()))
}

fn app(ctx: &systemprompt_runtime::AppContext, user: &UserId) -> axum::Router {
    webhook_router()
        .with_state(ctx.clone())
        .layer(Extension(request_context_for(user)))
}

fn with_user_id(mut value: serde_json::Value, user: &UserId) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "user_id".to_owned(),
            serde_json::Value::String(user.as_str().to_owned()),
        );
    }
    value
}

#[tokio::test]
async fn a2a_broadcast_user_match_succeeds() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let user = unique_user_id("wh-a2a");
    let event = A2AEventBuilder::task_status_update(
        TaskId::new("wh-task"),
        ContextId::generate(),
        ModelsTaskState::Working,
        Some("go".to_owned()),
    );
    let body = with_user_id(serde_json::to_value(&event)?, &user);
    let resp = app(&ctx, &user).oneshot(json_post("/a2a", body)).await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn a2a_broadcast_user_mismatch_returns_403() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let auth_user = unique_user_id("wh-a2a-auth");
    let claimed = unique_user_id("wh-a2a-claim");
    let event = A2AEventBuilder::task_status_update(
        TaskId::new("wh-task"),
        ContextId::generate(),
        ModelsTaskState::Working,
        None,
    );
    let body = with_user_id(serde_json::to_value(&event)?, &claimed);
    let resp = app(&ctx, &auth_user)
        .oneshot(json_post("/a2a", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 403, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn agui_broadcast_user_match_succeeds() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let user = unique_user_id("wh-agui");
    let event = AgUiEventBuilder::run_started(ContextId::generate(), TaskId::new("wh-task"), None);
    let body = with_user_id(serde_json::to_value(&event)?, &user);
    let resp = app(&ctx, &user).oneshot(json_post("/agui", body)).await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn agui_broadcast_user_mismatch_returns_403() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let auth_user = unique_user_id("wh-agui-auth");
    let claimed = unique_user_id("wh-agui-claim");
    let event = AgUiEventBuilder::run_started(ContextId::generate(), TaskId::new("wh-task"), None);
    let body = with_user_id(serde_json::to_value(&event)?, &claimed);
    let resp = app(&ctx, &auth_user)
        .oneshot(json_post("/agui", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 403, "{}", resp.status());
    Ok(())
}

fn webhook_body(
    event_type: &str,
    entity_id: &str,
    s: &Seeded,
    extra: serde_json::Value,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "event_type": event_type,
        "entity_id": entity_id,
        "context_id": s.context_id.as_str(),
        "user_id": s.user_id.as_str(),
    });
    if let (Some(obj), Some(extra_obj)) = (body.as_object_mut(), extra.as_object()) {
        for (k, v) in extra_obj {
            obj.insert(k.clone(), v.clone());
        }
    }
    body
}

#[tokio::test]
async fn broadcast_user_mismatch_returns_403() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let other = unique_user_id("wh-other");
    let body = webhook_body("task_created", "task-1", &s, serde_json::json!({}));
    let resp = app(&ctx, &other)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 403, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_context_updated_succeeds() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let body = webhook_body("context_updated", "ignored", &s, serde_json::json!({}));
    let resp = app(&ctx, &s.user_id)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_task_completed_loads_task() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let task_id = seed_task(&pool, &s).await?;
    let body = webhook_body(
        "task_completed",
        task_id.as_str(),
        &s,
        serde_json::json!({}),
    );
    let resp = app(&ctx, &s.user_id)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_artifact_created_loads_artifact() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let task_id = seed_task(&pool, &s).await?;
    let artifact_id = seed_artifact(&pool, &s, &task_id).await?;
    let body = webhook_body(
        "artifact_created",
        artifact_id.as_str(),
        &s,
        serde_json::json!({}),
    );
    let resp = app(&ctx, &s.user_id)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_message_received_missing_returns_404() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let body = webhook_body(
        "message_received",
        "no-such-message",
        &s,
        serde_json::json!({}),
    );
    let resp = app(&ctx, &s.user_id)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_execution_step_succeeds() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let step = ExecutionStep::new(TaskId::new("wh-step-task"), StepContent::understanding());
    let body = webhook_body(
        "execution_step",
        "wh-step-task",
        &s,
        serde_json::json!({ "step_data": serde_json::to_value(&step)? }),
    );
    let resp = app(&ctx, &s.user_id)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_execution_step_missing_data_returns_400() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let body = webhook_body("execution_step", "wh-step-task", &s, serde_json::json!({}));
    let resp = app(&ctx, &s.user_id)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 400, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_task_created_with_history_succeeds() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let mut task = Task::default();
    task.context_id = s.context_id.clone();
    task.history = Some(vec![Message {
        role: MessageRole::Agent,
        parts: vec![Part::Text(TextPart {
            text: "hi".to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id: None,
        context_id: s.context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }]);
    let body = webhook_body(
        "task_created",
        "wh-run",
        &s,
        serde_json::json!({ "task_data": { "task": serde_json::to_value(&task)? } }),
    );
    let resp = app(&ctx, &s.user_id)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_task_created_empty_history_returns_400() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let task = Task::default();
    let body = webhook_body(
        "task_created",
        "wh-run",
        &s,
        serde_json::json!({ "task_data": { "task": serde_json::to_value(&task)? } }),
    );
    let resp = app(&ctx, &s.user_id)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 400, "{}", resp.status());
    Ok(())
}

async fn seed_message(pool: &DbPool, s: &Seeded, task_id: &TaskId) -> anyhow::Result<MessageId> {
    let message_id = MessageId::generate();
    let handle = pool.pool_arc()?;
    sqlx::query(
        "INSERT INTO task_messages (task_id, message_id, role, context_id, user_id, \
         sequence_number) VALUES ($1, $2, 'user', $3, $4, 1)",
    )
    .bind(task_id.as_str())
    .bind(message_id.as_str())
    .bind(s.context_id.as_str())
    .bind(s.user_id.as_str())
    .execute(handle.as_ref())
    .await?;
    Ok(message_id)
}

#[tokio::test]
async fn broadcast_task_completed_unknown_task_returns_404() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let body = webhook_body("task_completed", "no-such-task", &s, serde_json::json!({}));
    let resp = app(&ctx, &s.user_id)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_task_completed_with_messages_carries_history() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let task_id = seed_task(&pool, &s).await?;
    seed_message(&pool, &s, &task_id).await?;
    let body = webhook_body(
        "task_completed",
        task_id.as_str(),
        &s,
        serde_json::json!({}),
    );
    let resp = app(&ctx, &s.user_id)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());

    let handle = pool.pool_arc()?;
    let status: String = sqlx::query_scalar("SELECT status FROM agent_tasks WHERE task_id = $1")
        .bind(task_id.as_str())
        .fetch_one(handle.as_ref())
        .await?;
    assert_eq!(status, "TASK_STATE_COMPLETED");
    Ok(())
}

#[tokio::test]
async fn broadcast_artifact_created_missing_returns_404() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let body = webhook_body(
        "artifact_created",
        "no-such-artifact",
        &s,
        serde_json::json!({}),
    );
    let resp = app(&ctx, &s.user_id)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_message_received_existing_succeeds() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let task_id = seed_task(&pool, &s).await?;
    let message_id = seed_message(&pool, &s, &task_id).await?;
    let body = webhook_body(
        "message_received",
        message_id.as_str(),
        &s,
        serde_json::json!({}),
    );
    let resp = app(&ctx, &s.user_id)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_unknown_event_type_returns_400() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let s = seed_context(&pool).await?;
    let body = webhook_body("totally_unknown", "x", &s, serde_json::json!({}));
    let resp = app(&ctx, &s.user_id)
        .oneshot(json_post("/broadcast", body))
        .await?;
    assert_eq!(resp.status().as_u16(), 400, "{}", resp.status());
    Ok(())
}
