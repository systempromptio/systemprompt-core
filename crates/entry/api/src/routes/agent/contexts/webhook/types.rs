use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, UserId};
use systemprompt_models::{A2AEvent, AgUiEvent};

#[derive(Debug, Deserialize, Serialize)]
pub struct WebhookRequest {
    pub event_type: String,
    pub entity_id: String,
    pub context_id: ContextId,
    pub user_id: UserId,
    #[serde(default)]
    pub step_data: Option<serde_json::Value>,
    #[serde(default)]
    pub task_data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct AgUiBroadcastRequest {
    #[serde(flatten)]
    pub event: AgUiEvent,
    pub user_id: UserId,
}

#[derive(Debug, Deserialize)]
pub struct A2ABroadcastRequest {
    #[serde(flatten)]
    pub event: A2AEvent,
    pub user_id: UserId,
}

#[derive(Debug)]
pub struct AgUiWebhookData {
    pub event_name: String,
    pub payload: serde_json::Value,
}
