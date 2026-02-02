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

    pub fn create_with_fallback(
        name: &str,
        config: &AiProviderConfig,
        all_configs: &HashMap<String, AiProviderConfig>,
    ) -> Result<BoxedImageProvider> {
        match Self::create(name, config) {
            Ok(provider) => Ok(provider),
            Err(_) if !Self::supports_image_generation(name) => {
                for fallback_name in &["openai", "gemini"] {
                    if let Some(fallback_config) = all_configs.get(*fallback_name) {
                        if fallback_config.enabled {
                            if let Ok(provider) = Self::create(fallback_name, fallback_config) {
                                tracing::info!(
                                    primary = %name,
                                    fallback = %fallback_name,
                                    "Using fallback image provider"
                                );
                                return Ok(provider);
                            }
                        }
                    }
                }
                Err(anyhow!(
                    "No image provider available (primary: {} does not support images)",
                    name
                ))
            },
            Err(e) => Err(e),
        }
    }

    pub fn supports_image_generation(provider_name: &str) -> bool {
        matches!(provider_name, "openai" | "gemini")
    }

    fn create_gemini(config: &AiProviderConfig) -> BoxedImageProvider {
        let base = config.endpoint.as_ref().map_or_else(
            || GeminiImageProvider::new(config.api_key.clone()),
            |ep| GeminiImageProvider::with_endpoint(config.api_key.clone(), ep.clone()),
        );

        let provider = base.with_model_definitions(config.models.clone());

        let provider = match config.default_image_model.as_str() {
            "" => provider,
            model => provider.with_default_model(model.to_string()),
        };

        Arc::new(provider)
    }

    fn create_openai(config: &AiProviderConfig) -> BoxedImageProvider {
        let base = config.endpoint.as_ref().map_or_else(
            || OpenAiImageProvider::new(config.api_key.clone()),
            |ep| OpenAiImageProvider::with_endpoint(config.api_key.clone(), ep.clone()),
        );

        let provider = match config.default_image_model.as_str() {
            "" => base,
            model => base.with_default_model(model.to_string()),
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
