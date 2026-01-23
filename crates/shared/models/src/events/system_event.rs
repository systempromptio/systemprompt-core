use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ContextId;

use super::payloads::system::{
    ConnectedPayload, ContextCreatedPayload, ContextDeletedPayload, ContextSummary,
    ContextUpdatedPayload, ContextsSnapshotPayload,
};
use super::system_event_type::SystemEventType;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SystemEvent {
    ContextCreated {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: ContextCreatedPayload,
    },
    ContextUpdated {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: ContextUpdatedPayload,
    },
    ContextDeleted {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: ContextDeletedPayload,
    },
    ContextsSnapshot {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: ContextsSnapshotPayload,
    },
    Connected {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: ConnectedPayload,
    },
    Heartbeat {
        timestamp: DateTime<Utc>,
    },
}

impl SystemEvent {
    pub const fn event_type(&self) -> SystemEventType {
        match self {
            Self::ContextCreated { .. } => SystemEventType::ContextCreated,
            Self::ContextUpdated { .. } => SystemEventType::ContextUpdated,
            Self::ContextDeleted { .. } => SystemEventType::ContextDeleted,
            Self::ContextsSnapshot { .. } => SystemEventType::ContextsSnapshot,
            Self::Connected { .. } => SystemEventType::Connected,
            Self::Heartbeat { .. } => SystemEventType::Heartbeat,
        }
    }

    pub const fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::ContextCreated { timestamp, .. }
            | Self::ContextUpdated { timestamp, .. }
            | Self::ContextDeleted { timestamp, .. }
            | Self::ContextsSnapshot { timestamp, .. }
            | Self::Connected { timestamp, .. }
            | Self::Heartbeat { timestamp } => *timestamp,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SystemEventBuilder;

impl SystemEventBuilder {
    pub fn context_created(context_id: ContextId, name: String) -> SystemEvent {
        SystemEvent::ContextCreated {
            timestamp: Utc::now(),
            payload: ContextCreatedPayload { context_id, name },
        }
    }

    pub fn context_updated(context_id: ContextId, name: Option<String>) -> SystemEvent {
        SystemEvent::ContextUpdated {
            timestamp: Utc::now(),
            payload: ContextUpdatedPayload { context_id, name },
        }
    }

    pub fn context_deleted(context_id: ContextId) -> SystemEvent {
        SystemEvent::ContextDeleted {
            timestamp: Utc::now(),
            payload: ContextDeletedPayload { context_id },
        }
    }

    pub fn contexts_snapshot(contexts: Vec<ContextSummary>) -> SystemEvent {
        SystemEvent::ContextsSnapshot {
            timestamp: Utc::now(),
            payload: ContextsSnapshotPayload { contexts },
        }
    }

    pub fn connected(connection_id: String) -> SystemEvent {
        SystemEvent::Connected {
            timestamp: Utc::now(),
            payload: ConnectedPayload { connection_id },
        }
    }

    pub fn heartbeat() -> SystemEvent {
        SystemEvent::Heartbeat {
            timestamp: Utc::now(),
        }
    }
}
