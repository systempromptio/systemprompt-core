use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum A2AEventType {
    TaskSubmitted,
    TaskStatusUpdate,
    ArtifactCreated,
    ArtifactUpdated,
    AgentMessage,
    InputRequired,
    AuthRequired,
    JsonRpcResponse,
    JsonRpcError,
}

impl A2AEventType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::TaskSubmitted => super::constants::a2a::TASK_SUBMITTED,
            Self::TaskStatusUpdate => super::constants::a2a::TASK_STATUS_UPDATE,
            Self::ArtifactCreated => super::constants::a2a::ARTIFACT_CREATED,
            Self::ArtifactUpdated => super::constants::a2a::ARTIFACT_UPDATED,
            Self::AgentMessage => super::constants::a2a::AGENT_MESSAGE,
            Self::InputRequired => super::constants::a2a::INPUT_REQUIRED,
            Self::AuthRequired => super::constants::a2a::AUTH_REQUIRED,
            Self::JsonRpcResponse => super::constants::a2a::JSON_RPC_RESPONSE,
            Self::JsonRpcError => super::constants::a2a::JSON_RPC_ERROR,
        }
    }
}
