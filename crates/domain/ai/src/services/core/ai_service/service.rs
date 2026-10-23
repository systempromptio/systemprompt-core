//! `AiService` construction: provider clients from the resolved catalog.
//!
//! The in-process default is `gateway.default_model` served by its first
//! deployment that has a client, so there is no separate default provider to
//! configure; a provider with `enabled: false` has no client and is never the
//! default.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::Result;
use std::sync::Arc;
use systemprompt_identifiers::{ModelId, ProviderId};

use crate::models::RequestStatus;
use crate::models::ai::AiRequest;
use crate::repository::AiRepositories;
use crate::services::config::{AiConfigError, ConfigValidator};
use crate::services::providers::ProviderClient;
use crate::services::tooled::{ResponseSynthesizer, TooledExecutor};
use crate::services::tools::ToolDiscovery;

use super::clients::{ProviderClients, WithheldEndpoint};

use super::super::request_storage::{RequestStorage, StoreParams};

use systemprompt_config::SecretsBootstrap;
use systemprompt_manifest::services::AiConfig;
use systemprompt_manifest::services::catalog::ResolvedCatalog;
use systemprompt_traits::{DynAiSessionProvider, ToolProvider};
use tokio_util::task::TaskTracker;

pub struct AiService {
    pub(super) clients: ProviderClients,
    pub(super) catalog: Arc<ResolvedCatalog>,
    pub(super) tool_provider: Arc<dyn ToolProvider>,
    pub(super) tool_discovery: Arc<ToolDiscovery>,
    pub(super) tooled_executor: TooledExecutor,
    pub(super) synthesizer: ResponseSynthesizer,
    pub(super) storage: RequestStorage,
    pub(super) audit_tasks: TaskTracker,
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

#[derive(Clone)]
pub struct AiServiceProviders {
    pub tools: Arc<dyn ToolProvider>,
    pub sessions: DynAiSessionProvider,
}

impl std::fmt::Debug for AiServiceProviders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiServiceProviders").finish_non_exhaustive()
    }
}

impl AiService {
    pub fn new(
        catalog: Arc<ResolvedCatalog>,
        ai_config: &AiConfig,
        providers: AiServiceProviders,
        repositories: &AiRepositories,
    ) -> Result<Self> {
        let AiServiceProviders {
            tools: tool_provider,
            sessions: session_provider,
        } = providers;
        SecretsBootstrap::get()?;
        let (clients, withheld) = ProviderClients::build(&catalog)?;
        ConfigValidator::validate(ai_config, &catalog, &clients, &withheld)?;
        let (default_provider, default_model) =
            Self::derive_default(&catalog, &clients, &withheld)?;

        let tool_discovery = Arc::new(ToolDiscovery::new(Arc::clone(&tool_provider)));
        let tooled_executor = TooledExecutor::new(Arc::clone(&tool_provider));

        let storage = RequestStorage::new(
            repositories.requests.clone(),
            repositories.payloads.clone(),
            session_provider,
        );

        Ok(Self {
            clients,
            catalog,
            tool_provider,
            tool_discovery,
            tooled_executor,
            synthesizer: ResponseSynthesizer::new(),
            storage,
            audit_tasks: TaskTracker::new(),
            default_provider,
            default_model,
            default_max_output_tokens: ai_config.default_max_output_tokens.unwrap_or(8192),
        })
    }

    fn derive_default(
        catalog: &ResolvedCatalog,
        clients: &ProviderClients,
        withheld: &[WithheldEndpoint],
    ) -> Result<(String, String)> {
        let model = catalog
            .default_model()
            .ok_or(AiConfigError::NoDefaultModel)?;
        let hop = clients
            .hop_for(catalog, None, model.as_str())
            .ok_or_else(|| AiConfigError::DefaultModelNoConnectivity {
                model: model.as_str().to_owned(),
                deployments: catalog
                    .deployments(model.as_str())
                    .iter()
                    .map(|hop| hop.endpoint_ref().to_string())
                    .collect(),
                connected: clients
                    .endpoints()
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                withheld: withheld
                    .iter()
                    .map(|w| format!("{}: {}", w.endpoint, w.reason))
                    .collect(),
            })?;
        Ok((hop.provider.as_str().to_owned(), model.as_str().to_owned()))
    }

    #[must_use]
    pub fn with_context_materializer(
        mut self,
        materializer: systemprompt_traits::DynContextMaterializer,
    ) -> Self {
        self.storage = self.storage.with_context_materializer(materializer);
        self
    }

    // Why: streaming audit rows are written by tasks that outlive the stream
    // that produced them; the owner drains this tracker before the runtime
    // is torn down so an abandoned stream's usage is never lost at shutdown.
    #[must_use]
    pub const fn audit_tasks(&self) -> &TaskTracker {
        &self.audit_tasks
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

    #[must_use]
    pub fn catalog(&self) -> &ResolvedCatalog {
        &self.catalog
    }

    pub(super) fn get_provider(&self, request: &AiRequest) -> Result<Arc<dyn ProviderClient>> {
        let provider = Some(request.provider()).filter(|p| !p.is_empty());
        self.clients
            .for_request(&self.catalog, provider, request.model())
            .ok_or_else(|| {
                let routing = match provider {
                    Some(provider) => crate::error::RoutingError::UnknownModel {
                        provider: ProviderId::new(provider),
                        model: ModelId::new(request.model()),
                    },
                    None => crate::error::RoutingError::NoProviderForModel {
                        model: ModelId::new(request.model()),
                    },
                };
                crate::error::AiError::Routing(routing)
            })
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
        request_id: systemprompt_identifiers::AiRequestId,
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
