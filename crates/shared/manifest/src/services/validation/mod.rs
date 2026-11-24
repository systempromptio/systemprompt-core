//! Cross-reference and port validation for [`ServicesConfig`].
//!
//! These private methods back [`ServicesConfig::validate`]: port conflict and
//! range checks, marketplace/plugin/skill `include` reference resolution, and
//! the single-default-agent and default-marketplace-selector business rules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bindings;
mod owners;

use std::collections::HashMap;

use systemprompt_models::errors::{ServicesValidationError, ValidationSubject};
use systemprompt_models::mcp::McpServerType;

use super::ServicesConfig;

impl ServicesConfig {
    pub(crate) fn validate_ports(&self) -> Result<(), ServicesValidationError> {
        self.validate_port_conflicts()?;
        self.validate_port_ranges()?;
        self.validate_mcp_port_ranges()
    }

    pub(crate) fn validate_skills(&self) -> Result<(), ServicesValidationError> {
        for (key, skill) in &self.skills.skills {
            if !skill.id.as_str().is_empty() && skill.id.as_str() != key.as_str() {
                return Err(ServicesValidationError::invalid_field(
                    ValidationSubject::clause("Skill map key", key),
                    format!("does not match skill id '{}'", skill.id),
                ));
            }

            let id = key.as_str();
            if id.len() < 3 || id.len() > 64 {
                return Err(ServicesValidationError::invalid_field(
                    ValidationSubject::named("Skill", key),
                    "id must be between 3 and 64 characters",
                ));
            }
            if !id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return Err(ServicesValidationError::invalid_field(
                    ValidationSubject::named("Skill", key),
                    "id must be lowercase alphanumeric with underscores only \
                     (snake_case)",
                ));
            }

            for mcp_ref in &skill.mcp_servers.include {
                match self.mcp_servers.get(mcp_ref) {
                    None => {
                        return Err(ServicesValidationError::unknown_reference(
                            ValidationSubject::named("Skill", key),
                            format!(
                                "mcp_servers.include references unknown mcp_server \
                             '{mcp_ref}'"
                            ),
                        ));
                    },
                    Some(deployment) if skill.enabled && !deployment.enabled => {
                        return Err(ServicesValidationError::business_rule(
                            ValidationSubject::clause("Skill", key),
                            format!(
                                "is enabled but depends on disabled mcp_server \
                             '{mcp_ref}' — enable the server or disable the skill"
                            ),
                        ));
                    },
                    Some(_) => {},
                }
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
        }

        Ok(())
    }

    fn validate_port_conflicts(&self) -> Result<(), ServicesValidationError> {
        let mut seen_ports = HashMap::new();

        for (name, agent) in &self.agents {
            if let Some(existing) = seen_ports.insert(agent.port, ("agent", name.as_str())) {
                return Err(ServicesValidationError::port_conflict(
                    agent.port,
                    existing,
                    ("agent", name),
                ));
            }
        }

        for (name, mcp) in &self.mcp_servers {
            if mcp.server_type == McpServerType::External {
                continue;
            }
            let Some(port) = mcp.port else {
                continue;
            };
            if let Some(existing) = seen_ports.insert(port, ("mcp_server", name.as_str())) {
                return Err(ServicesValidationError::port_conflict(
                    port,
                    existing,
                    ("mcp_server", name),
                ));
            }
        }

        Ok(())
    }

    fn validate_port_ranges(&self) -> Result<(), ServicesValidationError> {
        let (min, max) = self.settings.agent_port_range;

        for (name, agent) in &self.agents {
            if agent.port < min || agent.port > max {
                return Err(ServicesValidationError::invalid_field(
                    ValidationSubject::clause("Agent", name),
                    format!(
                        "port {} is outside allowed range {}-{}",
                        agent.port, min, max
                    ),
                ));
            }
        }

        Ok(())
    }

    fn validate_mcp_port_ranges(&self) -> Result<(), ServicesValidationError> {
        let (min, max) = self.settings.mcp_port_range;

        for (name, mcp) in &self.mcp_servers {
            if mcp.server_type == McpServerType::External {
                continue;
            }
            let Some(port) = mcp.port else {
                continue;
            };
            if port < min || port > max {
                return Err(ServicesValidationError::invalid_field(
                    ValidationSubject::clause("MCP server", name),
                    format!("port {} is outside allowed range {}-{}", port, min, max),
                ));
            }
        }

        Ok(())
    }
}
