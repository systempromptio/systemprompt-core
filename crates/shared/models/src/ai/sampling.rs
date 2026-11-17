//! Provider sampling configuration (temperature, top-p, penalties).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelPreferences {
    pub hints: Vec<ModelHint>,
    pub cost_priority: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ModelHint {
    ModelId(String),
    Category(String),
    Provider(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SamplingParams {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

/// Explicit reasoning controls for a request. A field left unset keeps the
/// provider's per-model default.
///
/// `thinking_budget_tokens` is the thinking budget of the Anthropic and Gemini
/// wires; `effort` is the reasoning effort of the `OpenAI` wires, which derive
/// one from the budget when only the budget is set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasoningParams {
    pub effort: Option<ReasoningEffort>,
    pub thinking_budget_tokens: Option<u32>,
}

impl ReasoningParams {
    pub const fn effort(effort: ReasoningEffort) -> Self {
        Self {
            effort: Some(effort),
            thinking_budget_tokens: None,
        }
    }

    pub const fn with_thinking_budget(mut self, budget_tokens: u32) -> Self {
        self.thinking_budget_tokens = Some(budget_tokens);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: String,
    pub model: String,
    pub max_output_tokens: u32,
}

impl ProviderConfig {
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        max_output_tokens: u32,
    ) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            max_output_tokens,
        }
    }
}
