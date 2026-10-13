//! A2A JSON-RPC request and response envelopes.
//!
//! Defines the typed parameter shapes for each A2A 1.0 method, the
//! [`A2aJsonRpcRequest`] wire envelope and its
//! [`A2aJsonRpcRequest::parse_request`] dispatcher into [`A2aRequestParams`],
//! the typed [`A2aResponse`] result variants, and the protocol error payloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::a2a::jsonrpc::{JSON_RPC_VERSION_2_0, JsonRpcResponse, RequestId};
use crate::models::a2a::{AgentCard, Message, Task, TaskState};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::a2a::{SendMessageResult, methods};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    pub message: Message,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<SendMessageConfiguration>,
    // JSON: A2A `SendMessageRequest.metadata` is spec-defined as a free-form object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageConfiguration {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_output_modes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_length: Option<u32>,
    #[serde(default)]
    pub return_immediately: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetTaskParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    pub id: TaskId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_length: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    pub id: TaskId,
    // JSON: A2A `CancelTaskRequest.metadata` is spec-defined as a free-form object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeToTaskParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    pub id: TaskId,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetExtendedAgentCardParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListTasksParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_id: Option<ContextId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_timestamp_after: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_artifacts: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListTasksResult {
    pub tasks: Vec<Task>,
    pub next_page_token: String,
    pub page_size: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_size: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct A2aRequest {
    pub method: String,
    // JSON: JSON-RPC 2.0 `params` is method-specific until dispatched.
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum A2aResponse {
    SendMessage(SendMessageResponse),
    GetTask(GetTaskResponse),
    ListTasks(ListTasksResponse),
    CancelTask(CancelTaskResponse),
    GetExtendedAgentCard(GetExtendedAgentCardResponse),
}

pub type SendMessageResponse = JsonRpcResponse<SendMessageResult>;
pub type GetTaskResponse = JsonRpcResponse<Task>;
pub type ListTasksResponse = JsonRpcResponse<ListTasksResult>;
pub type CancelTaskResponse = JsonRpcResponse<Task>;
pub type GetExtendedAgentCardResponse = JsonRpcResponse<AgentCard>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct A2aJsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    // JSON: JSON-RPC 2.0 `params` is method-specific until dispatched.
    #[serde(default)]
    pub params: serde_json::Value,
    pub id: RequestId,
}

impl A2aJsonRpcRequest {
    pub fn parse_request(&self) -> Result<A2aRequestParams, A2aParseError> {
        match self.method.as_str() {
            methods::SEND_MESSAGE => Ok(A2aRequestParams::SendMessage(self.parse_params()?)),
            methods::SEND_STREAMING_MESSAGE => {
                Ok(A2aRequestParams::SendStreamingMessage(self.parse_params()?))
            },
            methods::GET_TASK => Ok(A2aRequestParams::GetTask(self.parse_params()?)),
            methods::LIST_TASKS => Ok(A2aRequestParams::ListTasks(self.parse_params()?)),
            methods::CANCEL_TASK => Ok(A2aRequestParams::CancelTask(self.parse_params()?)),
            methods::SUBSCRIBE_TO_TASK => {
                Ok(A2aRequestParams::SubscribeToTask(self.parse_params()?))
            },
            methods::GET_EXTENDED_AGENT_CARD => {
                Ok(A2aRequestParams::GetExtendedAgentCard(self.parse_params()?))
            },
            _ => Err(A2aParseError::UnsupportedMethod {
                method: self.method.clone(),
            }),
        }
    }

    fn parse_params<T: serde::de::DeserializeOwned>(&self) -> Result<T, A2aParseError> {
        let params = if self.params.is_null() {
            serde_json::Value::Object(serde_json::Map::new())
        } else {
            self.params.clone()
        };
        serde_json::from_value(params).map_err(|e| A2aParseError::InvalidParams {
            method: self.method.clone(),
            source: Arc::new(e),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum A2aRequestParams {
    SendMessage(SendMessageParams),
    SendStreamingMessage(SendMessageParams),
    GetTask(GetTaskParams),
    ListTasks(ListTasksParams),
    CancelTask(CancelTaskParams),
    SubscribeToTask(SubscribeToTaskParams),
    GetExtendedAgentCard(GetExtendedAgentCardParams),
}

impl A2aRequestParams {
    #[must_use]
    pub fn tenant(&self) -> Option<&str> {
        match self {
            Self::SendMessage(p) | Self::SendStreamingMessage(p) => p.tenant.as_deref(),
            Self::GetTask(p) => p.tenant.as_deref(),
            Self::ListTasks(p) => p.tenant.as_deref(),
            Self::CancelTask(p) => p.tenant.as_deref(),
            Self::SubscribeToTask(p) => p.tenant.as_deref(),
            Self::GetExtendedAgentCard(p) => p.tenant.as_deref(),
        }
    }
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum A2aParseError {
    #[error("Unsupported method: {method}")]
    UnsupportedMethod { method: String },

    #[error("Invalid parameters for method '{method}': {source}")]
    InvalidParams {
        method: String,
        #[source]
        source: Arc<serde_json::Error>,
    },
}

fn ok<T>(result: T, id: RequestId) -> JsonRpcResponse<T> {
    JsonRpcResponse {
        jsonrpc: JSON_RPC_VERSION_2_0.to_owned(),
        result: Some(result),
        error: None,
        id,
    }
}

impl A2aResponse {
    pub fn send_message(result: SendMessageResult, id: RequestId) -> Self {
        Self::SendMessage(ok(result, id))
    }

    pub fn get_task(task: Task, id: RequestId) -> Self {
        Self::GetTask(ok(task, id))
    }

    pub fn list_tasks(result: ListTasksResult, id: RequestId) -> Self {
        Self::ListTasks(ok(result, id))
    }

    pub fn cancel_task(task: Task, id: RequestId) -> Self {
        Self::CancelTask(ok(task, id))
    }

    pub fn extended_agent_card(card: AgentCard, id: RequestId) -> Self {
        Self::GetExtendedAgentCard(ok(card, id))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskNotFoundError {
    pub task_id: TaskId,
    pub message: String,
    pub code: i32,
    // JSON: JSON-RPC 2.0 error `data` — the spec allows any value.
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TaskNotCancelableError {
    pub task_id: TaskId,
    pub state: TaskState,
    pub message: String,
    pub code: i32,
    // JSON: JSON-RPC 2.0 error `data` — the spec allows any value.
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UnsupportedOperationError {
    pub operation: String,
    pub message: String,
    pub code: i32,
    // JSON: JSON-RPC 2.0 error `data` — the spec allows any value.
    pub data: serde_json::Value,
}
