//! Durable gateway accounting records.
use crate::services::gateway::audit::payload::PayloadCapture;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AiRequestId, UserId};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Completion {
    pub usage: [u32; 6],
    pub cost: i64,
    pub latency: i32,
    pub upstream_latency: Option<i32>,
    pub payload: PayloadCapture,
    pub assistant: Option<String>,
    pub tools: Vec<(String, String, String)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Receipt {
    pub request_id: AiRequestId,
    pub user_id: UserId,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completion: Option<Completion>,
    pub failure: Option<String>,
}

impl Receipt {
    pub(crate) fn pending(request_id: AiRequestId, user_id: UserId) -> Self {
        Self {
            request_id,
            user_id,
            created_at: chrono::Utc::now(),
            completion: None,
            failure: None,
        }
    }
}
