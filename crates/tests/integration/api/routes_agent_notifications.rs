//! Inbound A2A notification handling through `contexts_router`.
//!
//! Seeds a real user-owned context (and, for the status path, a task) so the
//! handler resolves the owning user, persists the notification, runs the
//! method-dispatch matrix in `process_notification`, and fans out through
//! `broadcast_notification`. The unseeded-context and bad-envelope arms drive
//! the 404 and 400 branches.

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
    let handle = pool.pool_arc()?;
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
    let handle = pool.pool_arc()?;
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

fn app(ctx: &systemprompt_runtime::AppContext) -> axum::Router {
    contexts_router()
        .with_state(ctx.clone())
        .layer(Extension(request_context("notif-caller")))
}

#[tokio::test]
async fn task_status_update_applies_and_broadcasts() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, context_id) = seed_context(&pool).await?;
    let task_id = seed_task(&pool, &user_id, &context_id).await?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/taskStatusUpdate",
        "params": {
            "agentId": "agent-x",
            "taskId": task_id.as_str(),
            "status": { "state": "completed" },
            "task": { "id": task_id.as_str() }
        }
    });
    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx).oneshot(json_post(&uri, body)).await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn task_status_update_missing_task_id_is_error() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (_user, context_id) = seed_context(&pool).await?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/taskStatusUpdate",
        "params": { "status": { "state": "completed" } }
    });
    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx).oneshot(json_post(&uri, body)).await?;
    assert!(resp.status().is_server_error() || resp.status().is_client_error());
    Ok(())
}

#[tokio::test]
async fn artifact_created_broadcasts() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (_user, context_id) = seed_context(&pool).await?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/artifactCreated",
        "params": {
            "taskId": "task-a",
            "artifact": { "id": "artifact-a" }
        }
    });
    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx).oneshot(json_post(&uri, body)).await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn message_added_broadcasts() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (_user, context_id) = seed_context(&pool).await?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/messageAdded",
        "params": {
            "messageId": "msg-a",
            "message": { "role": "assistant" }
        }
    });
    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx).oneshot(json_post(&uri, body)).await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn unhandled_but_valid_method_persists_without_broadcast() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (_user, context_id) = seed_context(&pool).await?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/contextUpdated",
        "params": {}
    });
    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx).oneshot(json_post(&uri, body)).await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn unknown_method_type_rejected_by_persistence() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (_user, context_id) = seed_context(&pool).await?;

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/somethingElse",
        "params": {}
    });
    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx).oneshot(json_post(&uri, body)).await?;
    assert!(resp.status().is_server_error(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn invalid_jsonrpc_version_returns_400() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (_user, context_id) = seed_context(&pool).await?;

    let body = serde_json::json!({
        "jsonrpc": "1.0",
        "method": "notifications/messageAdded",
        "params": {}
    });
    let uri = format!("/{}/notifications", context_id.as_str());
    let resp = app(&ctx).oneshot(json_post(&uri, body)).await?;
    assert_eq!(resp.status().as_u16(), 400, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn unknown_context_returns_404() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let missing = ContextId::generate();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/messageAdded",
        "params": {}
    });
    let uri = format!("/{}/notifications", missing.as_str());
    let resp = app(&ctx).oneshot(json_post(&uri, body)).await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}
