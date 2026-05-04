//! Top-level AG-UI event enum and the convenience builder.

mod builder;

pub use builder::AgUiEventBuilder;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::agui::{
    AgUiEventType, CustomPayload, MessagesSnapshotPayload, RunErrorPayload, RunFinishedPayload,
    RunStartedPayload, StateDeltaPayload, StateSnapshotPayload, StepFinishedPayload,
    StepStartedPayload, TextMessageContentPayload, TextMessageEndPayload, TextMessageStartPayload,
    ToolCallArgsPayload, ToolCallEndPayload, ToolCallResultPayload, ToolCallStartPayload,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgUiEvent {
    RunStarted {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: RunStartedPayload,
    },
    RunFinished {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: RunFinishedPayload,
    },
    RunError {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: RunErrorPayload,
    },
    StepStarted {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: StepStartedPayload,
    },
    StepFinished {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: StepFinishedPayload,
    },
    TextMessageStart {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: TextMessageStartPayload,
    },
    TextMessageContent {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: TextMessageContentPayload,
    },
    TextMessageEnd {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: TextMessageEndPayload,
    },
    ToolCallStart {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: ToolCallStartPayload,
    },
    ToolCallArgs {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: ToolCallArgsPayload,
    },
    ToolCallEnd {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: ToolCallEndPayload,
    },
    ToolCallResult {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: ToolCallResultPayload,
    },
    StateSnapshot {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: StateSnapshotPayload,
    },
    StateDelta {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: StateDeltaPayload,
    },
    MessagesSnapshot {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: MessagesSnapshotPayload,
    },
    Custom {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: Box<CustomPayload>,
    },
}

impl AgUiEvent {
    pub const fn event_type(&self) -> AgUiEventType {
        match self {
            Self::RunStarted { .. } => AgUiEventType::RunStarted,
            Self::RunFinished { .. } => AgUiEventType::RunFinished,
            Self::RunError { .. } => AgUiEventType::RunError,
            Self::StepStarted { .. } => AgUiEventType::StepStarted,
            Self::StepFinished { .. } => AgUiEventType::StepFinished,
            Self::TextMessageStart { .. } => AgUiEventType::TextMessageStart,
            Self::TextMessageContent { .. } => AgUiEventType::TextMessageContent,
            Self::TextMessageEnd { .. } => AgUiEventType::TextMessageEnd,
            Self::ToolCallStart { .. } => AgUiEventType::ToolCallStart,
            Self::ToolCallArgs { .. } => AgUiEventType::ToolCallArgs,
            Self::ToolCallEnd { .. } => AgUiEventType::ToolCallEnd,
            Self::ToolCallResult { .. } => AgUiEventType::ToolCallResult,
            Self::StateSnapshot { .. } => AgUiEventType::StateSnapshot,
            Self::StateDelta { .. } => AgUiEventType::StateDelta,
            Self::MessagesSnapshot { .. } => AgUiEventType::MessagesSnapshot,
            Self::Custom { .. } => AgUiEventType::Custom,
        }
    }

    pub const fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::RunStarted { timestamp, .. }
            | Self::RunFinished { timestamp, .. }
            | Self::RunError { timestamp, .. }
            | Self::StepStarted { timestamp, .. }
            | Self::StepFinished { timestamp, .. }
            | Self::TextMessageStart { timestamp, .. }
            | Self::TextMessageContent { timestamp, .. }
            | Self::TextMessageEnd { timestamp, .. }
            | Self::ToolCallStart { timestamp, .. }
            | Self::ToolCallArgs { timestamp, .. }
            | Self::ToolCallEnd { timestamp, .. }
            | Self::ToolCallResult { timestamp, .. }
            | Self::StateSnapshot { timestamp, .. }
            | Self::StateDelta { timestamp, .. }
            | Self::MessagesSnapshot { timestamp, .. }
            | Self::Custom { timestamp, .. } => *timestamp,
        }
    }
}
