//! `AiService`: the type, its accessors and its
//! [`systemprompt_models::ai::AiProvider`] bridge; construction, routing and
//! audit writes live in the submodules.
//!
//! The in-process default is `gateway.default_model` served by its first
//! deployment that has a client, so there is no separate default provider to
//! configure; a provider with `enabled: false` has no client and is never the
//! default.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod audit;
mod construct;
mod provider;
mod routing;

use std::sync::Arc;

use systemprompt_manifest::services::catalog::ResolvedCatalog;
use systemprompt_security::credential::TokenCache;
use systemprompt_traits::{DynAiSessionProvider, ToolProvider};
use tokio_util::task::TaskTracker;

use super::clients::ProviderClients;
use crate::services::core::request_storage::RequestStorage;
use crate::services::tooled::{ResponseSynthesizer, TooledExecutor};
use crate::services::tools::ToolDiscovery;

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
    pub provider_tokens: Arc<TokenCache>,
}

impl std::fmt::Debug for AiServiceProviders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiServiceProviders").finish_non_exhaustive()
    }
}

impl AiService {
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
}
