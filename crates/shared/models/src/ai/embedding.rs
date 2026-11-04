//! Provider-neutral text embedding request and response types.
//!
//! An [`EmbeddingRequest`] names a model and the texts to embed; the provider
//! answers with one vector per input, in input order, plus the token usage it
//! billed. The shapes are the intersection of what the `OpenAI` `/embeddings`,
//! Vertex AI `:predict` and Gemini `:batchEmbedContents` endpoints return, so
//! a caller never sees a vendor body. A provider that reports no usage has
//! its input counted by the caller and the usage marked `estimated`, so a
//! priced call is never settled at zero.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ModelId, ProviderId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub model: ModelId,
    pub inputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderId>,
}

impl EmbeddingRequest {
    pub fn new(model: impl Into<ModelId>, inputs: Vec<String>) -> Self {
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
    pub fn with_provider(mut self, provider: impl Into<ProviderId>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    #[must_use]
    pub fn provider(&self) -> Option<&ProviderId> {
        self.provider.as_ref().filter(|p| !p.as_str().is_empty())
    }

    #[must_use]
    pub fn with_inputs(&self, inputs: Vec<String>) -> Self {
        Self {
            model: self.model.clone(),
            inputs,
            dimensions: self.dimensions,
            provider: self.provider.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    pub input_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub estimated: bool,
}

impl EmbeddingUsage {
    #[must_use]
    pub const fn reported(input_tokens: u32, total_tokens: u32) -> Self {
        Self {
            input_tokens,
            total_tokens,
            estimated: false,
        }
    }

    #[must_use]
    pub const fn from_estimate(input_tokens: u32) -> Self {
        Self {
            input_tokens,
            total_tokens: input_tokens,
            estimated: true,
        }
    }

    #[must_use]
    pub const fn summed_with(self, other: Self) -> Self {
        Self {
            input_tokens: self.input_tokens.saturating_add(other.input_tokens),
            total_tokens: self.total_tokens.saturating_add(other.total_tokens),
            estimated: self.estimated || other.estimated,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub model: ModelId,
    pub vectors: Vec<Vec<f32>>,
    #[serde(default)]
    pub usage: EmbeddingUsage,
}

impl EmbeddingResponse {
    #[must_use]
    pub const fn empty(model: ModelId) -> Self {
        Self {
            model,
            vectors: Vec::new(),
            usage: EmbeddingUsage {
                input_tokens: 0,
                total_tokens: 0,
                estimated: false,
            },
        }
    }

    #[must_use]
    pub fn dimensions(&self) -> Option<usize> {
        self.vectors.first().map(Vec::len)
    }
}
