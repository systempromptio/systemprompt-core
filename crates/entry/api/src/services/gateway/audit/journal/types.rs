//! Durable gateway accounting records.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::gateway::audit::payload::PayloadCapture;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AiRequestId, AiToolCallId, SessionId, UserId};

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

/// Usage a request consumed before it failed. The provider bills what it
/// streamed, so a truncated stream that reported a usage delta is settled
/// with that usage instead of at zero.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PartialUsage {
    pub usage: [u32; 6],
    pub cost: i64,
    pub latency: i32,
    pub upstream_latency: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Receipt {
    pub request_id: AiRequestId,
    pub user_id: UserId,
    /// Settling a completion bumps this session's AI counters. Defaulted so a
    /// journal written before 0.59.0 still replays on recovery.
    #[serde(default)]
    pub session_id: Option<SessionId>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completion: Option<Completion>,
    pub failure: Option<String>,
    #[serde(default)]
    pub partial: Option<PartialUsage>,
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

    pub(crate) fn pending(
        request_id: AiRequestId,
        user_id: UserId,
        session_id: Option<SessionId>,
    ) -> Self {
        Self {
            request_id,
            user_id,
            session_id,
            created_at: chrono::Utc::now(),
            completion: None,
            failure: None,
            partial: None,
            accounting_failure: None,
        }
    }
}
