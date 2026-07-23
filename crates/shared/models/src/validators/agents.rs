//! Domain validator for agent YAML: skills, models, card fields.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ValidationConfigProvider;
use crate::ServicesConfig;
use std::collections::HashMap;
use std::path::Path;
use systemprompt_traits::validation_report::{ValidationError, ValidationReport};
use systemprompt_traits::{ConfigProvider, DomainConfig, DomainConfigError};

#[derive(Debug, Default)]
pub struct AgentConfigValidator {
    config: Option<ServicesConfig>,
    skills_path: Option<String>,
}

impl AgentConfigValidator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DomainConfig for AgentConfigValidator {
    fn domain_id(&self) -> &'static str {
        "agents"
    }

    fn priority(&self) -> u32 {
        30
    }

    fn load(&mut self, config: &dyn ConfigProvider) -> Result<(), DomainConfigError> {
        let skills_path = config
            .get("skills_path")
            .ok_or_else(|| DomainConfigError::NotFound("skills_path not configured".into()))?;

        self.skills_path = Some(skills_path);

        let provider = config
            .as_any()
            .downcast_ref::<ValidationConfigProvider>()
            .ok_or_else(|| DomainConfigError::LoadError {
                message: "Expected ValidationConfigProvider with merged ServicesConfig".into(),
            })?;

        self.config = Some(provider.services_config().clone());
        Ok(())
    }

    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        let mut report = ValidationReport::new("agents");

        let config = self
            .config
            .as_ref()
            .ok_or_else(|| DomainConfigError::ValidationError {
                message: "Not loaded".into(),
            })?;

        let skills_path =
            self.skills_path
                .as_ref()
                .ok_or_else(|| DomainConfigError::ValidationError {
                    message: "Skills path not set".into(),
                })?;

        if !Path::new(skills_path).exists() {
            report.add_error(
                ValidationError::new("skills_path", "Skills directory does not exist")
                    .with_path(skills_path)
                    .with_suggestion("Create the skills directory"),
            );
        }

        Self::validate_port_uniqueness(config, &mut report);
        for (name, agent) in &config.agents {
            Self::validate_agent(name, agent, config, skills_path, &mut report);
        }

        Ok(report)
    }
}

impl AgentConfigValidator {
    fn validate_port_uniqueness(config: &ServicesConfig, report: &mut ValidationReport) {
        let mut used_ports: HashMap<u16, String> = HashMap::new();
        for (name, agent) in &config.agents {
            if let Some(existing) = used_ports.get(&agent.port) {
                report.add_error(
                    ValidationError::new(
                        format!("agents.{}.port", name),
                        format!("Port {} already used by agent '{}'", agent.port, existing),
                    )
                    .with_suggestion("Assign unique ports to each agent"),
                );
            } else {
                used_ports.insert(agent.port, name.clone());
            }
        }
    }

    fn validate_agent(
        name: &str,
        agent: &crate::services::AgentConfig,
        config: &ServicesConfig,
        skills_path: &str,
        report: &mut ValidationReport,
    ) {
        if agent.name.is_empty() {
            report.add_error(ValidationError::new(
                format!("agents.{}.name", name),
                "Agent name cannot be empty",
            ));
        }

        for skill_id in &agent.metadata.skills.include {
            let skill_path = Path::new(skills_path).join(skill_id);
            if !skill_path.exists() {
                report.add_error(
                    ValidationError::new(
                        format!("agents.{}.metadata.skills", name),
                        format!("Skill '{}' directory not found", skill_id),
                    )
                    .with_path(&skill_path)
                    .with_suggestion("Create the skill directory or remove it from the agent"),
                );
            }
        }

        for mcp_server in &agent.metadata.mcp_servers.include {
            Self::validate_agent_mcp_ref(name, agent, mcp_server, config, report);
        }
    }

    fn validate_agent_mcp_ref(
        name: &str,
        agent: &crate::services::AgentConfig,
        mcp_server: &str,
        config: &ServicesConfig,
        report: &mut ValidationReport,
    ) {
        let Some(mcp_config) = config.mcp_servers.get(mcp_server) else {
            report.add_error(
                ValidationError::new(
                    format!("agents.{}.metadata.mcp_servers", name),
                    format!("MCP server '{}' is not defined", mcp_server),
                )
                .with_suggestion(
                    "Define the MCP server in mcp_servers or remove it from the agent",
                ),
            );
            return;
        };

        if mcp_config.dev_only && !agent.dev_only {
            report.add_error(
                ValidationError::new(
                    format!("agents.{}.metadata.mcp_servers", name),
                    format!(
                        "Production agent '{}' references dev-only MCP server '{}'",
                        name, mcp_server
                    ),
                )
                .with_suggestion(
                    "Either mark the agent as dev_only: true, or use a production MCP server",
                ),
            );
        }
    }
}
