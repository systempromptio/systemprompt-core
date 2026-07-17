//! A2A JSON-RPC request and response envelopes.
//!
//! Defines the typed parameter shapes for each A2A method, the
//! [`A2aJsonRpcRequest`] wire envelope and its
//! [`A2aJsonRpcRequest::parse_request`] dispatcher into [`A2aRequestParams`],
//! the [`A2aResponse`] result variants, and the protocol error payloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::push_notification::{
    DeleteTaskPushNotificationConfigRequest, GetTaskPushNotificationConfigRequest,
    ListTaskPushNotificationConfigRequest, PushNotificationConfig,
    SetTaskPushNotificationConfigRequest, TaskResubscriptionRequest,
};
use crate::models::a2a::jsonrpc::{JsonRpcResponse, RequestId};
use crate::models::a2a::{AgentCard, Task, TaskState};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::TaskId;
use systemprompt_models::a2a::methods;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MessageSendParams {
    pub message: crate::models::a2a::Message,
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
pub struct TaskQueryParams {
    pub id: TaskId,
    pub history_length: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskIdParams {
    pub id: TaskId,
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
            methods::SEND_MESSAGE => Ok(A2aRequestParams::SendMessage(self.parse_params()?)),
            methods::GET_TASK => Ok(A2aRequestParams::GetTask(self.parse_params()?)),
            methods::CANCEL_TASK => Ok(A2aRequestParams::CancelTask(self.parse_params()?)),
            methods::GET_EXTENDED_AGENT_CARD => Ok(A2aRequestParams::GetAuthenticatedExtendedCard(
                self.parse_params()?,
            )),
            methods::SEND_STREAMING_MESSAGE => {
                Ok(A2aRequestParams::SendStreamingMessage(self.parse_params()?))
            },
            methods::SUBSCRIBE_TO_TASK => {
                Ok(A2aRequestParams::TaskResubscription(self.parse_params()?))
            },
            methods::CREATE_TASK_PUSH_NOTIFICATION_CONFIG => Ok(
                A2aRequestParams::SetTaskPushNotificationConfig(self.parse_params()?),
            ),
            methods::GET_TASK_PUSH_NOTIFICATION_CONFIG => Ok(
                A2aRequestParams::GetTaskPushNotificationConfig(self.parse_params()?),
            ),
            methods::LIST_TASK_PUSH_NOTIFICATION_CONFIGS => Ok(
                A2aRequestParams::ListTaskPushNotificationConfig(self.parse_params()?),
            ),
            methods::DELETE_TASK_PUSH_NOTIFICATION_CONFIG => Ok(
                A2aRequestParams::DeleteTaskPushNotificationConfig(self.parse_params()?),
            ),
            _ => Err(A2aParseError::UnsupportedMethod {
                method: self.method.clone(),
            }),
        }
    }

    fn parse_params<T: serde::de::DeserializeOwned>(&self) -> Result<T, A2aParseError> {
        serde_json::from_value(self.params.clone()).map_err(|e| A2aParseError::InvalidParams {
            method: self.method.clone(),
            error: e.to_string(),
        })
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
            jsonrpc: "2.0".to_owned(),
            id,
            result: Some(task),
            error: None,
        })
    }

    pub fn get_task(task: Task, id: RequestId) -> Self {
        Self::GetTask(JsonRpcResponse {
            jsonrpc: "2.0".to_owned(),
            id,
            result: Some(task),
            error: None,
        })
    }

    pub fn cancel_task(task: Task, id: RequestId) -> Self {
        Self::CancelTask(JsonRpcResponse {
            jsonrpc: "2.0".to_owned(),
            id,
            result: Some(task),
            error: None,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskNotFoundError {
    pub task_id: TaskId,
    pub message: String,
    pub code: i32,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskNotCancelableError {
    pub task_id: TaskId,
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
