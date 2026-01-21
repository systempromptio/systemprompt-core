use axum::response::sse::Event;
use serde::Serialize;
use systemprompt_models::api::CliOutputEvent;
use systemprompt_models::{A2AEvent, AgUiEvent, AnalyticsEvent, ContextEvent, SystemEvent};

pub trait ToSse {
    fn to_sse(&self) -> Result<Event, serde_json::Error>;
}

fn serialize_to_sse<T: Serialize>(value: &T) -> Result<Event, serde_json::Error> {
    let json = serde_json::to_string(value)?;
    Ok(Event::default().data(json))
}

impl ToSse for AgUiEvent {
    fn to_sse(&self) -> Result<Event, serde_json::Error> {
        serialize_to_sse(self)
    }
}

impl ToSse for A2AEvent {
    fn to_sse(&self) -> Result<Event, serde_json::Error> {
        serialize_to_sse(self)
    }
}

impl ToSse for SystemEvent {
    fn to_sse(&self) -> Result<Event, serde_json::Error> {
        serialize_to_sse(self)
    }
}

impl ToSse for ContextEvent {
    fn to_sse(&self) -> Result<Event, serde_json::Error> {
        serialize_to_sse(self)
    }
}

impl ToSse for AnalyticsEvent {
    fn to_sse(&self) -> Result<Event, serde_json::Error> {
        serialize_to_sse(self)
    }
}

impl ToSse for CliOutputEvent {
    fn to_sse(&self) -> Result<Event, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(Event::default().event("cli").data(json))
    }
}
