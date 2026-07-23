//! `AiService` construction: provider clients from policy plus registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;

use crate::models::RequestStatus;
use crate::models::ai::AiRequest;
use crate::repository::AiRequestRepository;
use crate::services::config::ConfigValidator;
use crate::services::providers::{AiProvider, ProviderClientParams, ProviderFactory};
use crate::services::tooled::{ResponseSynthesizer, TooledExecutor};
use crate::services::tools::ToolDiscovery;

use super::super::request_storage::{RequestStorage, StoreParams};

use systemprompt_config::SecretsBootstrap;
use systemprompt_database::DbPool;
use systemprompt_models::profile::{ProviderEntry, ProviderRegistry};
use systemprompt_models::services::{AiConfig, AiProviderConfig};
use systemprompt_traits::{DynAiSessionProvider, ToolProvider};

pub struct AiService {
    pub(super) providers: HashMap<String, Arc<dyn AiProvider>>,
    pub(super) tool_provider: Arc<dyn ToolProvider>,
    pub(super) tool_discovery: Arc<ToolDiscovery>,
    pub(super) tooled_executor: TooledExecutor,
    pub(super) synthesizer: ResponseSynthesizer,
    pub(super) storage: RequestStorage,
    default_provider: String,
    default_model: String,
    default_max_output_tokens: u32,
}

impl std::fmt::Debug for AiService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiService")
            .field("default_provider", &self.default_provider)
            .finish_non_exhaustive()
    }
}

impl AiService {
    pub fn new(
        db_pool: &DbPool,
        registry: &ProviderRegistry,
        ai_config: &AiConfig,
        tool_provider: Arc<dyn ToolProvider>,
        session_provider: DynAiSessionProvider,
    ) -> Result<Self> {
        let mut missing_env_vars = Vec::new();
        let providers = Self::build_providers(registry, ai_config, db_pool, &mut missing_env_vars)?;
        ConfigValidator::validate(ai_config, &providers, &missing_env_vars)?;

        let default_provider = ai_config.default_provider.clone();
        let provider = providers.get(&default_provider).ok_or_else(|| {
            crate::error::AiError::Internal(format!(
                "Default provider '{default_provider}' is not enabled or has no registry \
                 connectivity"
            ))
        })?;

        let default_model = ai_config
            .providers
            .get(&default_provider)
            .filter(|pc| !pc.default_model.is_empty())
            .map_or_else(
                || provider.default_model().to_owned(),
                |pc| pc.default_model.clone(),
            );

        let tool_discovery = Arc::new(ToolDiscovery::new(Arc::clone(&tool_provider)));
        let tooled_executor = TooledExecutor::new(Arc::clone(&tool_provider));

        let storage = RequestStorage::new(AiRequestRepository::new(db_pool)?, session_provider);

        Ok(Self {
            providers,
            tool_provider,
            tool_discovery,
            tooled_executor,
            synthesizer: ResponseSynthesizer::new(),
            storage,
            default_provider,
            default_model,
            default_max_output_tokens: ai_config.default_max_output_tokens.unwrap_or(8192),
        })
    }

    // Why: Build one client per enabled AI-policy provider that also has registry
    // connectivity. Providers with policy but no registry entry are skipped
    // with a warning. A missing credential never silently drops a configured
    // provider: the registry endpoint is always present (it may be an internal
    // mock), so the provider stays enabled with an empty key and the absence
    // is recorded for [`ConfigValidator`].
    fn build_providers(
        registry: &ProviderRegistry,
        ai_config: &AiConfig,
        db_pool: &DbPool,
        missing_env_vars: &mut Vec<String>,
    ) -> Result<HashMap<String, Arc<dyn AiProvider>>> {
        let secrets = SecretsBootstrap::get()?;
        let mut providers: HashMap<String, Arc<dyn AiProvider>> = HashMap::new();

        for (name, policy) in &ai_config.providers {
            if !policy.enabled {
                continue;
            }
            let Some(entry) = registry.find_provider(name) else {
                tracing::warn!(
                    provider = %name,
                    "AI policy enables provider but the profile registry has no connectivity \
                     entry — skipping"
                );
                continue;
            };

            let secret_name = entry.api_key_secret.as_str();
            let api_key = secrets.get(secret_name).map_or_else(
                || {
                    tracing::warn!(
                        provider = %name,
                        secret = %secret_name,
                        "api_key secret not found — keeping provider enabled with an empty key \
                         (registry endpoint may be an internal mock)"
                    );
                    missing_env_vars.push(format!(
                        "Provider '{name}': secret '{secret_name}' not found"
                    ));
                    String::new()
                },
                Clone::clone,
            );

            let provider = Self::build_one(entry, policy, api_key, db_pool)?;
            providers.insert(name.clone(), provider);
        }

        Ok(providers)
    }

    fn build_one(
        entry: &ProviderEntry,
        policy: &AiProviderConfig,
        api_key: String,
        db_pool: &DbPool,
    ) -> Result<Arc<dyn AiProvider>> {
        let params = ProviderClientParams {
            name: entry.name.as_str(),
            wire: entry.wire,
            endpoint: &entry.endpoint,
            api_key,
            google_search_enabled: policy.google_search_enabled,
            resilience: &policy.resilience,
            models: &entry.models,
            default_model: (!policy.default_model.is_empty())
                .then_some(policy.default_model.as_str()),
        };
        ProviderFactory::create(&params, Some(Arc::clone(db_pool)))
    }

    pub fn default_provider(&self) -> &str {
        &self.default_provider
    }

    pub fn default_model(&self) -> &str {
        &self.default_model
    }

    pub const fn default_max_output_tokens(&self) -> u32 {
        self.default_max_output_tokens
    }

    pub(super) fn get_provider(&self, name: &str) -> Result<Arc<dyn AiProvider>> {
        self.providers
            .get(name)
            .cloned()
            .ok_or_else(|| crate::error::AiError::Internal(format!("Provider {name} not found")))
    }

    pub(super) async fn audit(&self, params: &StoreParams<'_>) {
        if let Err(e) = self.storage.store(params).await {
            tracing::error!(
                error = %e,
                provider = %params.request.provider(),
                model = %params.request.model(),
                status = ?params.status,
                "audit write failed"
            );
        }
    }

    pub(super) async fn store_error(
        &self,
        request: &AiRequest,
        request_id: uuid::Uuid,
        latency_ms: u64,
        error_message: String,
    ) {
        use crate::models::ai::AiResponse;

        let error_response = AiResponse::new(
            request_id,
            String::new(),
            request.provider().to_owned(),
            request.model().to_owned(),
        )
        .with_latency(latency_ms);
        self.audit(&StoreParams {
            request,
            response: &error_response,
            context: &request.context,
            status: RequestStatus::Failed,
            error_message: Some(&error_message),
            cost_microdollars: 0,
        })
        .await;
    }
}
