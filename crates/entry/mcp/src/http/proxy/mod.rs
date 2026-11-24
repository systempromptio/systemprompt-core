//! The MCP backends behind the API's proxy engine.
//!
//! [`McpProxyBackends`] is the
//! [`systemprompt_extension::router::ProxyBackendResolver`]
//! the `mcp-http` member provides, sealed (it mints provider bearers with
//! the connector-token cache and the secret lookup), for the kernel's proxy
//! engine alone. It answers which servers are external and the port this
//! instance spawns a managed server on (from the MCP registry), binds managed
//! sessions to their server and principal, and opens the external upstream
//! for a caller: the provider endpoint and headers, the external session
//! binding, the fixed tool arguments and the audit every proxied `tools/call`
//! is recorded with — the execution row and the artifact ingest always, the
//! model's intent pairing when the `ai` member provides the claims.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod audit;
mod external;
mod external_sessions;
pub mod fixed_arguments;
mod mcp_session;

use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderMap;
use systemprompt_extension::router::{ExternalUpstream, ProxyBackendError, ProxyBackendResolver};
use systemprompt_identifiers::{ServiceName, SessionId, UserId};
use systemprompt_mcp::repository::McpProxySessionRepository;
use systemprompt_mcp::services::client::McpClient;
use systemprompt_mcp::services::spawn_target::SpawnTarget;
use systemprompt_mcp::{McpServerConfig, RegistryService};
use systemprompt_models::RequestContext;

pub use audit::AuditSinks;
pub use external::map_resolve_error;

/// The `mcp-http` member's MCP backends for the proxy engine.
#[derive(Debug, Clone)]
pub struct McpProxyBackends {
    registry: RegistryService,
    sessions: Arc<McpProxySessionRepository>,
    client: Arc<McpClient>,
    audit: AuditSinks,
}

impl McpProxyBackends {
    #[must_use]
    pub const fn new(
        registry: RegistryService,
        sessions: Arc<McpProxySessionRepository>,
        client: Arc<McpClient>,
        audit: AuditSinks,
    ) -> Self {
        Self {
            registry,
            sessions,
            client,
            audit,
        }
    }
}

#[async_trait]
impl ProxyBackendResolver for McpProxyBackends {
    fn external_server(
        &self,
        service: &ServiceName,
    ) -> Result<Option<McpServerConfig>, ProxyBackendError> {
        let found = self
            .registry
            .find_server(service.as_str())
            .map_err(|source| ProxyBackendError::RegistryUnavailable(Box::new(source)))?;
        Ok(found.filter(McpServerConfig::is_external))
    }

    fn spawn_port(&self, service: &ServiceName) -> Option<u16> {
        match self.registry.find_server(service.as_str()) {
            Ok(Some(config)) => config.spawn_port().ok(),
            Ok(None) => None,
            Err(e) => {
                tracing::debug!(service = %service, error = %e, "Registry lookup failed while reconciling service port");
                None
            },
        }
    }

    async fn session_bound(
        &self,
        service: &ServiceName,
        session: &SessionId,
        principal: Option<&UserId>,
    ) -> Result<bool, ProxyBackendError> {
        mcp_session::is_bound(&self.sessions, service, session, principal).await
    }

    async fn bind_session(
        &self,
        service: &ServiceName,
        session: &SessionId,
        principal: Option<&UserId>,
    ) -> Result<(), ProxyBackendError> {
        mcp_session::bind(&self.sessions, service, session, principal).await
    }

    async fn open_external(
        &self,
        server: &McpServerConfig,
        caller: &RequestContext,
    ) -> Result<Box<dyn ExternalUpstream>, ProxyBackendError> {
        let target = self
            .client
            .resolve_external_proxy_target(server, caller)
            .await
            .map_err(map_resolve_error)?;
        let headers: HeaderMap = target.headers.into_iter().collect();
        let service = ServiceName::new(server.name.as_str());
        let sessions = external_sessions::SessionGuard::new(
            Arc::clone(&self.sessions),
            service.clone(),
            caller.user_id().clone(),
            &headers,
        );
        Ok(Box::new(external::McpExternalUpstream {
            server: server.clone(),
            service,
            caller: caller.clone(),
            url: target.url,
            headers,
            sessions,
            audit: self.audit.clone(),
        }))
    }
}
