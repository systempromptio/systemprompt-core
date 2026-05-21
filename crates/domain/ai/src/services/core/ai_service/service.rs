use crate::error::Result;
use std::collections::HashMap;
use std::sync::Arc;

use crate::models::RequestStatus;
use crate::models::ai::AiRequest;
use crate::repository::AiRequestRepository;
use crate::services::config::ConfigValidator;
use crate::services::providers::{AiProvider, ProviderFactory};
use crate::services::tooled::{ResponseSynthesizer, TooledExecutor};
use crate::services::tools::ToolDiscovery;

use super::super::request_storage::{RequestStorage, StoreParams};

use systemprompt_config::SecretsBootstrap;
use systemprompt_database::DbPool;
use systemprompt_models::services::AiConfig;
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
        ai_config: &AiConfig,
        tool_provider: Arc<dyn ToolProvider>,
        session_provider: Option<DynAiSessionProvider>,
    ) -> Result<Self> {
        let mut config = ai_config.clone();
        let missing_env_vars = Self::expand_secrets(&mut config)?;
        ConfigValidator::validate(&config, &missing_env_vars)?;

        let providers = ProviderFactory::create_all(config.providers.clone(), Some(db_pool))?;
        let default_provider = config.default_provider.clone();

        let provider = providers.get(&default_provider).ok_or_else(|| {
            crate::error::AiError::Internal(format!(
                "Default provider '{}' not found or not enabled",
                default_provider
            ))
        })?;

        let provider_config = config.providers.get(&default_provider);
        let default_model = provider_config
            .and_then(|pc| {
                if pc.default_model.is_empty() {
                    None
                } else {
                    Some(pc.default_model.clone())
                }
            })
            .unwrap_or_else(|| provider.default_model().to_string());

        let tool_discovery = Arc::new(ToolDiscovery::new(Arc::clone(&tool_provider)));
        let tooled_executor = TooledExecutor::new(Arc::clone(&tool_provider));

        let mut storage = RequestStorage::new(AiRequestRepository::new(db_pool)?);
        if let Some(provider) = session_provider {
            storage = storage.with_session_provider(provider);
        }

        Ok(Self {
            providers,
            tool_provider,
            tool_discovery,
            tooled_executor,
            synthesizer: ResponseSynthesizer::new(),
            storage,
            default_provider,
            default_model,
            default_max_output_tokens: config.default_max_output_tokens.unwrap_or(8192),
        })
    }

    fn expand_secrets(config: &mut AiConfig) -> Result<Vec<String>> {
        let mut missing_vars = Vec::new();
        let secrets = SecretsBootstrap::get()?;

        for (name, provider_config) in &mut config.providers {
            // Resolve the endpoint first — a `${VAR}` endpoint is interpolated
            // from the same secrets store as the api_key. An unresolved
            // endpoint var clears `endpoint` to `None` so the provider falls
            // back to its built-in external URL (a no-op for non-air-gapped
            // deployments that simply omit the var).
            if let Some(endpoint) = provider_config.endpoint.as_ref() {
                if endpoint.starts_with("${") && endpoint.ends_with('}') {
                    let var_name = endpoint[2..endpoint.len() - 1].to_string();
                    if let Some(v) = secrets.get(&var_name) {
                        provider_config.endpoint = Some(v.clone());
                    } else {
                        provider_config.endpoint = None;
                        tracing::warn!(
                            provider = %name,
                            var = %var_name,
                            "endpoint secret not found — falling back to provider default URL"
                        );
                    }
                }
            }
            let has_custom_endpoint = provider_config.endpoint.is_some();

            if provider_config.api_key.starts_with("${") && provider_config.api_key.ends_with('}') {
                let var_name =
                    provider_config.api_key[2..provider_config.api_key.len() - 1].to_string();

                if let Some(v) = secrets.get(&var_name) {
                    provider_config.api_key.clone_from(v);
                } else if has_custom_endpoint {
                    // Air-gap: the provider points at an internal endpoint
                    // (e.g. a mock) that needs no upstream credential. Keep it
                    // enabled with an empty key rather than disabling it.
                    provider_config.api_key = String::new();
                    tracing::warn!(
                        provider = %name,
                        var = %var_name,
                        "api_key secret not found, but a custom endpoint is configured — \
                         keeping provider enabled with an empty key"
                    );
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
            request.provider().to_string(),
            request.model().to_string(),
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
