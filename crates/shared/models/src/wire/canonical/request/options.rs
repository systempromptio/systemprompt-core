//! Tool, thinking, response-format and search options of a canonical request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct CanonicalTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Debug, Clone)]
pub enum CanonicalToolChoice {
    Auto,
    Any,
    None,
    Required,
    Tool(String),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ThinkingConfig {
    pub enabled: bool,
    pub budget_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum ResponseFormat {
    JsonObject,
    JsonSchema {
        name: String,
        // JSON: JSON Schema document for structured output (OpenAI / Gemini / Anthropic).
        schema: Value,
        strict: bool,
    },
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

impl ReasoningEffort {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchConfig {
    pub max_uses: Option<u32>,
    pub context_size: Option<String>,
    pub urls: Vec<String>,
}
