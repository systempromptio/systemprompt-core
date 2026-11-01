//! The tools a host serves and the access each one resolves to.
//!
//! [`HostedCatalog::build`] fixes, once at start, which toolset owns each tool
//! name and refuses a configuration that cannot be served: two toolsets
//! claiming one name, or a `tools:` entry naming a tool no hosted toolset
//! serves (a per-tool access or policy rule that would silently apply to
//! nothing). Access itself is resolved per request from the services
//! configuration current then, so an access edit takes effect without a
//! restart.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use rmcp::model::Tool;
use systemprompt_identifiers::McpServerId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::mcp::{Deployment, ResolvedAccess};

use crate::toolset::McpToolset;

#[derive(Debug, thiserror::Error)]
pub enum ToolsetHostError {
    #[error("MCP server '{server}' is not declared in the services configuration")]
    UnknownServer { server: McpServerId },
    #[error("MCP server '{server}' hosts no toolsets; list them under `toolsets:`")]
    NoToolsets { server: McpServerId },
    #[error("MCP server '{server}': tool '{tool}' is served by both '{first}' and '{second}'")]
    DuplicateTool {
        server: McpServerId,
        tool: String,
        first: String,
        second: String,
    },
    #[error(
        "MCP server '{server}': `tools.{tool}` names a tool no hosted toolset serves (hosted: \
         {hosted})"
    )]
    UnhostedTool {
        server: McpServerId,
        tool: String,
        hosted: String,
    },
}

/// One served tool and the index of the toolset that owns it.
#[derive(Debug, Clone)]
pub struct HostedTool {
    pub tool: Tool,
    pub toolset: usize,
}

#[derive(Debug, Clone, Default)]
pub struct HostedCatalog {
    tools: BTreeMap<String, HostedTool>,
}

impl HostedCatalog {
    pub fn build(
        server: &McpServerId,
        deployment: &Deployment,
        toolsets: &[Box<dyn McpToolset>],
    ) -> Result<Self, ToolsetHostError> {
        if toolsets.is_empty() {
            return Err(ToolsetHostError::NoToolsets {
                server: server.clone(),
            });
        }
        let mut tools: BTreeMap<String, HostedTool> = BTreeMap::new();
        for (index, toolset) in toolsets.iter().enumerate() {
            for tool in toolset.tools() {
                let name = tool.name.to_string();
                if let Some(existing) = tools.get(&name) {
                    return Err(ToolsetHostError::DuplicateTool {
                        server: server.clone(),
                        tool: name,
                        first: toolsets[existing.toolset].id().to_owned(),
                        second: toolset.id().to_owned(),
                    });
                }
                tools.insert(
                    name,
                    HostedTool {
                        tool,
                        toolset: index,
                    },
                );
            }
        }
        let catalog = Self { tools };
        catalog.check_declared_tools(server, deployment)?;
        Ok(catalog)
    }

    fn check_declared_tools(
        &self,
        server: &McpServerId,
        deployment: &Deployment,
    ) -> Result<(), ToolsetHostError> {
        let mut declared: Vec<&String> = deployment.tools.keys().collect();
        declared.sort();
        declared
            .into_iter()
            .find(|tool| !self.tools.contains_key(tool.as_str()))
            .map_or(Ok(()), |tool| {
                Err(ToolsetHostError::UnhostedTool {
                    server: server.clone(),
                    tool: tool.clone(),
                    hosted: self.names().join(", "),
                })
            })
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&HostedTool> {
        self.tools.get(name)
    }

    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(String::as_str).collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &HostedTool)> {
        self.tools.iter().map(|(name, tool)| (name.as_str(), tool))
    }

    #[must_use]
    pub fn access_for<'a>(
        &self,
        toolsets: &[Box<dyn McpToolset>],
        deployment: &'a Deployment,
        tool: &str,
    ) -> Option<ResolvedAccess<'a>> {
        let hosted = self.tools.get(tool)?;
        let toolset = toolsets.get(hosted.toolset)?.id();
        Some(resolve_tool_access(deployment, toolset, tool))
    }

    #[must_use]
    pub fn needs_groups(&self, toolsets: &[Box<dyn McpToolset>], deployment: &Deployment) -> bool {
        self.tools.keys().any(|name| {
            self.access_for(toolsets, deployment, name)
                .is_some_and(|access| access.needs_groups())
        })
    }

    #[must_use]
    pub fn visible_tools(
        &self,
        toolsets: &[Box<dyn McpToolset>],
        deployment: &Deployment,
        user: Option<&AuthenticatedUser>,
        groups: &[String],
    ) -> Vec<Tool> {
        self.tools
            .iter()
            .filter(|(name, _)| {
                self.access_for(toolsets, deployment, name)
                    .is_some_and(|access| access.permits(user, groups))
            })
            .map(|(_, hosted)| hosted.tool.clone())
            .collect()
    }
}

#[must_use]
pub fn resolve_tool_access<'a>(
    deployment: &'a Deployment,
    toolset_id: &str,
    tool: &str,
) -> ResolvedAccess<'a> {
    let tool_access = deployment
        .tools
        .get(tool)
        .and_then(|meta| meta.access.as_ref());
    let toolset_access = deployment
        .toolsets
        .iter()
        .find(|toolset| toolset.id == toolset_id)
        .and_then(|toolset| toolset.access.as_ref());
    ResolvedAccess::resolve(tool_access, toolset_access)
}
