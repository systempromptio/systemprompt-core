use crate::models::a2a::AgentAuthentication;
use serde::{Deserialize, Serialize};

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
