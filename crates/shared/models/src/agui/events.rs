use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use systemprompt_identifiers::{AiToolCallId, ContextId, MessageId, SkillId, TaskId};

use super::{
    AgUiEventType, ArtifactCustomPayload, CustomPayload, ExecutionStepCustomPayload,
    JsonPatchOperation, MessageRole, MessagesSnapshotPayload, RunErrorPayload, RunFinishedPayload,
    RunStartedPayload, SkillLoadedCustomPayload, StateDeltaPayload, StateSnapshotPayload,
    StepFinishedPayload, StepStartedPayload, TextMessageContentPayload, TextMessageEndPayload,
    TextMessageStartPayload, ToolCallArgsPayload, ToolCallEndPayload, ToolCallResultPayload,
    ToolCallStartPayload,
};
use crate::a2a::Artifact;
use crate::execution::ExecutionStep;

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

    pub fn to_sse(&self) -> Result<Event, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(Event::default().data(json))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AgUiEventBuilder;

impl AgUiEventBuilder {
    pub fn run_started(context_id: ContextId, task_id: TaskId, input: Option<Value>) -> AgUiEvent {
        AgUiEvent::RunStarted {
            timestamp: Utc::now(),
            payload: RunStartedPayload {
                thread_id: context_id,
                run_id: task_id,
                input,
            },
        }
    }

    pub fn run_finished(
        context_id: ContextId,
        task_id: TaskId,
        result: Option<Value>,
    ) -> AgUiEvent {
        AgUiEvent::RunFinished {
            timestamp: Utc::now(),
            payload: RunFinishedPayload {
                thread_id: context_id,
                run_id: task_id,
                result,
            },
        }
    }

    pub fn run_error(message: String, code: Option<String>) -> AgUiEvent {
        AgUiEvent::RunError {
            timestamp: Utc::now(),
            payload: RunErrorPayload { message, code },
        }
    }

    pub fn step_started(step_name: impl Into<String>) -> AgUiEvent {
        AgUiEvent::StepStarted {
            timestamp: Utc::now(),
            payload: StepStartedPayload {
                step_name: step_name.into(),
            },
        }
    }

    pub fn step_finished(step_name: impl Into<String>) -> AgUiEvent {
        AgUiEvent::StepFinished {
            timestamp: Utc::now(),
            payload: StepFinishedPayload {
                step_name: step_name.into(),
            },
        }
    }

    pub fn text_message_start(message_id: impl Into<String>, role: MessageRole) -> AgUiEvent {
        AgUiEvent::TextMessageStart {
            timestamp: Utc::now(),
            payload: TextMessageStartPayload {
                message_id: MessageId::new(message_id),
                role,
            },
        }
    }

    pub fn text_message_content(
        message_id: impl Into<String>,
        delta: impl Into<String>,
    ) -> AgUiEvent {
        AgUiEvent::TextMessageContent {
            timestamp: Utc::now(),
            payload: TextMessageContentPayload {
                message_id: MessageId::new(message_id),
                delta: delta.into(),
            },
        }
    }

    pub fn text_message_end(message_id: impl Into<String>) -> AgUiEvent {
        AgUiEvent::TextMessageEnd {
            timestamp: Utc::now(),
            payload: TextMessageEndPayload {
                message_id: MessageId::new(message_id),
            },
        }
    }

    pub fn tool_call_start(
        tool_call_id: impl Into<String>,
        tool_call_name: impl Into<String>,
        parent_message_id: Option<String>,
    ) -> AgUiEvent {
        AgUiEvent::ToolCallStart {
            timestamp: Utc::now(),
            payload: ToolCallStartPayload {
                tool_call_id: AiToolCallId::new(tool_call_id),
                tool_call_name: tool_call_name.into(),
                parent_message_id: parent_message_id.map(MessageId::new),
            },
        }
    }

    pub fn tool_call_args(tool_call_id: impl Into<String>, delta: impl Into<String>) -> AgUiEvent {
        AgUiEvent::ToolCallArgs {
            timestamp: Utc::now(),
            payload: ToolCallArgsPayload {
                tool_call_id: AiToolCallId::new(tool_call_id),
                delta: delta.into(),
            },
        }
    }

    pub fn tool_call_end(tool_call_id: impl Into<String>) -> AgUiEvent {
        AgUiEvent::ToolCallEnd {
            timestamp: Utc::now(),
            payload: ToolCallEndPayload {
                tool_call_id: AiToolCallId::new(tool_call_id),
            },
        }
    }

    pub fn tool_call_result(
        message_id: impl Into<String>,
        tool_call_id: impl Into<String>,
        content: Value,
    ) -> AgUiEvent {
        AgUiEvent::ToolCallResult {
            timestamp: Utc::now(),
            payload: ToolCallResultPayload {
                message_id: MessageId::new(message_id),
                tool_call_id: AiToolCallId::new(tool_call_id),
                content,
                role: MessageRole::Tool,
            },
        }
    }

    pub fn state_snapshot(snapshot: Value) -> AgUiEvent {
        AgUiEvent::StateSnapshot {
            timestamp: Utc::now(),
            payload: StateSnapshotPayload { snapshot },
        }
    }

    pub fn state_delta(operations: Vec<JsonPatchOperation>) -> AgUiEvent {
        AgUiEvent::StateDelta {
            timestamp: Utc::now(),
            payload: StateDeltaPayload { delta: operations },
        }
    }

    pub fn messages_snapshot(messages: Vec<Value>) -> AgUiEvent {
        AgUiEvent::MessagesSnapshot {
            timestamp: Utc::now(),
            payload: MessagesSnapshotPayload { messages },
        }
    }

    pub fn custom(payload: CustomPayload) -> AgUiEvent {
        AgUiEvent::Custom {
            timestamp: Utc::now(),
            payload: Box::new(payload),
        }
    }

    pub fn artifact(artifact: Artifact, task_id: TaskId, context_id: ContextId) -> AgUiEvent {
        Self::custom(CustomPayload::Artifact(Box::new(ArtifactCustomPayload {
            artifact,
            task_id,
            context_id,
        })))
    }

    pub fn execution_step(step: ExecutionStep, context_id: ContextId) -> AgUiEvent {
        Self::custom(CustomPayload::ExecutionStep(Box::new(
            ExecutionStepCustomPayload { step, context_id },
        )))
    }

    pub fn skill_loaded(
        skill_id: SkillId,
        skill_name: String,
        description: Option<String>,
        task_id: Option<TaskId>,
    ) -> AgUiEvent {
        Self::custom(CustomPayload::SkillLoaded(SkillLoadedCustomPayload {
            skill_id,
            skill_name,
            description,
            task_id,
        }))
    }
}
