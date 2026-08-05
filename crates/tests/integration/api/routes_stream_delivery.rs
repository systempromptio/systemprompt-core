//! SSE stream registration, the per-user connection cap, and event delivery.
//!
//! The existing stream suite asserts `(200..600).contains(&status)`, which
//! cannot fail, and never reads the response body — so `StreamWithGuard` is
//! constructed but never polled, and the cap-rejection branch never runs. These
//! tests register connections directly on the process-wide broadcaster to reach
//! the cap, and read a frame off a live stream so the guard-wrapped receiver is
//! actually driven.

use std::time::Duration;

use axum::Extension;
use axum::body::Body;
use axum::http::Response;
use http_body_util::BodyExt;
use systemprompt_api::routes::stream::stream_router;
use systemprompt_events::{AGUI_BROADCASTER, Broadcaster};
use systemprompt_identifiers::{ConnectionId, ContextId, TaskId, UserId};
use systemprompt_models::agui::AgUiEventBuilder;
use tokio::sync::mpsc;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{empty_get, request_context, setup_ctx};

fn fresh_user() -> UserId {
    UserId::new(format!("streamer-{}", Uuid::new_v4().simple()))
}

async fn open_agui(user: &UserId) -> anyhow::Result<Response<Body>> {
    let (_pool, ctx) = setup_ctx().await?;
    let mut req_ctx = request_context("ignored");
    req_ctx.auth.actor.user_id = user.clone();
    Ok(stream_router(&ctx)
        .layer(Extension(req_ctx))
        .oneshot(empty_get("/agui"))
        .await?)
}

#[tokio::test]
async fn an_opened_stream_delivers_an_event_broadcast_to_that_user() -> anyhow::Result<()> {
    let user = fresh_user();
    let resp = open_agui(&user).await?;
    assert_eq!(resp.status().as_u16(), 200, "{:?}", resp.status());
    let mut body = resp.into_body();

    let event = AgUiEventBuilder::run_started(ContextId::generate(), TaskId::generate(), None);
    let delivered = AGUI_BROADCASTER.broadcast(&user, event).await;
    assert_eq!(
        delivered, 1,
        "the open connection must be the one recipient of its own user's event"
    );

    let frame = tokio::time::timeout(Duration::from_secs(5), body.frame())
        .await
        .expect("a broadcast event must reach the open stream")
        .expect("the stream must yield a frame rather than ending")?;
    let bytes = frame.into_data().expect("an SSE frame carries data");
    let text = String::from_utf8_lossy(&bytes).into_owned();

    assert!(
        text.contains("RUN_STARTED"),
        "the delivered frame must carry the broadcast event: {text}"
    );
    Ok(())
}

#[tokio::test]
async fn an_event_for_another_user_is_not_delivered() -> anyhow::Result<()> {
    let user = fresh_user();
    let stranger = fresh_user();
    let resp = open_agui(&user).await?;
    assert_eq!(resp.status().as_u16(), 200);

    let event = AgUiEventBuilder::run_started(ContextId::generate(), TaskId::generate(), None);
    let delivered = AGUI_BROADCASTER.broadcast(&stranger, event).await;

    assert_eq!(
        delivered, 0,
        "a stream is per-user; another user's event must never fan out to it"
    );
    Ok(())
}

#[tokio::test]
async fn a_user_at_the_connection_cap_is_refused_a_new_stream() -> anyhow::Result<()> {
    let user = fresh_user();
    // Hold the senders: dropping them would let the broadcaster reap the
    // registrations and the cap would no longer be in force.
    let mut held = Vec::new();
    for _ in 0..10 {
        let (tx, rx) = mpsc::channel(8);
        let accepted = AGUI_BROADCASTER
            .register(&user, &ConnectionId::generate(), tx)
            .await;
        assert!(accepted, "registrations below the cap must be accepted");
        held.push(rx);
    }

    let resp = open_agui(&user).await?;

    assert_eq!(
        resp.status().as_u16(),
        429,
        "a user at the per-connection cap must be refused rather than allowed to exhaust the \
         server's fan-out slots"
    );
    Ok(())
}

#[tokio::test]
async fn closing_a_stream_releases_its_connection_slot() -> anyhow::Result<()> {
    let user = fresh_user();
    let resp = open_agui(&user).await?;
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(AGUI_BROADCASTER.connection_count(&user).await, 1);

    // Dropping the response drops the guard that deregisters the connection.
    drop(resp);
    for _ in 0..50 {
        if AGUI_BROADCASTER.connection_count(&user).await == 0 {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("a dropped stream must release its slot, or a reconnecting client hits the cap");
}

#[tokio::test]
async fn the_a2a_stream_registers_on_its_own_broadcaster() -> anyhow::Result<()> {
    let user = fresh_user();
    let (_pool, ctx) = setup_ctx().await?;
    let mut req_ctx = request_context("ignored");
    req_ctx.auth.actor.user_id = user.clone();

    let resp = stream_router(&ctx)
        .layer(Extension(req_ctx))
        .oneshot(empty_get("/a2a"))
        .await?;

    assert_eq!(resp.status().as_u16(), 200, "{:?}", resp.status());
    assert_eq!(
        AGUI_BROADCASTER.connection_count(&user).await,
        0,
        "an A2A subscriber must not appear on the AgUI broadcaster"
    );
    Ok(())
}
