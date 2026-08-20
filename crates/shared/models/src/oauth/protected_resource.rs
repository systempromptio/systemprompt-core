//! RFC 9728 protected-resource metadata.
//!
//! One shape for both directions of the handshake: our routes serialise it to
//! answer `/.well-known/oauth-protected-resource`, and our MCP client
//! deserialises whatever a server points its `WWW-Authenticate` challenge at.
//! Keeping a single declaration is what stops the advertised document and the
//! parsed one drifting apart.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::mcp::McpExtensionId;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtectedResourceMetadata {
    pub resource: String,
    #[serde(default)]
    pub authorization_servers: Vec<String>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
    #[serde(default)]
    pub bearer_methods_supported: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_documentation: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_extensions_supported: Vec<McpExtensionId>,
}

impl ProtectedResourceMetadata {
    #[must_use]
    pub fn requires_enterprise_managed_auth(&self) -> bool {
        self.mcp_extensions_supported
            .contains(&McpExtensionId::EnterpriseManagedAuth)
    }
}
