//! Provider drivers and shared abstractions.
//!
//! Drivers live for Anthropic, `OpenAI`, Gemini (chat + image generation),
//! along with image provider abstractions. The internal
//! [`provider_trait::AiProvider`] trait is the dispatch surface
//! used by [`crate::AiService`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod anthropic;
pub mod canonical_bridge;
pub mod gemini;
mod gemini_image_mapping;
pub mod gemini_images;
pub mod http_client;
pub mod image_provider_factory;
pub mod image_provider_trait;
pub mod openai;
pub mod openai_images;
pub mod provider_factory;
pub mod provider_trait;
pub mod resilient_provider;
pub mod shared;

pub use anthropic::AnthropicProvider;
pub use canonical_bridge::CodeExecutionResponse;
pub use gemini::GeminiProvider;
pub use gemini_images::GeminiImageProvider;
pub use image_provider_factory::{ImageProviderFactory, ImageProviderParams};
pub use image_provider_trait::{BoxedImageProvider, ImageProvider, ImageProviderCapabilities};
pub use openai::OpenAiProvider;
pub use openai_images::OpenAiImageProvider;
pub use provider_factory::{ProviderClientParams, ProviderFactory};
pub use provider_trait::{
    AiProvider, GenerationParams, ModelPricing, SchemaGenerationParams, SearchGenerationParams,
    StructuredGenerationParams, ToolGenerationParams, ToolResultsParams, catalog_default_model,
    catalog_pricing, catalog_supports_model,
};
pub use resilient_provider::ResilientProvider;
