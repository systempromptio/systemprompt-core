//! The external MCP upstream: an external MCP server has no local backend
//! port; the `mcp-http` member mints a per-user provider bearer server-side
//! and names the provider endpoint, so the proxy engine forwards the MCP
//! frames there while withholding the systemprompt credential and the
//! provider URL from the client. Before a frame is forwarded the upstream
//! fixes the configured tool arguments, has the kernel govern a
//! `tools/call`, and builds the per-call audit the engine taps the response
//! with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderValue};
use systemprompt_extension::router::{
    CallAudit, ExternalUpstream, GovernedToolCall, ProxiedCallGovernor, ProxyBackendError,
};
use systemprompt_identifiers::{McpServerId, ServiceName};
use systemprompt_mcp::{McpDomainError, McpServerConfig};
use systemprompt_models::RequestContext;

use super::audit::{AuditSinks, McpAudit, ToolCallFrame, classify_tool_call, parse_tool_call};
use super::external_sessions::SessionGuard;

pub(super) struct McpExternalUpstream {
    pub(super) server: McpServerConfig,
    pub(super) service: ServiceName,
    pub(super) caller: RequestContext,
    pub(super) url: String,
    pub(super) headers: HeaderMap,
    pub(super) sessions: SessionGuard,
    pub(super) audit: AuditSinks,
}

#[async_trait]
impl ExternalUpstream for McpExternalUpstream {
    fn url(&self) -> &str {
        &self.url
    }

    fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    async fn accepts(&self, session: &HeaderValue) -> Result<bool, ProxyBackendError> {
        self.sessions.accepts(session).await
    }

    async fn remember(&self, response_headers: &HeaderMap) -> Result<(), ProxyBackendError> {
        self.sessions.remember(response_headers).await
    }

    async fn forget(&self, request_headers: &HeaderMap) -> Result<(), ProxyBackendError> {
        self.sessions.forget(request_headers).await
    }

    async fn prepare(
        &self,
        body: &mut Vec<u8>,
        governor: &dyn ProxiedCallGovernor,
    ) -> Result<Option<Box<dyn CallAudit>>, ProxyBackendError> {
        super::fixed_arguments::apply(&self.server.tools, body);
        match governed_call(&self.service, body)? {
            GovernedFrame::None => {},
            GovernedFrame::Valid(value) => {
                let tool = value
                    .pointer("/params/name")
                    .and_then(serde_json::Value::as_str)
                    .ok_or(ProxyBackendError::Forbidden)?;
                let arguments = value
                    .pointer("/params/arguments")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                governor
                    .enforce(
                        &self.caller,
                        &self.service,
                        GovernedToolCall::Valid { tool, arguments },
                    )
                    .await?;
            },
            GovernedFrame::InvalidName { raw_name } => {
                governor
                    .enforce(
                        &self.caller,
                        &self.service,
                        GovernedToolCall::InvalidName {
                            raw_name: raw_name.as_deref(),
                        },
                    )
                    .await?;
                return Err(ProxyBackendError::Forbidden);
            },
        }
        Ok(
            parse_tool_call(body).map(|invocation| -> Box<dyn CallAudit> {
                Box::new(McpAudit::new(
                    self.audit.clone(),
                    self.caller.clone(),
                    McpServerId::new(self.service.as_str()),
                    invocation,
                ))
            }),
        )
    }
}

enum GovernedFrame {
    None,
    // JSON: MCP JSON-RPC request frame — forwarded verbatim to the external MCP
    // server.
    Valid(serde_json::Value),
    InvalidName { raw_name: Option<String> },
}

fn governed_call(service: &ServiceName, body: &[u8]) -> Result<GovernedFrame, ProxyBackendError> {
    if body.is_empty() {
        return Ok(GovernedFrame::None);
    }
    let value: serde_json::Value = serde_json::from_slice(body).map_err(|error| {
        tracing::debug!(%error, %service, "Invalid external MCP request");
        ProxyBackendError::Forbidden
    })?;
    if !value.is_object() {
        return Err(ProxyBackendError::Forbidden);
    }
    match classify_tool_call(body) {
        ToolCallFrame::Call(_) => Ok(GovernedFrame::Valid(value)),
        ToolCallFrame::InvalidName { raw_name } => Ok(GovernedFrame::InvalidName { raw_name }),
        ToolCallFrame::NotToolCall
            if value.get("method").and_then(serde_json::Value::as_str) == Some("tools/call") =>
        {
            Ok(GovernedFrame::InvalidName { raw_name: None })
        },
        ToolCallFrame::NotToolCall => Ok(GovernedFrame::None),
    }
}

#[must_use]
pub fn map_resolve_error(error: McpDomainError) -> ProxyBackendError {
    match error {
        McpDomainError::AuthRequired(_) => ProxyBackendError::AuthenticationRequired,
        McpDomainError::ExternalAccountNotConnected { .. } => {
            ProxyBackendError::ProviderNotConnected
        },
        McpDomainError::ExternalAuthUnavailable { message, .. } => {
            ProxyBackendError::Unavailable { status: message }
        },
        other => ProxyBackendError::ResolveFailed(Box::new(other)),
    }
}
