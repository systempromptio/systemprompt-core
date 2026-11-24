//! [`McpHttpExtension`] (`mcp-http`): the MCP registry route set and the
//! `plugins mcp` commands.
//!
//! With `http` it contributes one route set, `GET /api/v1/mcp/registry`,
//! behind the public gate with a public request context, in the
//! MCP-and-stream role group under the `mcp_registry` rate-limit scope and
//! budget. Its shape equals the reference composition's conferral for
//! `mcp-http` at its base, which pins the set to that one route. The handler
//! reads the MCP server registry the `mcp` member provides.
//!
//! With `cli` it contributes the `plugins mcp` group under the kernel's
//! `plugins` group, requesting the `services-tree` grant (for the API's
//! external URL) and the `caller-session` grant its reserved path lists. The
//! commands read the registry and the read-only `Arc<McpServiceObserver>` the
//! `mcp` member provides; none of them can start, stop or signal a server,
//! and `tools` and `call` reach a server only through the API's MCP proxy
//! with the caller's own session. The member `requires` each value it reads.
//!
//! With `http` it also provides the open
//! [`DynMcpSurface`](systemprompt_extension::router::DynMcpSurface) the API
//! builds the tool-execution lookup `/api/v1/mcp/executions/{id}` from, and,
//! sealed, the [`McpHttpRepositories`](crate::http::McpHttpRepositories) it
//! reads (the tool-execution ledger, built here). The API keeps the lookup's
//! declaration: authenticated, a user context, the `mcp` rate scope and
//! budget. With `http` it provides, sealed and after governance, the
//! [`DynProxyBackendResolver`](systemprompt_extension::router::DynProxyBackendResolver)
//! the kernel's MCP proxy engine forwards through
//! ([`McpProxyBackends`](crate::http::proxy::McpProxyBackends): it mints
//! per-user provider bearers with the connector-token cache and the secret
//! lookup, so only the kernel reads it). With `mcp-http` disabled the lookup
//! and the MCP proxy are not mounted (404).
//!
//! The MCP proxy engine and routes under `/api/v1/mcp/{server}/…`, the MCP
//! OAuth discovery documents, the MCP child host and `admin mcp` are the
//! kernel's.
//!
//! | profile disables | registry route | `plugins mcp` |
//! |---|---|---|
//! | `mcp-http` | 404 (executions lookup and proxy too) | refused at dispatch |
//! | `mcp` | boot refused (unmet requirements) | boot refused |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;
use systemprompt_extension::reserved::CoreId;

pub const MCP_HTTP_EXTENSION: &str = CoreId::McpHttp.id();

/// The member that contributes the MCP registry route set and commands.
#[derive(Debug, Clone, Copy, Default)]
pub struct McpHttpExtension;

impl Extension for McpHttpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: MCP_HTTP_EXTENSION,
            name: "MCP HTTP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    #[cfg_attr(
        not(any(feature = "http", feature = "cli")),
        expect(
            unused_variables,
            reason = "the member contributes only with the http or cli feature"
        )
    )]
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(any(feature = "http", feature = "cli"))]
        c.requires::<std::sync::Arc<dyn systemprompt_traits::McpRegistryProvider>>();
        #[cfg(feature = "http")]
        surface::contribute(c);
        #[cfg(feature = "cli")]
        commands::contribute(c);
        Ok(())
    }
}

#[cfg(feature = "http")]
mod surface {
    use std::sync::Arc;

    use systemprompt_database::DbPool;
    use systemprompt_extension::prelude::*;
    use systemprompt_extension::router::{DynMcpSurface, DynProxyBackendResolver};
    use systemprompt_manifest::Config;
    use systemprompt_manifest::services::{ServicesView, SystemAdmin};
    use systemprompt_mcp::repository::{McpProxySessionRepository, ToolUsageRepository};
    use systemprompt_mcp::services::DeploymentService;
    use systemprompt_mcp::services::client::{McpClient, McpClientLimits};
    use systemprompt_mcp::{ArtifactIngest, IntentClaimService, RegistryService};
    use systemprompt_security::credential::TokenCache;
    use systemprompt_traits::{DynToolCallIntentClaims, SecretLookup};

    use crate::http::proxy::{AuditSinks, McpProxyBackends};
    use crate::http::{McpHttpRepositories, McpHttpSurface, registry_routes};

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.routes(registry_routes())
            .provide(|_r: &Resolved<'_>| -> Result<DynMcpSurface, ProvideError> {
                Ok(Arc::new(McpHttpSurface))
            })
            .provide_sealed(repositories)
            .provide_sealed_after_governance(proxy_backends);
    }

    fn proxy_backends(r: &Resolved<'_>) -> Result<DynProxyBackendResolver, ProvideError> {
        let db = r.get::<DbPool>()?;
        let services = r.get::<ServicesView>()?;
        let config = r.get::<Arc<Config>>()?;
        let registry = RegistryService::new(
            r.get::<SystemAdmin>()?.id().clone(),
            services.clone(),
            Arc::clone(&config),
        );
        let executions =
            Arc::new(ToolUsageRepository::new(&db).with_audit_spans(
                r.optional::<systemprompt_models::audit_spans::DynAuditSpanSink>(),
            ));
        let intent_claims = r.optional::<DynToolCallIntentClaims>();
        if intent_claims.is_none() {
            tracing::info!(
                "the ai member is not admitted: proxied MCP executions are recorded and scanned but not paired with a model intent"
            );
        }
        let limits = McpClientLimits::from(&services.config().ai.mcp.resilience);
        let client = McpClient::new(
            r.get::<Arc<TokenCache>>()?,
            DeploymentService::new(services),
            config,
            r.get::<Arc<dyn SecretLookup>>()?,
            limits,
        );
        Ok(Arc::new(McpProxyBackends::new(
            registry,
            Arc::new(McpProxySessionRepository::new(&db)),
            Arc::new(client),
            AuditSinks {
                intent_claims: intent_claims
                    .map(|intents| IntentClaimService::new(intents, Arc::clone(&executions))),
                executions,
                ingest: r.get::<Arc<ArtifactIngest>>()?,
                background: r.tasks().clone(),
            },
        )))
    }

    fn repositories(r: &Resolved<'_>) -> Result<Arc<McpHttpRepositories>, ProvideError> {
        let db = r.get::<DbPool>()?;
        Ok(Arc::new(McpHttpRepositories {
            tool_usage: Arc::new(ToolUsageRepository::new(&db)),
        }))
    }
}

#[cfg(feature = "cli")]
mod commands {
    use std::sync::Arc;

    use systemprompt_extension::cli::{CommandClaim, CommandDecl, KernelGrant, parents};
    use systemprompt_extension::prelude::*;
    use systemprompt_mcp::services::McpServiceObserver;

    use crate::cli::McpCommands;

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.requires::<Arc<McpServiceObserver>>().command(
            CommandDecl::group_under::<parents::Plugins, McpCommands>()
                .requesting(&[KernelGrant::ServicesTree, KernelGrant::CallerSession])
                .claiming(&[CommandClaim::PreservesData]),
        );
    }
}
