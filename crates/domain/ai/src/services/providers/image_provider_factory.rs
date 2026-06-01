//! Image-provider factory, keyed on registry [`WireProtocol`].
//!
//! Connectivity (endpoint, resolved key, model catalog) comes from a profile
//! `providers` registry [`ProviderEntry`]; the per-provider AI policy supplies
//! the image-model default. Only the `gemini` and `openai-chat`/`-responses`
//! protocols generate images; other protocols fall back to one that can.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_models::profile::{ProviderEntry, WireProtocol};
use systemprompt_models::services::{AiProviderConfig, ModelDefinition};

use crate::error::Result;

use super::{BoxedImageProvider, GeminiImageProvider, OpenAiImageProvider};

#[derive(Debug)]
pub struct ImageProviderParams<'a> {
    pub entry: &'a ProviderEntry,
    pub policy: &'a AiProviderConfig,
    pub api_key: String,
}

#[derive(Debug, Copy, Clone)]
pub struct ImageProviderFactory;

impl ImageProviderFactory {
    pub fn create(params: &ImageProviderParams<'_>) -> Result<BoxedImageProvider> {
        if !params.policy.enabled {
            return Err(crate::error::AiError::Internal(format!(
                "Image provider {} is disabled",
                params.entry.name.as_str()
            )));
        }

        match params.entry.protocol {
            WireProtocol::Gemini => Ok(Self::create_gemini(params)),
            WireProtocol::OpenAiChat | WireProtocol::OpenAiResponses => {
                Ok(Self::create_openai(params))
            },
            WireProtocol::Anthropic => Err(crate::error::AiError::Internal(format!(
                "Provider {} does not support image generation",
                params.entry.name.as_str()
            ))),
        }
    }

    #[must_use]
    pub const fn supports_image_generation(protocol: WireProtocol) -> bool {
        matches!(
            protocol,
            WireProtocol::Gemini | WireProtocol::OpenAiChat | WireProtocol::OpenAiResponses
        )
    }

    fn create_gemini(params: &ImageProviderParams<'_>) -> BoxedImageProvider {
        let base = GeminiImageProvider::with_endpoint(
            params.api_key.clone(),
            params.entry.endpoint.clone(),
        )
        .with_model_definitions(Self::model_definitions(params.entry));

        let provider = match params.policy.default_image_model.as_str() {
            "" => base,
            model => base.with_default_model(model.to_owned()),
        };

        Arc::new(provider)
    }

    fn create_openai(params: &ImageProviderParams<'_>) -> BoxedImageProvider {
        let base = OpenAiImageProvider::with_endpoint(
            params.api_key.clone(),
            params.entry.endpoint.clone(),
        );

        let provider = match params.policy.default_image_model.as_str() {
            "" => base,
            model => base.with_default_model(model.to_owned()),
        };

        Arc::new(provider)
    }

    /// Project the registry model catalog into the per-model capability/limit
    /// definitions the image providers consume for prompt/resolution checks.
    fn model_definitions(entry: &ProviderEntry) -> HashMap<String, ModelDefinition> {
        entry
            .models
            .iter()
            .map(|m| {
                (
                    m.id.as_str().to_owned(),
                    ModelDefinition {
                        capabilities: m.capabilities,
                        limits: m.limits,
                        pricing: m.pricing,
                    },
                )
            })
            .collect()
    }
}
