//! `/broadcast` reads a task or artifact only when the caller owns it and it
//! sits in the addressed context.
//!
//! Two users each own a context; one owns a task, a message and an artifact.
//! The other user, broadcasting into their own context, gets 404 for each
//! foreign entity and the task row is left as it was.

use axum::Extension;
use systemprompt_api::routes::webhook_router;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ArtifactId, ContextId, SessionId, TaskId, UserId};
use systemprompt_test_fixtures::{seed_user_row, seed_user_session, unique_user_id};
use tower::ServiceExt;

use super::common::{json_post, request_context, setup_ctx};

async fn seed_context(pool: &DbPool, user_id: &UserId) -> anyhow::Result<ContextId> {
    let context_id = ContextId::generate();
    sqlx::query("INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)")
        .bind(context_id.as_str())
        .bind(user_id.as_str())
        .bind("wh-own-context")
        .execute(pool.pool().as_ref())
        .await?;
    Ok(context_id)
}

async fn seed_user(pool: &DbPool) -> anyhow::Result<(UserId, ContextId)> {
    let user_id = unique_user_id("whown");
    let email = format!("{}@whown.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await?;
    seed_user_session(pool, &user_id, &SessionId::generate()).await?;
    let context_id = seed_context(pool, &user_id).await?;
    Ok((user_id, context_id))
}

async fn seed_task(
    pool: &DbPool,
    user_id: &UserId,
    context_id: &ContextId,
) -> anyhow::Result<TaskId> {
    let task_id = TaskId::generate();
    sqlx::query(
        "INSERT INTO agent_tasks (task_id, context_id, status, status_timestamp, user_id, \
         agent_name) VALUES ($1, $2, 'TASK_STATE_WORKING', now(), $3, 'wh-agent')",
    )
    .bind(task_id.as_str())
    .bind(context_id.as_str())
    .bind(user_id.as_str())
    .execute(pool.pool().as_ref())
    .await?;
    Ok(task_id)
}

async fn seed_artifact(
    pool: &DbPool,
    context_id: &ContextId,
    task_id: &TaskId,
) -> anyhow::Result<ArtifactId> {
    let artifact_id = ArtifactId::generate();
    sqlx::query(
        "INSERT INTO task_artifacts (task_id, context_id, artifact_id, name, artifact_type) \
         VALUES ($1, $2, $3, 'private-artifact-name', 'table')",
    )
    .bind(task_id.as_str())
    .bind(context_id.as_str())
    .bind(artifact_id.as_str())
    .execute(pool.pool().as_ref())
    .await?;
    Ok(artifact_id)
}

fn app(ctx: &systemprompt_runtime::AppContext, user: &UserId) -> axum::Router {
    webhook_router()
        .with_state(ctx.clone())
        .layer(Extension(request_context(user.as_str())))
}

fn body(
    event_type: &str,
    entity_id: &str,
    context_id: &ContextId,
    user: &UserId,
) -> serde_json::Value {
    serde_json::json!({
        "event_type": event_type,
        "entity_id": entity_id,
        "context_id": context_id.as_str(),
        "user_id": user.as_str(),
    })
}

async fn read_body(resp: axum::response::Response) -> anyhow::Result<String> {
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

async fn task_status(pool: &DbPool, task_id: &TaskId) -> anyhow::Result<String> {
    let status: String = sqlx::query_scalar("SELECT status FROM agent_tasks WHERE task_id = $1")
        .bind(task_id.as_str())
        .fetch_one(pool.pool().as_ref())
        .await?;
    Ok(status)
}

#[tokio::test]
async fn task_completed_for_another_users_task_is_refused() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (victim, victim_context) = seed_user(&pool).await?;
    let (intruder, intruder_context) = seed_user(&pool).await?;
    let task_id = seed_task(&pool, &victim, &victim_context).await?;

    let resp = app(&ctx, &intruder)
        .oneshot(json_post(
            "/broadcast",
            body(
                "task_completed",
                task_id.as_str(),
                &intruder_context,
                &intruder,
            ),
        ))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    assert!(!read_body(resp).await?.contains("TASK_STATE_WORKING"));
    assert_eq!(task_status(&pool, &task_id).await?, "TASK_STATE_WORKING");
    Ok(())
}

#[tokio::test]
async fn artifact_created_for_another_users_artifact_is_refused() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (victim, victim_context) = seed_user(&pool).await?;
    let (intruder, intruder_context) = seed_user(&pool).await?;
    let task_id = seed_task(&pool, &victim, &victim_context).await?;
    let artifact_id = seed_artifact(&pool, &victim_context, &task_id).await?;

    let resp = app(&ctx, &intruder)
        .oneshot(json_post(
            "/broadcast",
            body(
                "artifact_created",
                artifact_id.as_str(),
                &intruder_context,
                &intruder,
            ),
        ))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    assert!(!read_body(resp).await?.contains("private-artifact-name"));
    Ok(())
}

#[tokio::test]
async fn own_task_in_a_different_context_is_refused() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, task_context) = seed_user(&pool).await?;
    let other_context = seed_context(&pool, &user_id).await?;
    let task_id = seed_task(&pool, &user_id, &task_context).await?;
    let artifact_id = seed_artifact(&pool, &task_context, &task_id).await?;

    for (event, entity) in [
        ("task_completed", task_id.as_str()),
        ("artifact_created", artifact_id.as_str()),
    ] {
        let resp = app(&ctx, &user_id)
            .oneshot(json_post(
                "/broadcast",
                body(event, entity, &other_context, &user_id),
            ))
            .await?;
        assert_eq!(resp.status().as_u16(), 404, "{event}: {}", resp.status());
    }
    Ok(())
}

#[tokio::test]
async fn owner_broadcasts_own_task_and_artifact() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, context_id) = seed_user(&pool).await?;
    let task_id = seed_task(&pool, &user_id, &context_id).await?;
    let artifact_id = seed_artifact(&pool, &context_id, &task_id).await?;

    for (event, entity) in [
        ("task_completed", task_id.as_str()),
        ("artifact_created", artifact_id.as_str()),
    ] {
        let resp = app(&ctx, &user_id)
            .oneshot(json_post(
                "/broadcast",
                body(event, entity, &context_id, &user_id),
            ))
            .await?;
        assert_eq!(resp.status().as_u16(), 200, "{event}: {}", resp.status());
    }
    assert_eq!(task_status(&pool, &task_id).await?, "TASK_STATE_WORKING");
    Ok(())
}
