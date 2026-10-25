//! The process-wide [`AiService`] handed to every consumer of [`AppContext`].
//!
//! One service per process: its audit tasks are never closed, they drain with
//! the runtime. It is built from the booted `ResolvedCatalog`; a catalog
//! without a servable default model (a bridge-only deployment, say) yields
//! `None` after a single warning, so boot never depends on inference being
//! configured.
//!
//! [`AppContext`]: crate::context::AppContext
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_security::credential::TokenCache;

use systemprompt_ai::{AiService, AiServiceProviders};
use systemprompt_database::DbPool;
use systemprompt_mcp::McpToolProvider;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_users::{SessionRepository, UsersAiSessionProvider};

use super::composition::RepositoryBundles;

pub(super) fn build_ai_service(
    database: &DbPool,
    repositories: &RepositoryBundles,
    mcp_registry: &RegistryService,
    services_state: &systemprompt_loader::ServicesState,
) -> (Option<Arc<AiService>>, Arc<TokenCache>) {
    let services = services_state.config();
    let connector_tokens: Arc<TokenCache> = Arc::default();
    let tool_provider = Arc::new(McpToolProvider::new(
        Arc::clone(database),
        mcp_registry.clone(),
        &services.ai.mcp.resilience,
        Arc::clone(&connector_tokens),
    ));
    let session_provider = Arc::new(UsersAiSessionProvider::from_repository(
        SessionRepository::new(database),
    ));
    let materializer = Arc::new(systemprompt_agent::services::ContextProviderService::new(
        repositories.a2a.contexts.clone(),
    ));
    let catalog = Arc::clone(services_state.catalog());
    match AiService::new(
        catalog,
        &services.ai,
        AiServiceProviders {
            tools: tool_provider,
            sessions: session_provider,
        },
        &repositories.ai,
    ) {
        Ok(service) => (
            Some(Arc::new(service.with_context_materializer(materializer))),
            connector_tokens,
        ),
        Err(err) => {
            tracing::warn!(error = %err, "Inference is not configured; the runtime carries no AI service");
            (None, connector_tokens)
        },
    }
}
