//! `AiService` construction: provider clients, default model and storage
//! wiring.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::SecretsBootstrap;
use systemprompt_manifest::services::AiConfig;
use systemprompt_manifest::services::catalog::ResolvedCatalog;

use super::{AiService, AiServiceProviders};
use crate::error::Result;
use crate::repository::AiRepositories;
use crate::services::config::{AiConfigError, ConfigValidator};
use crate::services::core::ai_service::clients::{ProviderClients, WithheldEndpoint};
use crate::services::core::request_storage::RequestStorage;
use crate::services::tooled::{ResponseSynthesizer, TooledExecutor};
use crate::services::tools::ToolDiscovery;
use tokio_util::task::TaskTracker;

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
            provider_tokens,
        } = providers;
        SecretsBootstrap::get()?;
        let (clients, withheld) = ProviderClients::build(&provider_tokens, &catalog)?;
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
}
