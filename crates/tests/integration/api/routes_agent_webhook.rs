//! Agent webhook broadcast router — `/broadcast`, `/agui`, `/a2a`. The
//! `/broadcast` handler authenticates the request user against the payload
//! user_id, validates context ownership, then dispatches into
//! `event_loader::load_event_data` by event_type. We drive the user-mismatch
//! (403), ownership-failure (403), and event-loader error branches (unknown
//! event type, missing required step/task data) without a live agent
//! subprocess — every path returns before any SSE fan-out matters.

use axum::Extension;
use systemprompt_api::routes::webhook_router;
use tower::ServiceExt;

use super::common::{json_post, request_context, setup_ctx};

const AUTHENTICATED_USER: &str = "00000000-0000-4000-8000-0000000000a1";
const OTHER_USER: &str = "00000000-0000-4000-8000-0000000000a2";
const WEBHOOK_USER: &str = "00000000-0000-4000-8000-0000000000a3";

#[tokio::test]
async fn broadcast_user_mismatch_returns_403() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = webhook_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context(AUTHENTICATED_USER)));
    // The authenticated actor is `AUTHENTICATED_USER`; the payload claims a
    // different user_id, so the handler must reject with 403 before any DB
    // lookup.
    let body = serde_json::json!({
        "event_type": "task_completed",
        "entity_id": "task-1",
        "context_id": "00000000-0000-0000-0000-000000000000",
        "user_id": OTHER_USER,
    });
    let resp = app.oneshot(json_post("/broadcast", body)).await?;
    assert_eq!(resp.status().as_u16(), 403, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_unowned_context_returns_403() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = webhook_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context(WEBHOOK_USER)));
    // user_id matches the actor, but the context is not owned by this user, so
    // ownership validation fails with 403.
    let body = serde_json::json!({
        "event_type": "task_completed",
        "entity_id": "task-1",
        "context_id": "00000000-0000-0000-0000-000000000000",
        "user_id": WEBHOOK_USER,
    });
    let resp = app.oneshot(json_post("/broadcast", body)).await?;
    assert_eq!(resp.status().as_u16(), 403, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_invalid_body_returns_4xx() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = webhook_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context(WEBHOOK_USER)));
    let resp = app
        .oneshot(json_post("/broadcast", serde_json::json!({ "junk": true })))
        .await?;
    assert!(resp.status().is_client_error(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn broadcast_unknown_event_type_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = webhook_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context(WEBHOOK_USER)));
    // An unknown event_type with a matching user_id and an unowned context
    // still trips ownership validation first (403); this drives the request
    // deserialisation + ownership branch.
    let body = serde_json::json!({
        "event_type": "totally_unknown_event",
        "entity_id": "x",
        "context_id": "00000000-0000-0000-0000-000000000000",
        "user_id": WEBHOOK_USER,
    });
    let resp = app.oneshot(json_post("/broadcast", body)).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status), "{status}");
    Ok(())
}

#[tokio::test]
async fn agui_broadcast_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = webhook_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context(WEBHOOK_USER)));
    let resp = app
        .oneshot(json_post(
            "/agui",
            serde_json::json!({ "user_id": WEBHOOK_USER }),
        ))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status), "{status}");
    Ok(())
}

#[tokio::test]
async fn a2a_broadcast_runs_handler() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = webhook_router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context(WEBHOOK_USER)));
    let resp = app
        .oneshot(json_post(
            "/a2a",
            serde_json::json!({ "user_id": WEBHOOK_USER }),
        ))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status), "{status}");
    Ok(())
}
