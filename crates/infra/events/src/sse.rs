//! Conversion from typed event payloads into Server-Sent Event records.
//!
//! Each event kind in [`systemprompt_models`] renders itself once as a
//! pre-rendered [`SseFrame`]: the payload serialized to JSON as the `data:`
//! line, with an explicit `event:` name where the kind needs disambiguation
//! (e.g. `CliOutputEvent`). The `axum` record is built from that frame, so a
//! frame subscriber and an `axum` stream see the same event.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::response::sse::Event;
use serde::Serialize;
use systemprompt_extension::router::frame_event;
use systemprompt_models::api::CliOutputEvent;
use systemprompt_models::{A2AEvent, AgUiEvent, AnalyticsEvent, ContextEvent, SystemEvent};
use systemprompt_traits::events::SseFrame;

/// An event payload that renders itself as one server-sent event.
pub trait ToSse {
    fn to_frame(&self) -> Result<SseFrame, serde_json::Error>;

    fn to_sse(&self) -> Result<Event, serde_json::Error> {
        self.to_frame().map(frame_event)
    }
}


fn serialize_to_frame<T: Serialize>(value: &T) -> Result<SseFrame, serde_json::Error> {
    Ok(SseFrame {
        event: None,
        id: None,
        data: serde_json::to_string(value)?,
    })
}

impl ToSse for AgUiEvent {
    fn to_frame(&self) -> Result<SseFrame, serde_json::Error> {
        serialize_to_frame(self)
    }
}

impl ToSse for A2AEvent {
    fn to_frame(&self) -> Result<SseFrame, serde_json::Error> {
        serialize_to_frame(self)
    }
}

impl ToSse for SystemEvent {
    fn to_frame(&self) -> Result<SseFrame, serde_json::Error> {
        serialize_to_frame(self)
    }
}

impl ToSse for ContextEvent {
    fn to_frame(&self) -> Result<SseFrame, serde_json::Error> {
        serialize_to_frame(self)
    }
}

impl ToSse for AnalyticsEvent {
    fn to_frame(&self) -> Result<SseFrame, serde_json::Error> {
        serialize_to_frame(self)
    }
}

impl ToSse for CliOutputEvent {
    fn to_frame(&self) -> Result<SseFrame, serde_json::Error> {
        Ok(SseFrame {
            event: Some("cli".to_owned()),
            id: None,
            data: serde_json::to_string(self)?,
        })
    }
}
