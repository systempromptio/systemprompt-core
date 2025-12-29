use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SystemEventType {
    ContextCreated,
    ContextUpdated,
    ContextDeleted,
    ContextsSnapshot,
    Connected,
    Heartbeat,
}

impl SystemEventType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ContextCreated => super::constants::system::CONTEXT_CREATED,
            Self::ContextUpdated => super::constants::system::CONTEXT_UPDATED,
            Self::ContextDeleted => super::constants::system::CONTEXT_DELETED,
            Self::ContextsSnapshot => super::constants::system::CONTEXTS_SNAPSHOT,
            Self::Connected => super::constants::system::CONNECTED,
            Self::Heartbeat => super::constants::system::HEARTBEAT,
        }
    }
}
