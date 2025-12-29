use super::jsonrpc::{JsonRpcResponse, RequestId};
use super::{AgentAuthentication, AgentCard, Artifact, Message, Task, TaskState, TaskStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MessageSendParams {
    pub message: Message,
    pub configuration: Option<MessageSendConfiguration>,
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MessageSendConfiguration {
    pub accepted_output_modes: Option<Vec<String>>,
    pub history_length: Option<u32>,
    pub push_notification_config: Option<PushNotificationConfig>,
    pub blocking: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PushNotificationConfig {
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    pub endpoint: String,
    pub headers: Option<serde_json::Map<String, serde_json::Value>>,
    pub url: String,
    pub token: Option<String>,
    pub authentication: Option<AgentAuthentication>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskQueryParams {
    pub id: String,
    pub history_length: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskIdParams {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct A2aRequest {
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum A2aResponse {
    SendMessage(SendMessageResponse),
    GetTask(GetTaskResponse),
    CancelTask(CancelTaskResponse),
    GetAuthenticatedExtendedCard(GetAuthenticatedExtendedCardResponse),
    SendStreamingMessage(SendStreamingMessageResponse),
}

pub type SendMessageResponse = JsonRpcResponse<Task>;
pub type GetTaskResponse = JsonRpcResponse<Task>;
pub type CancelTaskResponse = JsonRpcResponse<Task>;
pub type GetAuthenticatedExtendedCardResponse = JsonRpcResponse<AgentCard>;
pub type SendStreamingMessageResponse = JsonRpcResponse<Task>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct A2aJsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
    pub id: RequestId,
}

impl A2aJsonRpcRequest {
    pub fn parse_request(&self) -> Result<A2aRequestParams, A2aParseError> {
        match self.method.as_str() {
            "message/send" => {
                let params: MessageSendParams = serde_json::from_value(self.params.clone())
                    .map_err(|e| A2aParseError::InvalidParams {
                        method: self.method.clone(),
                        error: e.to_string(),
                    })?;
                Ok(A2aRequestParams::SendMessage(params))
            },
            "tasks/get" => {
                let params: TaskQueryParams =
                    serde_json::from_value(self.params.clone()).map_err(|e| {
                        A2aParseError::InvalidParams {
                            method: self.method.clone(),
                            error: e.to_string(),
                        }
                    })?;
                Ok(A2aRequestParams::GetTask(params))
            },
            "tasks/cancel" => {
                let params: TaskIdParams =
                    serde_json::from_value(self.params.clone()).map_err(|e| {
                        A2aParseError::InvalidParams {
                            method: self.method.clone(),
                            error: e.to_string(),
                        }
                    })?;
                Ok(A2aRequestParams::CancelTask(params))
            },
            "agent/getAuthenticatedExtendedCard" => {
                let params: serde_json::Value = serde_json::from_value(self.params.clone())
                    .map_err(|e| A2aParseError::InvalidParams {
                        method: self.method.clone(),
                        error: e.to_string(),
                    })?;
                Ok(A2aRequestParams::GetAuthenticatedExtendedCard(params))
            },
            "message/stream" => {
                let params: MessageSendParams = serde_json::from_value(self.params.clone())
                    .map_err(|e| A2aParseError::InvalidParams {
                        method: self.method.clone(),
                        error: e.to_string(),
                    })?;
                Ok(A2aRequestParams::SendStreamingMessage(params))
            },
            "tasks/resubscribe" => {
                let params: TaskResubscriptionRequest = serde_json::from_value(self.params.clone())
                    .map_err(|e| A2aParseError::InvalidParams {
                        method: self.method.clone(),
                        error: e.to_string(),
                    })?;
                Ok(A2aRequestParams::TaskResubscription(params))
            },
            "tasks/pushNotificationConfig/set" => {
                let params: SetTaskPushNotificationConfigRequest =
                    serde_json::from_value(self.params.clone()).map_err(|e| {
                        A2aParseError::InvalidParams {
                            method: self.method.clone(),
                            error: e.to_string(),
                        }
                    })?;
                Ok(A2aRequestParams::SetTaskPushNotificationConfig(params))
            },
            "tasks/pushNotificationConfig/get" => {
                let params: GetTaskPushNotificationConfigRequest =
                    serde_json::from_value(self.params.clone()).map_err(|e| {
                        A2aParseError::InvalidParams {
                            method: self.method.clone(),
                            error: e.to_string(),
                        }
                    })?;
                Ok(A2aRequestParams::GetTaskPushNotificationConfig(params))
            },
            "tasks/pushNotificationConfig/list" => {
                let params: ListTaskPushNotificationConfigRequest =
                    serde_json::from_value(self.params.clone()).map_err(|e| {
                        A2aParseError::InvalidParams {
                            method: self.method.clone(),
                            error: e.to_string(),
                        }
                    })?;
                Ok(A2aRequestParams::ListTaskPushNotificationConfig(params))
            },
            "tasks/pushNotificationConfig/delete" => {
                let params: DeleteTaskPushNotificationConfigRequest =
                    serde_json::from_value(self.params.clone()).map_err(|e| {
                        A2aParseError::InvalidParams {
                            method: self.method.clone(),
                            error: e.to_string(),
                        }
                    })?;
                Ok(A2aRequestParams::DeleteTaskPushNotificationConfig(params))
            },
            _ => Err(A2aParseError::UnsupportedMethod {
                method: self.method.clone(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum A2aRequestParams {
    SendMessage(MessageSendParams),
    GetTask(TaskQueryParams),
    CancelTask(TaskIdParams),
    GetAuthenticatedExtendedCard(serde_json::Value),
    SendStreamingMessage(MessageSendParams),
    TaskResubscription(TaskResubscriptionRequest),
    SetTaskPushNotificationConfig(SetTaskPushNotificationConfigRequest),
    GetTaskPushNotificationConfig(GetTaskPushNotificationConfigRequest),
    ListTaskPushNotificationConfig(ListTaskPushNotificationConfigRequest),
    DeleteTaskPushNotificationConfig(DeleteTaskPushNotificationConfigRequest),
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum A2aParseError {
    #[error("Unsupported method: {method}")]
    UnsupportedMethod { method: String },

    #[error("Invalid parameters for method '{method}': {error}")]
    InvalidParams { method: String, error: String },
}

impl A2aResponse {
    pub fn send_message(task: Task, id: RequestId) -> Self {
        Self::SendMessage(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(task),
            error: None,
        })
    }

    pub fn get_task(task: Task, id: RequestId) -> Self {
        Self::GetTask(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(task),
            error: None,
        })
    }

    pub fn cancel_task(task: Task, id: RequestId) -> Self {
        Self::CancelTask(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(task),
            error: None,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusUpdateEvent {
    pub kind: String,
    pub task_id: String,
    pub context_id: String,
    pub status: TaskStatus,
    #[serde(rename = "final")]
    pub is_final: bool,
}

impl TaskStatusUpdateEvent {
    pub fn new(
        task_id: impl Into<String>,
        context_id: impl Into<String>,
        status: TaskStatus,
        is_final: bool,
    ) -> Self {
        Self {
            kind: "status-update".to_string(),
            task_id: task_id.into(),
            context_id: context_id.into(),
            status,
            is_final,
        }
    }

    pub fn to_jsonrpc_response(&self) -> serde_json::Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "result": self
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactUpdateEvent {
    pub kind: String,
    pub task_id: String,
    pub context_id: String,
    pub artifact: Artifact,
    #[serde(rename = "final")]
    pub is_final: bool,
}

impl TaskArtifactUpdateEvent {
    pub fn new(
        task_id: impl Into<String>,
        context_id: impl Into<String>,
        artifact: Artifact,
        is_final: bool,
    ) -> Self {
        Self {
            kind: "artifact-update".to_string(),
            task_id: task_id.into(),
            context_id: context_id.into(),
            artifact,
            is_final,
        }
    }

    pub fn to_jsonrpc_response(&self) -> serde_json::Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "result": self
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskNotFoundError {
    pub task_id: String,
    pub message: String,
    pub code: i32,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskNotCancelableError {
    pub task_id: String,
    pub state: TaskState,
    pub message: String,
    pub code: i32,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UnsupportedOperationError {
    pub operation: String,
    pub message: String,
    pub code: i32,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PushNotificationNotSupportedError {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskPushNotificationConfig {
    pub id: String,
    pub push_notification_config: PushNotificationConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SetTaskPushNotificationConfigRequest {
    pub task_id: String,
    pub config: PushNotificationConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SetTaskPushNotificationConfigResponse {
    pub success: bool,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct GetTaskPushNotificationConfigRequest {
    pub task_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct GetTaskPushNotificationConfigResponse {
    pub config: Option<PushNotificationConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct GetTaskPushNotificationConfigParams {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DeleteTaskPushNotificationConfigRequest {
    pub task_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DeleteTaskPushNotificationConfigResponse {
    pub success: bool,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DeleteTaskPushNotificationConfigParams {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ListTaskPushNotificationConfigRequest {
    pub task_id: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ListTaskPushNotificationConfigResponse {
    pub configs: Vec<PushNotificationConfig>,
    pub total: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskResubscriptionRequest {
    pub task_id: String,
    pub config: PushNotificationConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskResubscriptionResponse {
    pub success: bool,
    pub message: Option<String>,
}
