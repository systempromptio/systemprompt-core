use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_models::services::AiProviderConfig;

use super::{AiProvider, AnthropicProvider, GeminiProvider, OpenAiProvider};

#[derive(Debug, Copy, Clone)]
pub struct ProviderFactory;

impl ProviderFactory {
    pub fn create(
        name: &str,
        config: &AiProviderConfig,
        db_pool: Option<DbPool>,
    ) -> Result<Arc<dyn AiProvider>> {
        if !config.enabled {
            return Err(anyhow!("Provider {name} is disabled"));
        }

        let provider: Arc<dyn AiProvider> = match name {
            "openai" => config.endpoint.as_ref().map_or_else(
                || Arc::new(OpenAiProvider::new(config.api_key.clone())),
                |endpoint| {
                    Arc::new(OpenAiProvider::with_endpoint(
                        config.api_key.clone(),
                        endpoint.clone(),
                    ))
                },
            ),
            "anthropic" => config.endpoint.as_ref().map_or_else(
                || Arc::new(AnthropicProvider::new(config.api_key.clone())),
                |endpoint| {
                    Arc::new(AnthropicProvider::with_endpoint(
                        config.api_key.clone(),
                        endpoint.clone(),
                    ))
                },
            ),
            "gemini" => {
                let mut provider = if let Some(endpoint) = &config.endpoint {
                    GeminiProvider::with_endpoint(config.api_key.clone(), endpoint.clone())?
                } else {
                    GeminiProvider::new(config.api_key.clone())?
                };

                if config.google_search_enabled {
                    provider = provider.with_google_search();
                }

                if let Some(pool) = db_pool {
                    provider = provider.with_db_pool(pool);
                }

                Arc::new(provider)
            },
            _ => return Err(anyhow!("Unknown provider: {name}")),
        };

        Ok(provider)
    }

    pub fn create_all(
        configs: HashMap<String, AiProviderConfig>,
        db_pool: Option<&DbPool>,
    ) -> Result<HashMap<String, Arc<dyn AiProvider>>> {
        let mut providers = HashMap::new();

        for (name, config) in configs {
            if config.enabled {
                if let Ok(provider) = Self::create(&name, &config, db_pool.cloned()) {
                    providers.insert(name, provider);
                }
            }
        }

        if providers.is_empty() {
            return Err(anyhow!("No providers could be initialized"));
        }

        Ok(providers)
    }
}
