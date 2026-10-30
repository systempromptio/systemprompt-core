//! [`ToolsetServer`]: the rmcp `ServerHandler` that serves hosted toolsets.
//!
//! Every list and call authenticates through
//! [`enforce_rbac_from_registry`](crate::middleware::rbac::enforce_rbac_from_registry),
//! which also runs the governance authz hook for the server. `tools/list` then
//! answers only the tools whose resolved access admits the caller; `tools/call`
//! refuses a tool the caller may not see with a forbidden error and a recorded
//! denial before the owning toolset runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, Implementation, ListResourcesResult, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, ReadResourceRequestParams, ReadResourceResponse,
    ServerCapabilities, ServerConfig,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, ServerHandler};
use systemprompt_governance::authz::{SharedAuthzHook, SubjectProviderSet};
use systemprompt_identifiers::McpServerId;
use systemprompt_manifest::services::ServicesView;
use systemprompt_models::auth::TokenAuthority;

use super::catalog::{HostedCatalog, ToolsetHostError};
use super::recorder::{McpAccessOutcome, SharedMcpAccessRecorder};
use crate::toolset::{McpToolset, ToolsetCall};
use crate::{
    McpToolExecutor, WEBSITE_URL, build_tool_list_result, client_profile_from_peer,
    default_server_icons,
};

/// What a [`ToolsetServer`] authorises against.
#[derive(Clone)]
pub struct ToolsetAuthz {
    pub hook: SharedAuthzHook,
    pub services: ServicesView,
    pub tokens: Arc<dyn TokenAuthority>,
    pub config: Arc<systemprompt_manifest::Config>,
    pub subjects: SubjectProviderSet,
}

impl std::fmt::Debug for ToolsetAuthz {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolsetAuthz").finish_non_exhaustive()
    }
}

/// Everything a [`ToolsetServer`] is built from.
#[derive(Debug)]
pub struct ToolsetServerParts {
    pub server_id: McpServerId,
    pub executor: McpToolExecutor,
    pub authz: ToolsetAuthz,
    pub recorders: Vec<SharedMcpAccessRecorder>,
    pub toolsets: Vec<Box<dyn McpToolset>>,
}

#[derive(Debug, Clone)]
pub struct ToolsetServer {
    pub(super) inner: Arc<Inner>,
}

#[derive(Debug)]
pub(super) struct Inner {
    pub(super) server_id: McpServerId,
    pub(super) executor: McpToolExecutor,
    pub(super) authz: ToolsetAuthz,
    pub(super) recorders: Vec<SharedMcpAccessRecorder>,
    pub(super) toolsets: Vec<Box<dyn McpToolset>>,
    pub(super) catalog: HostedCatalog,
    pub(super) instructions: String,
    pub(super) title: String,
}

impl ToolsetServer {
    pub fn new(parts: ToolsetServerParts) -> Result<Self, ToolsetHostError> {
        let services = parts.authz.services.config();
        let deployment = services
            .mcp_servers
            .get(parts.server_id.as_str())
            .ok_or_else(|| ToolsetHostError::UnknownServer {
                server: parts.server_id.clone(),
            })?;
        let catalog = HostedCatalog::build(&parts.server_id, deployment, &parts.toolsets)?;
        let instructions = parts
            .toolsets
            .iter()
            .map(|toolset| toolset.instructions().trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        let title = deployment
            .description
            .clone()
            .unwrap_or_else(|| parts.server_id.to_string());
        Ok(Self {
            inner: Arc::new(Inner {
                server_id: parts.server_id,
                executor: parts.executor,
                authz: parts.authz,
                recorders: parts.recorders,
                toolsets: parts.toolsets,
                catalog,
                instructions,
                title,
            }),
        })
    }

    #[must_use]
    pub fn server_id(&self) -> &McpServerId {
        &self.inner.server_id
    }

    #[must_use]
    pub fn tool_names(&self) -> Vec<&str> {
        self.inner.catalog.names()
    }

    fn serves_resources(&self) -> bool {
        self.inner.toolsets.iter().any(|t| t.serves_resources())
    }
}

impl ServerHandler for ToolsetServer {
    fn get_info(&self) -> ServerConfig {
        let capabilities = if self.serves_resources() {
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build()
        } else {
            ServerCapabilities::builder().enable_tools().build()
        };
        let config = ServerConfig::new(capabilities)
            .with_protocol_version(ProtocolVersion::V_2026_07_28)
            .with_server_info(
                Implementation::new(
                    format!("{} ({})", self.inner.title, self.inner.server_id),
                    env!("CARGO_PKG_VERSION"),
                )
                .with_title(self.inner.title.clone())
                .with_icons(default_server_icons())
                .with_website_url(WEBSITE_URL),
            );
        if self.inner.instructions.is_empty() {
            config
        } else {
            config.with_instructions(self.inner.instructions.clone())
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let context = self.authenticate("tools/list", &ctx).await?;
        Ok(build_tool_list_result(self.visible_tools(&context).await))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let tool = request.name.to_string();
        let context = self.authenticate(&tool, &ctx).await?;
        self.authorise_tool(&tool, &context).await?;
        let Some(hosted) = self.inner.catalog.get(&tool) else {
            return Err(McpError::invalid_params(
                format!("Unknown tool: '{tool}'"),
                None,
            ));
        };
        self.record(&tool, Some(&context), McpAccessOutcome::Used)
            .await;
        let client = client_profile_from_peer(&ctx);
        self.inner.toolsets[hosted.toolset]
            .call(ToolsetCall {
                executor: &self.inner.executor,
                request: &request,
                context: &context,
                client: &client,
            })
            .await
            .map(Into::into)
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        self.authenticate("resources/list", &ctx).await?;
        let mut resources = Vec::new();
        for toolset in self.inner.toolsets.iter().filter(|t| t.serves_resources()) {
            resources.extend(toolset.list_resources().resources);
        }
        Ok(ListResourcesResult::with_all_items(resources))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        self.authenticate("resources/read", &ctx).await?;
        self.inner
            .toolsets
            .iter()
            .filter(|t| t.serves_resources())
            .find_map(|toolset| toolset.read_resource(&request))
            .unwrap_or_else(|| {
                Err(McpError::resource_not_found(
                    format!("Unknown resource: {}", request.uri),
                    None,
                ))
            })
            .map(Into::into)
    }
}
