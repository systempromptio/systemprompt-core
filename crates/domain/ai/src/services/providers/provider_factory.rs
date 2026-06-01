//! Protocol-driven provider-client factory.
//!
//! A provider client is selected by the registry entry's [`WireProtocol`], not
//! by its name: any vendor speaking `anthropic` reuses [`AnthropicProvider`],
//! any speaking `openai-chat`/`openai-responses` reuses [`OpenAiProvider`], and
//! `gemini` uses [`GeminiProvider`]. Connectivity (endpoint, resolved key)
//! comes from the profile `providers` registry; the per-provider AI policy
//! supplies resilience and the web-search toggle.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_models::profile::WireProtocol;
use systemprompt_models::services::ResilienceSettings;

use crate::error::Result;

use super::{AiProvider, AnthropicProvider, GeminiProvider, OpenAiProvider, ResilientProvider};

/// Resolved connectivity + policy for one provider client.
#[derive(Debug)]
pub struct ProviderClientParams<'a> {
    pub name: &'a str,
    pub protocol: WireProtocol,
    pub endpoint: &'a str,
    pub api_key: String,
    pub google_search_enabled: bool,
    pub resilience: &'a ResilienceSettings,
}

#[derive(Debug, Copy, Clone)]
pub struct ProviderFactory;

impl ProviderFactory {
    pub fn create(
        params: &ProviderClientParams<'_>,
        db_pool: Option<DbPool>,
    ) -> Result<Arc<dyn AiProvider>> {
        let inner: Arc<dyn AiProvider> = match params.protocol {
            WireProtocol::Anthropic => {
                let provider = AnthropicProvider::with_endpoint(
                    params.api_key.clone(),
                    params.endpoint.to_owned(),
                );
                let provider = if params.google_search_enabled {
                    provider.with_web_search()
                } else {
                    provider
                };
                Arc::new(provider)
            },
            WireProtocol::OpenAiChat | WireProtocol::OpenAiResponses => {
                let provider = OpenAiProvider::with_endpoint(
                    params.api_key.clone(),
                    params.endpoint.to_owned(),
                );
                let provider = if params.google_search_enabled {
                    provider.with_web_search()
                } else {
                    provider
                };
                Arc::new(provider)
            },
            WireProtocol::Gemini => {
                let mut provider = GeminiProvider::with_endpoint(
                    params.api_key.clone(),
                    params.endpoint.to_owned(),
                )?;
                if params.google_search_enabled {
                    provider = provider.with_google_search();
                }
                if let Some(pool) = db_pool {
                    provider = provider.with_db_pool(pool);
                }
                Arc::new(provider)
            },
        };

        Ok(Arc::new(ResilientProvider::new(
            params.name,
            inner,
            params.resilience,
        )))
    }
}
