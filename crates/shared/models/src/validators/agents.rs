//! Agent configuration validator.

use super::ValidationConfigProvider;
use crate::ServicesConfig;
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
            .ok_or_else(|| {
                DomainConfigError::LoadError(
                    "Expected ValidationConfigProvider with merged ServicesConfig".into(),
                )
            })?;

        self.config = Some(provider.services_config().clone());
        Ok(())
    }

    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        let mut report = ValidationReport::new("agents");

        let config = self
            .config
            .as_ref()
            .ok_or_else(|| DomainConfigError::ValidationError("Not loaded".into()))?;

        let skills_path = self
            .skills_path
            .as_ref()
            .ok_or_else(|| DomainConfigError::ValidationError("Skills path not set".into()))?;

        if !Path::new(skills_path).exists() {
            report.add_error(
                ValidationError::new("skills_path", "Skills directory does not exist")
                    .with_path(skills_path)
                    .with_suggestion("Create the skills directory"),
            );
        }

        for (name, agent) in &config.agents {
            if agent.name.is_empty() {
                report.add_error(ValidationError::new(
                    format!("agents.{}.name", name),
                    "Agent name cannot be empty",
                ));
            }

            for skill in &agent.card.skills {
                let skill_id = &skill.id;
                let skill_path = Path::new(skills_path).join(skill_id);
                if !skill_path.exists() {
                    report.add_error(
                        ValidationError::new(
                            format!("agents.{}.skills", name),
                            format!("Skill '{}' directory not found", skill_id),
                        )
                        .with_path(&skill_path)
                        .with_suggestion("Create the skill directory or remove it from the agent"),
                    );
                }
            }

            // Validate metadata.skills references
            for skill_id in &agent.metadata.skills {
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

            // Validate metadata.mcp_servers references
            for mcp_server in &agent.metadata.mcp_servers {
                if !config.mcp_servers.contains_key(mcp_server) {
                    report.add_error(
                        ValidationError::new(
                            format!("agents.{}.metadata.mcp_servers", name),
                            format!("MCP server '{}' is not defined", mcp_server),
                        )
                        .with_suggestion(
                            "Define the MCP server in mcp_servers or remove it from the agent",
                        ),
                    );
                }
            }
        }

        Ok(report)
    }
}
