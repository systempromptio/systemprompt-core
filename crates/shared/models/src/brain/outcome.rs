//! Outcome signals: what happened after facts were injected.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AiRequestId, BrainFactId, BrainRecallId};

use super::recall::RecallAudit;

pub const CITED_IN_RESPONSE: &str = "cited_in_response";
pub const USER_CORRECTED_FACT: &str = "user_corrected_fact";

/// One observed outcome of one injected fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrainOutcome {
    pub recall_id: BrainRecallId,
    pub fact_id: BrainFactId,
    pub signal: String,
    pub value: f64,
    pub observed_at: DateTime<Utc>,
}

/// What an outcome signal is shown: the gateway request, its recall and,
/// when the response was tapped, the assistant's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeObservation {
    pub ai_request_id: AiRequestId,
    pub recall: RecallAudit,
    pub response_text: Option<String>,
    pub observed_at: DateTime<Utc>,
}
