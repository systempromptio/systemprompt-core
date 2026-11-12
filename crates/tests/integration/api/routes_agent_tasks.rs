//! Agent tasks routes — list/get/delete against a real DB. These hit the
//! TaskRepository and exercise the typed-ID path conversions, status filters,
//! per-handler error mappings, and the `/core/*` resource-ownership guard
//! (a task is only visible to a user who owns its context).

use axum::Extension;
use systemprompt_api::routes::tasks_router;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, TaskId, UserId};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::seed_user_row;
use tower::ServiceExt;

use super::common::{empty_delete, empty_get, request_context, setup_ctx};

fn app(ctx: &AppContext) -> axum::Router {
    app_for(ctx, "user_tasks")
}

fn app_for(ctx: &AppContext, user: &str) -> axum::Router {
    tasks_router()
        .with_state(ctx.clone())
        .layer(Extension(request_context(user)))
}

async fn seed_owned_task(pool: &DbPool, owner: &UserId) -> anyhow::Result<(ContextId, TaskId)> {
    seed_user_row(pool, owner, &format!("{}@test.local", owner.as_str())).await?;
    let p = pool.pool();
    let context_id = ContextId::generate();
    let task_id = TaskId::generate();
    sqlx::query!(
        "INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)",
        context_id.as_str(),
        owner.as_str(),
        "ownership-test",
    )
    .execute(p.as_ref())
    .await?;
    sqlx::query!(
        "INSERT INTO agent_tasks (task_id, context_id, agent_name) VALUES ($1, $2, $3)",
        task_id.as_str(),
        context_id.as_str(),
        "ownership-agent",
    )
    .execute(p.as_ref())
    .await?;
    Ok((context_id, task_id))
}

#[tokio::test]
async fn list_user_tasks_returns_array() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let resp = app(&ctx).oneshot(empty_get("/")).await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn list_user_tasks_accepts_status_filter() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let resp = app(&ctx)
        .oneshot(empty_get("/?status=completed&limit=5"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn list_user_tasks_handles_all_status_values() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    for s in [
        "submitted",
        "working",
        "input-required",
        "completed",
        "canceled",
        "failed",
        "rejected",
        "auth-required",
    ] {
        let resp = app(&ctx)
            .oneshot(empty_get(&format!("/?status={s}")))
            .await?;
        assert!(
            resp.status().is_success(),
            "status={s} -> {}",
            resp.status()
        );
    }
    for s in ["cancelled", "weird-unknown"] {
        let resp = app(&ctx)
            .oneshot(empty_get(&format!("/?status={s}")))
            .await?;
        assert_eq!(
            resp.status(),
            http::StatusCode::BAD_REQUEST,
            "status={s} is outside the task-state vocabulary"
        );
    }
    Ok(())
}

#[tokio::test]
async fn get_missing_task_returns_404() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let resp = app(&ctx).oneshot(empty_get("/task_does_not_exist")).await?;
    assert_eq!(resp.status().as_u16(), 404);
    Ok(())
}

#[tokio::test]
async fn get_messages_for_missing_task_returns_404() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let resp = app(&ctx)
        .oneshot(empty_get("/task_missing/messages"))
        .await?;
    assert_eq!(resp.status().as_u16(), 404);
    Ok(())
}

#[tokio::test]
async fn delete_missing_task_returns_404() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let resp = app(&ctx).oneshot(empty_delete("/task_missing")).await?;
    assert_eq!(resp.status().as_u16(), 404);
    Ok(())
}

#[tokio::test]
async fn owner_can_get_own_task() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = UserId::new(format!("owner-{}", uuid::Uuid::new_v4()));
    let (_context_id, task_id) = seed_owned_task(&pool, &owner).await?;

    let resp = app_for(&ctx, owner.as_str())
        .oneshot(empty_get(&format!("/{}", task_id.as_str())))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn non_owner_get_task_returns_404() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = UserId::new(format!("owner-{}", uuid::Uuid::new_v4()));
    let (_context_id, task_id) = seed_owned_task(&pool, &owner).await?;

    let resp = app_for(&ctx, "intruder")
        .oneshot(empty_get(&format!("/{}", task_id.as_str())))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn non_owner_get_messages_returns_404() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = UserId::new(format!("owner-{}", uuid::Uuid::new_v4()));
    let (_context_id, task_id) = seed_owned_task(&pool, &owner).await?;

    let resp = app_for(&ctx, "intruder")
        .oneshot(empty_get(&format!("/{}/messages", task_id.as_str())))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn non_owner_delete_task_returns_404() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = UserId::new(format!("owner-{}", uuid::Uuid::new_v4()));
    let (_context_id, task_id) = seed_owned_task(&pool, &owner).await?;

    let resp = app_for(&ctx, "intruder")
        .oneshot(empty_delete(&format!("/{}", task_id.as_str())))
        .await?;
    assert_eq!(resp.status().as_u16(), 404, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn owner_can_delete_own_task() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = UserId::new(format!("owner-{}", uuid::Uuid::new_v4()));
    let (_context_id, task_id) = seed_owned_task(&pool, &owner).await?;

    let resp = app_for(&ctx, owner.as_str())
        .oneshot(empty_delete(&format!("/{}", task_id.as_str())))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

async fn seed_user_task(
    pool: &DbPool,
    user: &UserId,
    context_id: &ContextId,
    status: &str,
    age_minutes: i32,
) -> anyhow::Result<TaskId> {
    let task_id = TaskId::generate();
    sqlx::query(
        "INSERT INTO agent_tasks (task_id, context_id, user_id, status, agent_name, created_at) \
         VALUES ($1, $2, $3, $4, 'filter-agent', now() - make_interval(mins => $5))",
    )
    .bind(task_id.as_str())
    .bind(context_id.as_str())
    .bind(user.as_str())
    .bind(status)
    .bind(age_minutes)
    .execute(pool.pool().as_ref())
    .await?;
    Ok(task_id)
}

#[tokio::test]
async fn status_filter_applies_before_the_page_limit() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = UserId::new(format!("filter-{}", uuid::Uuid::new_v4().simple()));
    let (context_id, _) = seed_owned_task(&pool, &owner).await?;
    let completed = seed_user_task(&pool, &owner, &context_id, "TASK_STATE_COMPLETED", 60).await?;
    for age in 1..=3 {
        seed_user_task(&pool, &owner, &context_id, "TASK_STATE_WORKING", age).await?;
    }

    let resp = app_for(&ctx, owner.as_str())
        .oneshot(empty_get("/?status=TASK_STATE_COMPLETED&limit=2"))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await?;
    let tasks: Vec<serde_json::Value> = serde_json::from_slice(&bytes)?;
    let ids: Vec<&str> = tasks.iter().filter_map(|t| t["id"].as_str()).collect();
    assert_eq!(ids, vec![completed.as_str()]);
    Ok(())
}

#[tokio::test]
async fn oversized_limit_is_clamped_to_the_page_maximum() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = UserId::new(format!("clamp-{}", uuid::Uuid::new_v4().simple()));
    let (context_id, _) = seed_owned_task(&pool, &owner).await?;
    sqlx::query(
        "INSERT INTO agent_tasks (task_id, context_id, user_id, status, agent_name) SELECT \
         'clamp-' || $3 || '-' || g, $1, $2, 'TASK_STATE_WORKING', 'clamp-agent' FROM \
         generate_series(1, 501) AS g",
    )
    .bind(context_id.as_str())
    .bind(owner.as_str())
    .bind(owner.as_str())
    .execute(pool.pool().as_ref())
    .await?;

    let resp = app_for(&ctx, owner.as_str())
        .oneshot(empty_get("/?limit=4294967295"))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024).await?;
    let tasks: Vec<serde_json::Value> = serde_json::from_slice(&bytes)?;
    assert_eq!(tasks.len(), 500);
    Ok(())
}
