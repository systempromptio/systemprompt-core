//! Inbound A2A notification handling through `contexts_router`.
//!
//! Seeds a real user-owned context (and, for the status path, a task) and
//! calls as that owner, so the handler persists the notification, applies the
//! status update and fans out through `broadcast_notification`. A caller who
//! does not own the context, or names a task outside it, is refused with 404
//! and leaves the task and the notification log untouched. The unseeded-context
//! and bad-envelope arms drive the 404 and 400 branches.

use axum::Extension;
use systemprompt_api::routes::contexts_router;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, SessionId, TaskId, UserId};
use systemprompt_test_fixtures::{seed_user_row, seed_user_session, unique_user_id};
use tower::ServiceExt;

use super::common::{json_post, request_context, setup_ctx};

async fn seed_context(pool: &DbPool) -> anyhow::Result<(UserId, ContextId)> {
    let user_id = unique_user_id("notif");
    let session_id = SessionId::generate();
    let email = format!("{}@notif.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await?;
    seed_user_session(pool, &user_id, &session_id).await?;

    let context_id = ContextId::generate();
    let handle = pool.pool();
    sqlx::query(
        "INSERT INTO user_contexts (context_id, user_id, session_id, name) VALUES ($1, $2, $3, $4)",
    )
    .bind(context_id.as_str())
    .bind(user_id.as_str())
    .bind(session_id.as_str())
    .bind("notif-context")
    .execute(handle.as_ref())
    .await?;
    Ok((user_id, context_id))
}

async fn seed_task(
    pool: &DbPool,
    user_id: &UserId,
    context_id: &ContextId,
) -> anyhow::Result<TaskId> {
    let task_id = TaskId::generate();
    let handle = pool.pool();
    sqlx::query(
        "INSERT INTO agent_tasks (task_id, context_id, status, status_timestamp, user_id, \
         agent_name) VALUES ($1, $2, 'TASK_STATE_WORKING', now(), $3, 'notif-agent')",
    )
    .bind(task_id.as_str())
    .bind(context_id.as_str())
    .bind(user_id.as_str())
    .execute(handle.as_ref())
    .await?;
    Ok(task_id)
}

fn app(ctx: &systemprompt_runtime::AppContext, caller: &UserId) -> axum::Router {
    contexts_router()
        .with_state(ctx.clone())
        .layer(Extension(request_context(caller.as_str())))
}

fn status_update(task_id: &TaskId, state: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/taskStatusUpdate",
        "params": {
            "agentId": "agent-x",
            "taskId": task_id.as_str(),
            "status": { "state": state },
            "task": { "id": task_id.as_str() }
        }
    })
}

async fn task_row(pool: &DbPool, task_id: &TaskId) -> anyhow::Result<(String, bool)> {
    let handle = pool.pool();
    let row: (String, bool) = sqlx::query_as(
        "SELECT status, completed_at IS NOT NULL FROM agent_tasks WHERE task_id = $1",
    )
    .bind(task_id.as_str())
    .fetch_one(handle.as_ref())
    .await?;
    Ok(row)
}

async fn notification_count(pool: &DbPool, context_id: &ContextId) -> anyhow::Result<i64> {
    let handle = pool.pool();
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM context_notifications WHERE context_id = $1")
            .bind(context_id.as_str())
            .fetch_one(handle.as_ref())
            .await?;
    Ok(count)
}

#[tokio::test]
async fn owner_task_status_update_applies_and_stamps_completion() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, context_id) = seed_context(&pool).await?;
    let task_id = seed_task(&pool, &user_id, &context_id).await?;

    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx, &user_id)
        .oneshot(json_post(&uri, status_update(&task_id, "completed")))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());

    let (status, completed) = task_row(&pool, &task_id).await?;
    assert_eq!(status, "TASK_STATE_COMPLETED");
    assert!(completed, "a terminal state stamps completed_at");
    assert_eq!(notification_count(&pool, &context_id).await?, 1);
    Ok(())
}

#[tokio::test]
async fn non_owner_cannot_update_a_task_in_another_users_context() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (owner, context_id) = seed_context(&pool).await?;
    let (intruder, _intruder_context) = seed_context(&pool).await?;
    let task_id = seed_task(&pool, &owner, &context_id).await?;

    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx, &intruder)
        .oneshot(json_post(&uri, status_update(&task_id, "canceled")))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());

    let (status, completed) = task_row(&pool, &task_id).await?;
    assert_eq!(status, "TASK_STATE_WORKING");
    assert!(!completed);
    assert_eq!(notification_count(&pool, &context_id).await?, 0);
    Ok(())
}

#[tokio::test]
async fn own_context_cannot_carry_another_users_task() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (victim, victim_context) = seed_context(&pool).await?;
    let (intruder, intruder_context) = seed_context(&pool).await?;
    let task_id = seed_task(&pool, &victim, &victim_context).await?;

    let uri = format!("/{}/notifications", intruder_context.as_str());
    let resp = app(&ctx, &intruder)
        .oneshot(json_post(&uri, status_update(&task_id, "canceled")))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());

    let (status, _) = task_row(&pool, &task_id).await?;
    assert_eq!(status, "TASK_STATE_WORKING");
    assert_eq!(notification_count(&pool, &intruder_context).await?, 0);
    Ok(())
}

#[tokio::test]
async fn task_must_belong_to_the_addressed_context() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, task_context) = seed_context(&pool).await?;
    let task_id = seed_task(&pool, &user_id, &task_context).await?;
    let other_context = ContextId::generate();
    let handle = pool.pool();
    sqlx::query("INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)")
        .bind(other_context.as_str())
        .bind(user_id.as_str())
        .bind("notif-other")
        .execute(handle.as_ref())
        .await?;

    let uri = format!("/{}/notifications", other_context.as_str());
    let resp = app(&ctx, &user_id)
        .oneshot(json_post(&uri, status_update(&task_id, "completed")))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());

    let (status, _) = task_row(&pool, &task_id).await?;
    assert_eq!(status, "TASK_STATE_WORKING");
    Ok(())
}

#[tokio::test]
async fn task_status_update_missing_task_id_is_error() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, context_id) = seed_context(&pool).await?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/taskStatusUpdate",
        "params": { "status": { "state": "completed" } }
    });
    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx, &user_id).oneshot(json_post(&uri, body)).await?;
    assert!(resp.status().is_server_error() || resp.status().is_client_error());
    Ok(())
}

#[tokio::test]
async fn artifact_created_broadcasts() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, context_id) = seed_context(&pool).await?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/artifactCreated",
        "params": {
            "taskId": "task-a",
            "artifact": { "id": "artifact-a" }
        }
    });
    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx, &user_id).oneshot(json_post(&uri, body)).await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn message_added_broadcasts() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, context_id) = seed_context(&pool).await?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/messageAdded",
        "params": {
            "messageId": "msg-a",
            "message": { "role": "assistant" }
        }
    });
    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx, &user_id).oneshot(json_post(&uri, body)).await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn unhandled_but_valid_method_persists_without_broadcast() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, context_id) = seed_context(&pool).await?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/contextUpdated",
        "params": {}
    });
    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx, &user_id).oneshot(json_post(&uri, body)).await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn unknown_method_type_rejected_by_persistence() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, context_id) = seed_context(&pool).await?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/somethingElse",
        "params": {}
    });
    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx, &user_id).oneshot(json_post(&uri, body)).await?;
    assert_eq!(resp.status().as_u16(), 400, "{}", resp.status());
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024).await?;
    let json: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(json["error_key"], "check_violation", "{json}");
    assert!(
        !json.to_string().contains("notification_type_check"),
        "the constraint name stays internal: {json}"
    );
    Ok(())
}

#[tokio::test]
async fn invalid_jsonrpc_version_returns_400() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, context_id) = seed_context(&pool).await?;

    let body = serde_json::json!({
        "jsonrpc": "1.0",
        "method": "notifications/messageAdded",
        "params": {}
    });
    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx, &user_id).oneshot(json_post(&uri, body)).await?;
    assert_eq!(resp.status().as_u16(), 400, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn unknown_context_returns_404() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, _context_id) = seed_context(&pool).await?;
    let missing = ContextId::generate();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/messageAdded",
        "params": {}
    });
    let uri = format!("/{}/notifications", missing.as_str());
    let resp = app(&ctx, &user_id).oneshot(json_post(&uri, body)).await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}
