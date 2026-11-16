//! The server-sent-event response a conferred stream handler returns.
//!
//! The handler subscribes through its handed `AnalyticsSubscription` and
//! passes the [`SubscribedStream`] here; frames arrive pre-rendered, the
//! response sends the standard heartbeat, and dropping the response drops the
//! subscription, which unregisters it from the broadcaster. A subscription the
//! broadcaster refuses at the per-user cap answers [`subscription_refused`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::convert::Infallible;
use std::time::Duration;

use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_traits::events::{SseFrame, SubscribedStream};

use super::ApiHttpError;

pub const HEARTBEAT_JSON: &str = r#"{"type":"heartbeat"}"#;
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

#[must_use]
pub fn standard_keep_alive() -> KeepAlive {
    KeepAlive::new()
        .interval(HEARTBEAT_INTERVAL)
        .event(Event::default().event("heartbeat").data(HEARTBEAT_JSON))
}

pub fn sse_response(req: &RequestContext, sub: SubscribedStream, label: &'static str) -> Response {
    tracing::info!(user_id = %req.user_id(), stream = label, "SSE stream ready");
    let frames = futures::stream::unfold(sub, |mut sub| async move {
        let frame = sub.frames.recv().await?;
        Some((Ok::<Event, Infallible>(frame_event(frame)), sub))
    });
    Sse::new(frames)
        .keep_alive(standard_keep_alive())
        .into_response()
}

#[must_use]
pub fn subscription_refused(req: &RequestContext, label: &'static str) -> Response {
    tracing::warn!(
        user_id = %req.user_id(),
        stream = label,
        "SSE stream rejected: per-user connection cap reached"
    );
    ApiHttpError::from(ApiError::rate_limited(
        "Per-user stream connection limit reached",
    ))
    .into_response()
}

#[must_use]
pub fn frame_event(frame: SseFrame) -> Event {
    let mut event = Event::default();
    if let Some(name) = frame.event {
        event = event.event(name);
    }
    if let Some(id) = frame.id {
        event = event.id(id);
    }
    event.data(frame.data)
}
