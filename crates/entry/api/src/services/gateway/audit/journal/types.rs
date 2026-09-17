//! Durable gateway accounting records.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::gateway::audit::payload::PayloadCapture;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AiRequestId, AiToolCallId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CapturedToolCall {
    pub id: AiToolCallId,
    pub name: String,
    pub input: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Completion {
    pub usage: [u32; 6],
    pub cost: i64,
    pub latency: i32,
    pub upstream_latency: Option<i32>,
    #[serde(default)]
    pub finish_reason: Option<String>,
    pub payload: PayloadCapture,
    pub assistant: Option<String>,
    pub tools: Vec<CapturedToolCall>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Receipt {
    pub request_id: AiRequestId,
    pub user_id: UserId,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completion: Option<Completion>,
    pub failure: Option<String>,
    #[serde(default)]
    pub accounting_failure: Option<String>,
}

impl Receipt {
    pub(crate) fn storage_id(&self) -> AiRequestId {
        if self.accounting_failure.is_some() {
            AiRequestId::new(format!("accounting-failure:{}", self.request_id))
        } else {
            self.request_id.clone()
        }
    }

    pub(crate) fn pending(request_id: AiRequestId, user_id: UserId) -> Self {
        Self {
            request_id,
            user_id,
            created_at: chrono::Utc::now(),
            completion: None,
            failure: None,
            accounting_failure: None,
        }
    }
}
