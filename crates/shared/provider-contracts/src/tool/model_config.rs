//! Per-tool model override declared by an MCP server or an agent config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ToolModelConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
}

impl ToolModelConfig {
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: Some(provider.into()),
            model: Some(model.into()),
            max_output_tokens: None,
        }
    }

    pub const fn with_max_output_tokens(mut self, tokens: u32) -> Self {
        self.max_output_tokens = Some(tokens);
        self
    }

    pub const fn is_empty(&self) -> bool {
        self.provider.is_none() && self.model.is_none() && self.max_output_tokens.is_none()
    }

    pub fn merge_with(&self, other: &Self) -> Self {
        Self {
            provider: other.provider.as_ref().or(self.provider.as_ref()).cloned(),
            model: other.model.as_ref().or(self.model.as_ref()).cloned(),
            max_output_tokens: other.max_output_tokens.or(self.max_output_tokens),
        }
    }
}
