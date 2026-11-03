//! Provider-neutral text embedding request and response types.
//!
//! An [`EmbeddingRequest`] names a model and the texts to embed; the provider
//! answers with one vector per input, in input order, plus the token usage it
//! billed. The shapes are the intersection of what the `OpenAI` `/embeddings`,
//! Vertex AI `:predict` and Gemini `:batchEmbedContents` endpoints return, so
//! a caller never sees a vendor body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub inputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

impl EmbeddingRequest {
    pub fn new(model: impl Into<String>, inputs: Vec<String>) -> Self {
        Self {
            model: model.into(),
            inputs,
            dimensions: None,
            provider: None,
        }
    }

    #[must_use]
    pub const fn with_dimensions(mut self, dimensions: u32) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    #[must_use]
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    #[must_use]
    pub fn provider(&self) -> Option<&str> {
        self.provider.as_deref().filter(|p| !p.is_empty())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    pub input_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub model: String,
    pub vectors: Vec<Vec<f32>>,
    #[serde(default)]
    pub usage: EmbeddingUsage,
}

impl EmbeddingResponse {
    #[must_use]
    pub fn dimensions(&self) -> Option<usize> {
        self.vectors.first().map(Vec::len)
    }
}
