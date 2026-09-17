//! The process-wide [`AiService`] handed to every consumer of [`AppContext`].
//!
//! One service per process: its audit tasks are never closed, they drain with
//! the runtime. A catalog without a usable default provider (a bridge-only
//! deployment, say) yields `None` after a single warning, so boot never
//! depends on inference being configured.
//!
//! [`AppContext`]: crate::context::AppContext
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_ai::{AiService, AiServiceProviders};
use systemprompt_database::DbPool;
use systemprompt_mcp::McpToolProvider;
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_users::{SessionRepository, UsersAiSessionProvider};

use super::composition::RepositoryBundles;
use crate::error::{RuntimeError, RuntimeResult};

pub(super) fn build_ai_service(
    database: &DbPool,
    repositories: &RepositoryBundles,
    mcp_registry: &RegistryService,
) -> RuntimeResult<Option<Arc<AiService>>> {
    let services = systemprompt_loader::ServicesBootstrap::get()
        .map_err(|err| RuntimeError::Internal(format!("services config: {err}")))?;
    let tool_provider = Arc::new(McpToolProvider::new(
        Arc::clone(database),
        mcp_registry.clone(),
        &services.ai.mcp.resilience,
    ));
    let session_provider = Arc::new(UsersAiSessionProvider::from_repository(
        SessionRepository::new(database)
            .map_err(|err| RuntimeError::Internal(format!("session repository: {err}")))?,
    ));
    let materializer = Arc::new(systemprompt_agent::services::ContextProviderService::new(
        repositories.a2a.contexts.clone(),
    ));
    match AiService::new(
        database,
        &services.providers,
        &services.ai,
        AiServiceProviders {
            tools: tool_provider,
            sessions: session_provider,
        },
        &repositories.ai,
    ) {
        Ok(service) => Ok(Some(Arc::new(
            service.with_context_materializer(materializer),
        ))),
        Err(err) => {
            tracing::warn!(error = %err, "Inference is not configured; the runtime carries no AI service");
            Ok(None)
        },
    }
}
