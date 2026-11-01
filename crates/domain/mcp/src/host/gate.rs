//! The per-request gate of [`ToolsetServer`]: authentication, the caller's
//! groups, per-caller tool visibility, per-tool authorisation and the audit
//! of every outcome.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use rmcp::service::{RequestContext, RoleServer};
use systemprompt_models::RequestContext as SysRequestContext;

use super::recorder::{McpAccessEvent, McpAccessOutcome};
use super::server::ToolsetServer;
use crate::middleware::rbac::{AuthResult, JwtVerifier, enforce_rbac_from_registry};

const GROUP_DIMENSION: &str = "group";

impl ToolsetServer {
    pub(super) async fn record(
        &self,
        tool: &str,
        ctx: Option<&SysRequestContext>,
        outcome: McpAccessOutcome<'_>,
    ) {
        let event = McpAccessEvent {
            server: &self.inner.server_id,
            tool,
            user_id: ctx.map(SysRequestContext::user_id),
            outcome,
        };
        for recorder in &self.inner.recorders {
            recorder.record(event).await;
        }
    }

    pub(super) async fn authenticate(
        &self,
        tool: &str,
        ctx: &RequestContext<RoleServer>,
    ) -> Result<SysRequestContext, McpError> {
        let services = self.inner.authz.services.config();
        let verifier = JwtVerifier {
            authority: self.inner.authz.tokens.as_ref(),
            config: &self.inner.authz.config,
        };
        match enforce_rbac_from_registry(
            ctx,
            &self.inner.server_id,
            &self.inner.authz.hook,
            &services,
            &verifier,
        )
        .await
        {
            Ok(AuthResult::Authenticated(authenticated)) => {
                self.record(
                    tool,
                    Some(&authenticated.context),
                    McpAccessOutcome::Authenticated,
                )
                .await;
                Ok(authenticated.context)
            },
            Ok(AuthResult::Anonymous(context)) => Ok(context),
            Err(error) => {
                self.record(
                    tool,
                    None,
                    McpAccessOutcome::Rejected {
                        reason: error.message.as_ref(),
                    },
                )
                .await;
                Err(error)
            },
        }
    }

    pub(super) async fn caller_groups(&self, context: &SysRequestContext) -> Vec<String> {
        let Some(user) = context.user.as_ref() else {
            return Vec::new();
        };
        let Some(provider) = self.inner.authz.subjects.find(GROUP_DIMENSION) else {
            return Vec::new();
        };
        provider.values_for(&user.id).await.unwrap_or_else(|error| {
            tracing::warn!(
                server = %self.inner.server_id,
                error = %error,
                "group lookup failed; group-scoped tools are hidden for this request"
            );
            Vec::new()
        })
    }

    pub(super) async fn visible_tools(
        &self,
        context: &SysRequestContext,
    ) -> Vec<rmcp::model::Tool> {
        let services = self.inner.authz.services.config();
        let Some(deployment) = services.mcp_servers.get(self.inner.server_id.as_str()) else {
            return Vec::new();
        };
        let catalog = &self.inner.catalog;
        let toolsets = &self.inner.toolsets;
        let groups = if catalog.needs_groups(toolsets, deployment) {
            self.caller_groups(context).await
        } else {
            Vec::new()
        };
        catalog.visible_tools(toolsets, deployment, context.user.as_ref(), &groups)
    }

    pub(super) async fn authorise_tool(
        &self,
        tool: &str,
        context: &SysRequestContext,
    ) -> Result<(), McpError> {
        let services = self.inner.authz.services.config();
        let deployment = services
            .mcp_servers
            .get(self.inner.server_id.as_str())
            .ok_or_else(|| McpError::internal_error("MCP server not found in registry", None))?;
        let Some(access) = self
            .inner
            .catalog
            .access_for(&self.inner.toolsets, deployment, tool)
        else {
            return Err(McpError::invalid_params(
                format!(
                    "Unknown tool: '{tool}'. This server exposes: {}.",
                    self.inner.catalog.names().join(", ")
                ),
                None,
            ));
        };
        let groups = if access.needs_groups() {
            self.caller_groups(context).await
        } else {
            Vec::new()
        };
        if access.permits(context.user.as_ref(), &groups) {
            return Ok(());
        }
        let reason = format!("tool '{tool}' is not granted to this caller");
        tracing::warn!(server = %self.inner.server_id, tool, "tool access denied");
        self.record(
            tool,
            Some(context),
            McpAccessOutcome::Forbidden { reason: &reason },
        )
        .await;
        Err(McpError::invalid_request(
            format!("Forbidden: {reason}"),
            Some(serde_json::json!({ "reason": "forbidden", "tool": tool })),
        ))
    }
}
