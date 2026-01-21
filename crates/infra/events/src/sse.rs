use axum::response::sse::Event;
use serde::Serialize;
use systemprompt_models::{
    A2AEvent, AgUiEvent, AnalyticsEvent, CliOutputEvent, ContextEvent, SystemEvent,
};

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

impl CliOutputEvent {
    pub fn to_sse_event(&self) -> Event {
        Event::default()
            .event("cli")
            .json_data(self)
            .unwrap_or_else(|_| Event::default().event("cli").data("{}"))
    }
}
