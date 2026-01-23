use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{A2AEvent, AgUiEvent, SystemEvent};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "protocol", content = "event")]
pub enum ContextEvent {
    #[serde(rename = "agui")]
    AgUi(AgUiEvent),
    #[serde(rename = "a2a")]
    A2A(Box<A2AEvent>),
    #[serde(rename = "system")]
    System(SystemEvent),
}

impl ContextEvent {
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::AgUi(e) => e.timestamp(),
            Self::A2A(e) => e.as_ref().timestamp(),
            Self::System(e) => e.timestamp(),
        }
    }
}

impl From<AgUiEvent> for ContextEvent {
    fn from(event: AgUiEvent) -> Self {
        Self::AgUi(event)
    }
}

impl From<A2AEvent> for ContextEvent {
    fn from(event: A2AEvent) -> Self {
        Self::A2A(Box::new(event))
    }
}

impl From<SystemEvent> for ContextEvent {
    fn from(event: SystemEvent) -> Self {
        Self::System(event)
    }
}
