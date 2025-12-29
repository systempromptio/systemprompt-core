use axum::response::sse::Event;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};

use super::a2a_event_type::A2AEventType;
use super::payloads::a2a::{
    AgentMessagePayload, ArtifactCreatedPayload, ArtifactUpdatedPayload, AuthRequiredPayload,
    InputRequiredPayload, JsonRpcErrorPayload, JsonRpcResponsePayload, TaskStatusUpdatePayload,
    TaskSubmittedPayload,
};
use crate::a2a::{Artifact, TaskState};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum A2AEvent {
    TaskSubmitted {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: TaskSubmittedPayload,
    },
    TaskStatusUpdate {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: TaskStatusUpdatePayload,
    },
    ArtifactCreated {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: Box<ArtifactCreatedPayload>,
    },
    ArtifactUpdated {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: ArtifactUpdatedPayload,
    },
    AgentMessage {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: AgentMessagePayload,
    },
    InputRequired {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: InputRequiredPayload,
    },
    AuthRequired {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: AuthRequiredPayload,
    },
    JsonRpcResponse {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: JsonRpcResponsePayload,
    },
    JsonRpcError {
        timestamp: DateTime<Utc>,
        #[serde(flatten)]
        payload: JsonRpcErrorPayload,
    },
}

impl A2AEvent {
    pub const fn event_type(&self) -> A2AEventType {
        match self {
            Self::TaskSubmitted { .. } => A2AEventType::TaskSubmitted,
            Self::TaskStatusUpdate { .. } => A2AEventType::TaskStatusUpdate,
            Self::ArtifactCreated { .. } => A2AEventType::ArtifactCreated,
            Self::ArtifactUpdated { .. } => A2AEventType::ArtifactUpdated,
            Self::AgentMessage { .. } => A2AEventType::AgentMessage,
            Self::InputRequired { .. } => A2AEventType::InputRequired,
            Self::AuthRequired { .. } => A2AEventType::AuthRequired,
            Self::JsonRpcResponse { .. } => A2AEventType::JsonRpcResponse,
            Self::JsonRpcError { .. } => A2AEventType::JsonRpcError,
        }
    }

    pub const fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::TaskSubmitted { timestamp, .. }
            | Self::TaskStatusUpdate { timestamp, .. }
            | Self::ArtifactCreated { timestamp, .. }
            | Self::ArtifactUpdated { timestamp, .. }
            | Self::AgentMessage { timestamp, .. }
            | Self::InputRequired { timestamp, .. }
            | Self::AuthRequired { timestamp, .. }
            | Self::JsonRpcResponse { timestamp, .. }
            | Self::JsonRpcError { timestamp, .. } => *timestamp,
        }
    }

    pub fn to_sse(&self) -> Result<Event, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(Event::default().data(json))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct A2AEventBuilder;

impl A2AEventBuilder {
    pub fn task_submitted(
        task_id: TaskId,
        context_id: ContextId,
        agent_name: String,
        input: Option<serde_json::Value>,
    ) -> A2AEvent {
        A2AEvent::TaskSubmitted {
            timestamp: Utc::now(),
            payload: TaskSubmittedPayload {
                task_id,
                context_id,
                agent_name,
                input,
            },
        }
    }

    pub fn task_status_update(
        task_id: TaskId,
        context_id: ContextId,
        state: TaskState,
        message: Option<String>,
    ) -> A2AEvent {
        A2AEvent::TaskStatusUpdate {
            timestamp: Utc::now(),
            payload: TaskStatusUpdatePayload {
                task_id,
                context_id,
                state,
                message,
            },
        }
    }

    pub fn artifact_created(
        task_id: TaskId,
        context_id: ContextId,
        artifact: Artifact,
    ) -> A2AEvent {
        A2AEvent::ArtifactCreated {
            timestamp: Utc::now(),
            payload: Box::new(ArtifactCreatedPayload {
                task_id,
                context_id,
                artifact,
            }),
        }
    }

    pub fn artifact_updated(payload: ArtifactUpdatedPayload) -> A2AEvent {
        A2AEvent::ArtifactUpdated {
            timestamp: Utc::now(),
            payload,
        }
    }

    pub fn agent_message(
        task_id: TaskId,
        context_id: ContextId,
        message_id: MessageId,
        content: String,
    ) -> A2AEvent {
        A2AEvent::AgentMessage {
            timestamp: Utc::now(),
            payload: AgentMessagePayload {
                task_id,
                context_id,
                message_id,
                content,
            },
        }
    }

    pub fn input_required(task_id: TaskId, context_id: ContextId, prompt: String) -> A2AEvent {
        A2AEvent::InputRequired {
            timestamp: Utc::now(),
            payload: InputRequiredPayload {
                task_id,
                context_id,
                prompt,
            },
        }
    }

    pub fn auth_required(task_id: TaskId, context_id: ContextId, auth_url: String) -> A2AEvent {
        A2AEvent::AuthRequired {
            timestamp: Utc::now(),
            payload: AuthRequiredPayload {
                task_id,
                context_id,
                auth_url,
            },
        }
    }

    pub fn json_rpc_response(id: serde_json::Value, result: serde_json::Value) -> A2AEvent {
        A2AEvent::JsonRpcResponse {
            timestamp: Utc::now(),
            payload: JsonRpcResponsePayload { id, result },
        }
    }

    pub fn json_rpc_error(
        id: serde_json::Value,
        code: i32,
        message: String,
        data: Option<serde_json::Value>,
    ) -> A2AEvent {
        A2AEvent::JsonRpcError {
            timestamp: Utc::now(),
            payload: JsonRpcErrorPayload {
                id,
                code,
                message,
                data,
            },
        }
    }
}
