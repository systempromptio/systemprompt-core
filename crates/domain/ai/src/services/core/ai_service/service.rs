use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

use crate::models::ai::AiRequest;
use crate::models::RequestStatus;
use crate::repository::AiRequestRepository;
use crate::services::config::ConfigValidator;
use crate::services::providers::{AiProvider, ProviderFactory};
use crate::services::tooled::{ResponseSynthesizer, TooledExecutor};
use crate::services::tools::ToolDiscovery;

use super::super::request_storage::{RequestStorage, StoreParams};

use systemprompt_analytics::SessionRepository;
use systemprompt_models::services::AiConfig;
use systemprompt_models::SecretsBootstrap;
use systemprompt_runtime::AppContext;
use systemprompt_traits::ToolProvider;

pub struct AiService {
    pub(super) providers: HashMap<String, Arc<dyn AiProvider>>,
    pub(super) tool_provider: Arc<dyn ToolProvider>,
    pub(super) tool_discovery: Arc<ToolDiscovery>,
    pub(super) tooled_executor: TooledExecutor,
    pub(super) synthesizer: ResponseSynthesizer,
    pub(super) storage: RequestStorage,
    _db_pool: systemprompt_database::DbPool,
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
        app_context: &Arc<AppContext>,
        ai_config: &AiConfig,
        tool_provider: Arc<dyn ToolProvider>,
    ) -> Result<Self> {
        let db_pool = Arc::clone(app_context.db_pool());

        let mut config = ai_config.clone();
        let missing_env_vars = Self::expand_secrets(&mut config)?;
        ConfigValidator::validate(&config, &missing_env_vars)?;

        let providers = ProviderFactory::create_all(config.providers.clone(), Some(&db_pool))?;
        let default_provider = config.default_provider.clone();
        let default_model = providers
            .get(&default_provider)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Default provider '{}' not found or not enabled",
                    default_provider
                )
            })?
            .default_model()
            .to_string();

        let tool_discovery = Arc::new(ToolDiscovery::new(Arc::clone(&tool_provider)));
        let tooled_executor = TooledExecutor::new(Arc::clone(&tool_provider));

        let storage = RequestStorage::new(
            AiRequestRepository::new(&db_pool)?,
            SessionRepository::new(Arc::clone(&db_pool)),
        );

        Ok(Self {
            providers,
            tool_provider,
            tool_discovery,
            tooled_executor,
            synthesizer: ResponseSynthesizer::new(),
            storage,
            _db_pool: db_pool,
            default_provider,
            default_model,
            default_max_output_tokens: config.default_max_output_tokens.unwrap_or(8192),
        })
    }

    fn expand_secrets(config: &mut AiConfig) -> Result<Vec<String>> {
        let mut missing_vars = Vec::new();
        let secrets = SecretsBootstrap::get()?;

        for (name, provider_config) in &mut config.providers {
            if provider_config.api_key.starts_with("${") && provider_config.api_key.ends_with('}') {
                let var_name =
                    provider_config.api_key[2..provider_config.api_key.len() - 1].to_string();

                if let Some(v) = secrets.get(&var_name) {
                    provider_config.api_key.clone_from(v);
                } else {
                    provider_config.enabled = false;
                    provider_config.api_key = String::new();
                    missing_vars.push(format!(
                        "Provider '{}' disabled: secret {} not found",
                        name, var_name
                    ));
                }
            }
        }

        Ok(missing_vars)
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
            .ok_or_else(|| anyhow::anyhow!("Provider {name} not found"))
    }

    pub(super) fn store_error(
        &self,
        request: &AiRequest,
        request_id: uuid::Uuid,
        latency_ms: u64,
        error: &anyhow::Error,
    ) {
        use crate::models::ai::AiResponse;

        let error_response = AiResponse::new(
            request_id,
            String::new(),
            request.provider().to_string(),
            request.model().to_string(),
        )
        .with_latency(latency_ms);
        self.storage.store(&StoreParams {
            request,
            response: &error_response,
            context: &request.context,
            status: RequestStatus::Failed,
            error_message: Some(&error.to_string()),
            cost_cents: 0,
        });
    }
}
