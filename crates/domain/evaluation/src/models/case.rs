//! Evaluation case model captured from sampled traffic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use systemprompt_identifiers::{AiRequestId, EvalCaseId, UserId};

/// Provider-neutral reconstruction of an AI request.
///
/// Built from `ai_request_messages` plus the request row's model/provider
/// columns — never from the provider-specific wire body — so it can be
/// replayed through any configured provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalPrompt {
    pub messages: Vec<CanonicalMessage>,
    pub system_prompt: Option<String>,
    pub offered_tools: Option<Value>,
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct EvalCase {
    pub id: EvalCaseId,
    pub name: String,
    pub prompt_body: Value,
    pub source_ai_request_id: Option<AiRequestId>,
    pub expectation: Option<String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub repair_hint: Option<String>,
    pub canonical_messages: Option<Value>,
    pub system_prompt: Option<String>,
    pub offered_tools: Option<Value>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub prepared_body_sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewCaseParams {
    pub name: String,
    pub prompt: CanonicalPrompt,
    pub source_ai_request_id: Option<AiRequestId>,
    pub expectation: Option<String>,
    pub tags: Vec<String>,
    pub created_by: UserId,
    pub prepared_body_sha256: Option<String>,
}
