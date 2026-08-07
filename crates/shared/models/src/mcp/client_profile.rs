//! Negotiated MCP client identity used to shape tool results per client.
//!
//! [`ClientProfile`] captures what a connected client declared during
//! `initialize`: its protocol version, implementation name, and negotiated
//! extension keys. The response builder consults it to decide which wire
//! pieces a client can accept — embedded UI resources, `structuredContent`,
//! and custom `_meta`. An absent or unparseable declaration yields
//! [`ClientProfile::unknown`], which downgrades the result to the
//! plain-text shape every conforming client accepts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::model::{InitializeRequestParams, ProtocolVersion};
use std::collections::BTreeSet;

use super::capabilities::McpExtensionId;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientProfile {
    pub protocol_version: Option<ProtocolVersion>,
    pub client_name: Option<String>,
    pub extensions: BTreeSet<String>,
}

impl ClientProfile {
    #[must_use]
    pub fn unknown() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_initialize_params(params: &InitializeRequestParams) -> Self {
        Self {
            protocol_version: Some(params.protocol_version.clone()),
            client_name: Some(params.client_info.name.clone()),
            extensions: params
                .capabilities
                .extensions
                .as_ref()
                .map(|exts| exts.keys().cloned().collect())
                .unwrap_or_default(),
        }
    }

    #[must_use]
    pub fn supports_ui(&self) -> bool {
        self.extensions.contains(McpExtensionId::McpAppsUi.as_str())
    }

    #[must_use]
    pub fn supports_structured_content(&self) -> bool {
        self.protocol_version
            .as_ref()
            .is_some_and(|v| *v >= ProtocolVersion::V_2025_06_18)
    }
}
