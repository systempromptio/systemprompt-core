//! Evaluation case model captured from sampled traffic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AiRequestId, EvalCaseId, ModelId, ProviderId, UserId};
use systemprompt_traits::{TraceMessage, TraceSample};

/// Provider-neutral reconstruction of an AI request.
///
/// Built from the stored conversation turns plus the request row's
/// model/provider columns — never from the provider-specific wire body — so
/// it can be replayed through any configured provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalPrompt {
    pub messages: Vec<TraceMessage>,
    pub system_prompt: Option<String>,
    // JSON: provider tool schemas are an MCP/provider protocol boundary
    pub offered_tools: Option<serde_json::Value>,
    pub provider: ProviderId,
    pub model: ModelId,
}

impl CanonicalPrompt {
    #[must_use]
    pub fn from_sample(sample: &TraceSample) -> Self {
        Self {
            messages: sample.messages.clone(),
            system_prompt: sample.system_prompt_override.clone(),
            offered_tools: sample.offered_tools.clone(),
            provider: sample.provider.clone(),
            model: sample.model.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvalCase {
    pub id: EvalCaseId,
    pub name: String,
    pub prompt: CanonicalPrompt,
    pub source_ai_request_id: Option<AiRequestId>,
    pub expectation: Option<String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub repair_hint: Option<String>,
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
