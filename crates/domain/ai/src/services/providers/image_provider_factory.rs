use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_models::services::AiProviderConfig;

use super::{BoxedImageProvider, GeminiImageProvider, OpenAiImageProvider};

#[derive(Debug, Copy, Clone)]
pub struct ImageProviderFactory;

impl ImageProviderFactory {
    pub fn create(name: &str, config: &AiProviderConfig) -> Result<BoxedImageProvider> {
        if !config.enabled {
            return Err(anyhow!("Image provider {name} is disabled"));
        }

        match name {
            "gemini" => Ok(Self::create_gemini(config)),
            "openai" => Ok(Self::create_openai(config)),
            _ => Err(anyhow!("Unknown image provider: {name}")),
        }
    }

    fn create_gemini(config: &AiProviderConfig) -> BoxedImageProvider {
        let base = config.endpoint.as_ref().map_or_else(
            || GeminiImageProvider::new(config.api_key.clone()),
            |ep| GeminiImageProvider::with_endpoint(config.api_key.clone(), ep.clone()),
        );

        let provider = base.with_model_definitions(config.models.clone());

        let provider = if config.default_model.is_empty() {
            provider
        } else {
            provider.with_default_model(config.default_model.clone())
        };

        Arc::new(provider)
    }

    fn create_openai(config: &AiProviderConfig) -> BoxedImageProvider {
        let base = config.endpoint.as_ref().map_or_else(
            || OpenAiImageProvider::new(config.api_key.clone()),
            |ep| OpenAiImageProvider::with_endpoint(config.api_key.clone(), ep.clone()),
        );

        let provider = if config.default_model.is_empty() {
            base
        } else {
            base.with_default_model(config.default_model.clone())
        };

        Arc::new(provider)
    }

    pub fn create_all(
        configs: &HashMap<String, AiProviderConfig>,
    ) -> Result<HashMap<String, BoxedImageProvider>> {
        let mut providers = HashMap::new();

        for (name, config) in configs.iter().filter(|(_, c)| c.enabled) {
            match Self::create(name, config) {
                Ok(provider) => {
                    providers.insert(name.clone(), provider);
                },
                Err(e) => {
                    tracing::warn!(provider = %name, error = %e, "Failed to create image provider");
                },
            }
        }

        if providers.is_empty() {
            return Err(anyhow!("No image providers could be initialized"));
        }

        Ok(providers)
    }
}
