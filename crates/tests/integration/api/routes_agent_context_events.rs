//! `POST /{id}/events` forwarding through `contexts_router`.
//!
//! Seeds a user-owned context and drives all three `ContextEvent` arms
//! (AG-UI, A2A, System) past ownership validation into the event router, which
//! returns zero broadcasts with no subscribers. The foreign-user and unknown
//! context arms drive the ownership-403 branch.

use axum::Extension;
use systemprompt_api::routes::contexts_router;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_models::a2a::TaskState;
use systemprompt_models::{
    A2AEventBuilder, AgUiEventBuilder, ContextEvent, RequestContext, SystemEventBuilder,
};
use systemprompt_test_fixtures::{seed_user_row, seed_user_session, unique_user_id};
use tower::ServiceExt;

use super::common::{json_post, setup_ctx};

async fn seed_context(pool: &DbPool) -> anyhow::Result<(UserId, ContextId)> {
    let user_id = unique_user_id("evt");
    let session_id = SessionId::generate();
    let email = format!("{}@evt.invalid", user_id.as_str());
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
    .bind("evt-context")
    .execute(handle.as_ref())
    .await?;
    Ok((user_id, context_id))
}

fn request_context_for(user: &UserId) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("evt-agent"),
    )
    .with_actor(Actor::user(user.clone()))
}

fn app(ctx: &systemprompt_runtime::AppContext, user: &UserId) -> axum::Router {
    contexts_router()
        .with_state(ctx.clone())
        .layer(Extension(request_context_for(user)))
}

#[tokio::test]
async fn forward_agui_event_succeeds() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, context_id) = seed_context(&pool).await?;

    let event = ContextEvent::AgUi(AgUiEventBuilder::run_started(
        context_id.clone(),
        TaskId::new("evt-task"),
        None,
    ));
    let uri = format!("/{}/events", context_id.as_str());
    let resp = app(&ctx, &user_id)
        .oneshot(json_post(&uri, serde_json::to_value(&event)?))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn forward_a2a_event_succeeds() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, context_id) = seed_context(&pool).await?;

    let event = ContextEvent::A2A(Box::new(A2AEventBuilder::task_status_update(
        TaskId::new("evt-task"),
        context_id.clone(),
        TaskState::Working,
        Some("working".to_owned()),
    )));
    let uri = format!("/{}/events", context_id.as_str());
    let resp = app(&ctx, &user_id)
        .oneshot(json_post(&uri, serde_json::to_value(&event)?))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn forward_system_event_succeeds() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (user_id, context_id) = seed_context(&pool).await?;

    let event = ContextEvent::System(SystemEventBuilder::heartbeat());
    let uri = format!("/{}/events", context_id.as_str());
    let resp = app(&ctx, &user_id)
        .oneshot(json_post(&uri, serde_json::to_value(&event)?))
        .await?;
    assert_eq!(resp.status().as_u16(), 200, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn forward_event_foreign_user_returns_403() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (_owner, context_id) = seed_context(&pool).await?;
    let intruder = unique_user_id("evt-intruder");

    let event = ContextEvent::System(SystemEventBuilder::heartbeat());
    let uri = format!("/{}/events", context_id.as_str());
    let resp = app(&ctx, &intruder)
        .oneshot(json_post(&uri, serde_json::to_value(&event)?))
        .await?;
    assert_eq!(resp.status().as_u16(), 403, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn forward_event_unknown_context_returns_403() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let user_id = unique_user_id("evt-nobody");
    let missing = ContextId::generate();

    let event = ContextEvent::System(SystemEventBuilder::heartbeat());
    let uri = format!("/{}/events", missing.as_str());
    let resp = app(&ctx, &user_id)
        .oneshot(json_post(&uri, serde_json::to_value(&event)?))
        .await?;
    assert_eq!(resp.status().as_u16(), 403, "{}", resp.status());
    Ok(())
}
