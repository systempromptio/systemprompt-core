use serde::{Deserialize, Serialize};
use serde_json::Value;
use systemprompt_identifiers::{AiToolCallId, ContextId, MessageId, TaskId};

use super::JsonPatchOperation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStartedPayload {
    pub thread_id: ContextId,
    pub run_id: TaskId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunFinishedPayload {
    pub thread_id: ContextId,
    pub run_id: TaskId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunErrorPayload {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepStartedPayload {
    pub step_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepFinishedPayload {
    pub step_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextMessageStartPayload {
    pub message_id: MessageId,
    pub role: MessageRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextMessageContentPayload {
    pub message_id: MessageId,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextMessageEndPayload {
    pub message_id: MessageId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallStartPayload {
    pub tool_call_id: AiToolCallId,
    pub tool_call_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_message_id: Option<MessageId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallArgsPayload {
    pub tool_call_id: AiToolCallId,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallEndPayload {
    pub tool_call_id: AiToolCallId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResultPayload {
    pub message_id: MessageId,
    pub tool_call_id: AiToolCallId,
    pub content: Value,
    pub role: MessageRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateSnapshotPayload {
    pub snapshot: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateDeltaPayload {
    pub delta: Vec<JsonPatchOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagesSnapshotPayload {
    pub messages: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactCustomPayload {
    pub artifact: crate::a2a::Artifact,
    pub task_id: TaskId,
    pub context_id: ContextId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionStepCustomPayload {
    pub step: crate::execution::ExecutionStep,
    pub context_id: ContextId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillLoadedCustomPayload {
    pub skill_id: systemprompt_identifiers::SkillId,
    pub skill_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericCustomPayload {
    pub name: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "name", content = "value", rename_all = "snake_case")]
pub enum CustomPayload {
    Artifact(Box<ArtifactCustomPayload>),
    ExecutionStep(Box<ExecutionStepCustomPayload>),
    SkillLoaded(SkillLoadedCustomPayload),
    #[serde(untagged)]
    Generic(GenericCustomPayload),
}
