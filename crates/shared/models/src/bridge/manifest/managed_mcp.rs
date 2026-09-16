//! The MCP servers a manifest provisions into a managed client, with the
//! per-tool policy map the bridge enforces.
//!
//! `tool_policy` decides per tool; the key `*`
//! ([`ManagedMcpServer::TOOL_POLICY_WILDCARD`]) stands for every tool the
//! server exposes, and a tool with neither its own entry nor a wildcard
//! resolves to `None`, in which case the client keeps its own default.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::bridge::ids::{ManagedMcpServerName, ToolName, ToolPolicy};
use systemprompt_identifiers::{McpServerId, ValidatedUrl};

/// An MCP server the bridge provisions into a managed client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "ManagedMcpServerWire")]
pub struct ManagedMcpServer {
    pub id: McpServerId,
    pub name: ManagedMcpServerName,
    pub url: ValidatedUrl,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_policy: Option<BTreeMap<ToolName, ToolPolicy>>,
}

impl ManagedMcpServer {
    pub const TOOL_POLICY_WILDCARD: &'static str = "*";

    #[must_use]
    pub fn policy_for_tool(&self, tool: &str) -> Option<ToolPolicy> {
        let map = self.tool_policy.as_ref()?;
        map.iter()
            .find(|(name, _)| name.as_str() == tool)
            .or_else(|| {
                map.iter()
                    .find(|(name, _)| name.as_str() == Self::TOOL_POLICY_WILDCARD)
            })
            .map(|(_, policy)| *policy)
    }

    #[must_use]
    pub fn default_tool_policy(&self) -> Option<ToolPolicy> {
        self.policy_for_tool(Self::TOOL_POLICY_WILDCARD)
    }
}

#[derive(Deserialize)]
struct ManagedMcpServerWire {
    #[serde(default)]
    id: Option<McpServerId>,
    name: ManagedMcpServerName,
    url: ValidatedUrl,
    #[serde(default)]
    transport: Option<String>,
    #[serde(default)]
    headers: Option<BTreeMap<String, String>>,
    #[serde(default)]
    oauth: Option<bool>,
    #[serde(default)]
    tool_policy: Option<BTreeMap<ToolName, ToolPolicy>>,
}

impl TryFrom<ManagedMcpServerWire> for ManagedMcpServer {
    type Error = systemprompt_identifiers::error::IdValidationError;

    fn try_from(wire: ManagedMcpServerWire) -> Result<Self, Self::Error> {
        let id = match wire.id {
            Some(id) => id,
            None => McpServerId::try_new(wire.name.as_str())?,
        };
        Ok(Self {
            id,
            name: wire.name,
            url: wire.url,
            transport: wire.transport,
            headers: wire.headers,
            oauth: wire.oauth,
            tool_policy: wire.tool_policy,
        })
    }
}
