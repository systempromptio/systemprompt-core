//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde_json::Value;
use systemprompt_identifiers::AiRequestId;

use super::case::{CanonicalMessage, CanonicalPrompt};

#[derive(Debug, Clone, Default)]
pub struct SampleFilter {
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub ids: Option<Vec<String>>,
    pub limit: i64,
}

impl SampleFilter {
    #[must_use]
    pub fn with_limit(limit: i64) -> Self {
        Self {
            limit,
            ..Self::default()
        }
    }

    #[must_use]
    pub const fn since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    #[must_use]
    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    #[must_use]
    pub fn ids(mut self, ids: Vec<String>) -> Self {
        self.ids = Some(ids);
        self
    }
}

/// A completed production request hydrated with everything the judge needs.
#[derive(Debug, Clone)]
pub struct SampledRequest {
    pub ai_request_id: AiRequestId,
    pub provider: String,
    pub model: String,
    pub system_prompt_override: Option<String>,
    pub messages: Vec<CanonicalMessage>,
    pub response_text: Option<String>,
    pub offered_tools: Option<Value>,
    pub prepared_body_sha256: Option<String>,
    pub latency_ms: Option<i32>,
    pub cost_microdollars: i64,
    pub created_at: DateTime<Utc>,
}

impl SampledRequest {
    #[must_use]
    pub fn canonical_prompt(&self) -> CanonicalPrompt {
        CanonicalPrompt {
            messages: self.messages.clone(),
            system_prompt: self.system_prompt_override.clone(),
            offered_tools: self.offered_tools.clone(),
            provider: self.provider.clone(),
            model: self.model.clone(),
        }
    }
}
