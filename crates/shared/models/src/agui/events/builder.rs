//! Convenience builder that stamps each [`super::AgUiEvent`] with the
//! current timestamp and assembles its inner payload.

use chrono::Utc;
use serde_json::Value;
use systemprompt_identifiers::{AiToolCallId, ContextId, MessageId, SkillId, TaskId};

use super::AgUiEvent;
use crate::a2a::Artifact;
use crate::agui::{
    ArtifactCustomPayload, CustomPayload, ExecutionStepCustomPayload, JsonPatchOperation,
    MessageRole, MessagesSnapshotPayload, RunErrorPayload, RunFinishedPayload, RunStartedPayload,
    SkillLoadedCustomPayload, StateDeltaPayload, StateSnapshotPayload, StepFinishedPayload,
    StepStartedPayload, TextMessageContentPayload, TextMessageEndPayload, TextMessageStartPayload,
    ToolCallArgsPayload, ToolCallEndPayload, ToolCallResultPayload, ToolCallStartPayload,
};
use crate::execution::ExecutionStep;

/// Stateless namespace for [`AgUiEvent`] constructors.
#[derive(Debug, Clone, Copy)]
pub struct AgUiEventBuilder;

impl AgUiEventBuilder {
    /// Build a `RunStarted` event for the given context / task pair.
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

    /// Build a `RunFinished` event with the optional final result.
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

    /// Build a `RunError` event.
    pub fn run_error(message: String, code: Option<String>) -> AgUiEvent {
        AgUiEvent::RunError {
            timestamp: Utc::now(),
            payload: RunErrorPayload { message, code },
        }
    }

    /// Build a `StepStarted` event tagged with `step_name`.
    pub fn step_started(step_name: impl Into<String>) -> AgUiEvent {
        AgUiEvent::StepStarted {
            timestamp: Utc::now(),
            payload: StepStartedPayload {
                step_name: step_name.into(),
            },
        }
    }

    /// Build a `StepFinished` event tagged with `step_name`.
    pub fn step_finished(step_name: impl Into<String>) -> AgUiEvent {
        AgUiEvent::StepFinished {
            timestamp: Utc::now(),
            payload: StepFinishedPayload {
                step_name: step_name.into(),
            },
        }
    }

    /// Build a `TextMessageStart` event for a freshly-opened streaming message.
    pub fn text_message_start(message_id: impl Into<String>, role: MessageRole) -> AgUiEvent {
        AgUiEvent::TextMessageStart {
            timestamp: Utc::now(),
            payload: TextMessageStartPayload {
                message_id: MessageId::new(message_id),
                role,
            },
        }
    }

    /// Build a `TextMessageContent` delta event.
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

    /// Build a `TextMessageEnd` event closing a streaming message.
    pub fn text_message_end(message_id: impl Into<String>) -> AgUiEvent {
        AgUiEvent::TextMessageEnd {
            timestamp: Utc::now(),
            payload: TextMessageEndPayload {
                message_id: MessageId::new(message_id),
            },
        }
    }

    /// Build a `ToolCallStart` event.
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

    /// Build a `ToolCallArgs` delta event.
    pub fn tool_call_args(tool_call_id: impl Into<String>, delta: impl Into<String>) -> AgUiEvent {
        AgUiEvent::ToolCallArgs {
            timestamp: Utc::now(),
            payload: ToolCallArgsPayload {
                tool_call_id: AiToolCallId::new(tool_call_id),
                delta: delta.into(),
            },
        }
    }

    /// Build a `ToolCallEnd` event.
    pub fn tool_call_end(tool_call_id: impl Into<String>) -> AgUiEvent {
        AgUiEvent::ToolCallEnd {
            timestamp: Utc::now(),
            payload: ToolCallEndPayload {
                tool_call_id: AiToolCallId::new(tool_call_id),
            },
        }
    }

    /// Build a `ToolCallResult` event carrying the resolved tool output.
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

    /// Build a `StateSnapshot` event.
    pub fn state_snapshot(snapshot: Value) -> AgUiEvent {
        AgUiEvent::StateSnapshot {
            timestamp: Utc::now(),
            payload: StateSnapshotPayload { snapshot },
        }
    }

    /// Build a `StateDelta` event from a list of JSON-patch operations.
    pub fn state_delta(operations: Vec<JsonPatchOperation>) -> AgUiEvent {
        AgUiEvent::StateDelta {
            timestamp: Utc::now(),
            payload: StateDeltaPayload { delta: operations },
        }
    }

    /// Build a `MessagesSnapshot` event.
    pub fn messages_snapshot(messages: Vec<Value>) -> AgUiEvent {
        AgUiEvent::MessagesSnapshot {
            timestamp: Utc::now(),
            payload: MessagesSnapshotPayload { messages },
        }
    }

    /// Build a `Custom` event wrapping `payload`.
    pub fn custom(payload: CustomPayload) -> AgUiEvent {
        AgUiEvent::Custom {
            timestamp: Utc::now(),
            payload: Box::new(payload),
        }
    }

    /// Build a custom event carrying an [`Artifact`].
    pub fn artifact(artifact: Artifact, task_id: TaskId, context_id: ContextId) -> AgUiEvent {
        Self::custom(CustomPayload::Artifact(Box::new(ArtifactCustomPayload {
            artifact,
            task_id,
            context_id,
        })))
    }

    /// Build a custom event carrying an [`ExecutionStep`].
    pub fn execution_step(step: ExecutionStep, context_id: ContextId) -> AgUiEvent {
        Self::custom(CustomPayload::ExecutionStep(Box::new(
            ExecutionStepCustomPayload { step, context_id },
        )))
    }

    /// Build a custom `SkillLoaded` event.
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
