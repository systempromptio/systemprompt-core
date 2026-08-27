//! Cross-reference and port validation for [`ServicesConfig`].
//!
//! These private methods back [`ServicesConfig::validate`]: port conflict and
//! range checks, marketplace/plugin/skill `include` reference resolution, and
//! the single-default-agent and default-marketplace-selector business rules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bindings;

use std::collections::HashMap;

use crate::errors::ConfigValidationError;
use crate::mcp::McpServerType;

use super::ServicesConfig;

impl ServicesConfig {
    pub(crate) fn validate_ports(&self) -> Result<(), ConfigValidationError> {
        self.validate_port_conflicts()?;
        self.validate_port_ranges()?;
        self.validate_mcp_port_ranges()
    }

    pub(crate) fn validate_skills(&self) -> Result<(), ConfigValidationError> {
        for (key, skill) in &self.skills.skills {
            if !skill.id.as_str().is_empty() && skill.id.as_str() != key.as_str() {
                return Err(ConfigValidationError::invalid_field(format!(
                    "Skill map key '{}' does not match skill id '{}'",
                    key, skill.id
                )));
            }

            let id = key.as_str();
            if id.len() < 3 || id.len() > 64 {
                return Err(ConfigValidationError::invalid_field(format!(
                    "Skill '{key}': id must be between 3 and 64 characters"
                )));
            }
            if !id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return Err(ConfigValidationError::invalid_field(format!(
                    "Skill '{key}': id must be lowercase alphanumeric with underscores only \
                     (snake_case)"
                )));
            }

            for agent_ref in &skill.assigned_agents.include {
                if !self.agents.contains_key(agent_ref) {
                    tracing::warn!(
                        skill = %key,
                        agent = %agent_ref,
                        "Skill references agent that is not defined in services config"
                    );
                }
            }

            for mcp_ref in &skill.mcp_servers.include {
                if !self.mcp_servers.contains_key(mcp_ref) {
                    tracing::warn!(
                        skill = %key,
                        mcp_server = %mcp_ref,
                        "Skill references MCP server that is not defined in services config"
                    );
                }
            }
        }

        Ok(())
    }

    fn validate_port_conflicts(&self) -> Result<(), ConfigValidationError> {
        let mut seen_ports = HashMap::new();

        for (name, agent) in &self.agents {
            if let Some(existing) = seen_ports.insert(agent.port, ("agent", name.as_str())) {
                return Err(ConfigValidationError::port_conflict(format!(
                    "Port conflict: {} used by both {} '{}' and agent '{}'",
                    agent.port, existing.0, existing.1, name
                )));
            }
        }

        for (name, mcp) in &self.mcp_servers {
            if mcp.server_type == McpServerType::External {
                continue;
            }
            if let Some(existing) = seen_ports.insert(mcp.port, ("mcp_server", name.as_str())) {
                return Err(ConfigValidationError::port_conflict(format!(
                    "Port conflict: {} used by both {} '{}' and mcp_server '{}'",
                    mcp.port, existing.0, existing.1, name
                )));
            }
        }

        Ok(())
    }

    fn validate_port_ranges(&self) -> Result<(), ConfigValidationError> {
        let (min, max) = self.settings.agent_port_range;

        for (name, agent) in &self.agents {
            if agent.port < min || agent.port > max {
                return Err(ConfigValidationError::invalid_field(format!(
                    "Agent '{}' port {} is outside allowed range {}-{}",
                    name, agent.port, min, max
                )));
            }
        }

        Ok(())
    }

    fn validate_mcp_port_ranges(&self) -> Result<(), ConfigValidationError> {
        let (min, max) = self.settings.mcp_port_range;

        for (name, mcp) in &self.mcp_servers {
            if mcp.server_type == McpServerType::External {
                continue;
            }
            if mcp.port < min || mcp.port > max {
                return Err(ConfigValidationError::invalid_field(format!(
                    "MCP server '{}' port {} is outside allowed range {}-{}",
                    name, mcp.port, min, max
                )));
            }
        }

        Ok(())
    }
}
