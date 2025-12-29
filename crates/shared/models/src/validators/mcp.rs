//! MCP configuration validator.

use super::ValidationConfigProvider;
use crate::ServicesConfig;
use std::collections::HashMap;
use systemprompt_traits::validation_report::{ValidationError, ValidationReport};
use systemprompt_traits::{ConfigProvider, DomainConfig, DomainConfigError};

#[derive(Debug, Default)]
pub struct McpConfigValidator {
    services_config: Option<ServicesConfig>,
}

impl McpConfigValidator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DomainConfig for McpConfigValidator {
    fn domain_id(&self) -> &'static str {
        "mcp"
    }

    fn priority(&self) -> u32 {
        40
    }

    fn dependencies(&self) -> &[&'static str] {
        &["agents"]
    }

    fn load(&mut self, config: &dyn ConfigProvider) -> Result<(), DomainConfigError> {
        let provider = config
            .as_any()
            .downcast_ref::<ValidationConfigProvider>()
            .ok_or_else(|| {
                DomainConfigError::LoadError(
                    "Expected ValidationConfigProvider with pre-loaded configs".into(),
                )
            })?;

        self.services_config = Some(provider.services_config().clone());
        Ok(())
    }

    fn validate(&self) -> Result<ValidationReport, DomainConfigError> {
        let mut report = ValidationReport::new("mcp");

        let Some(config) = self.services_config.as_ref() else {
            return Ok(report);
        };

        let mut used_ports: HashMap<u16, String> = HashMap::new();

        for (name, deployment) in &config.mcp_servers {
            let port = deployment.port;

            if let Some(existing) = used_ports.get(&port) {
                report.add_error(
                    ValidationError::new(
                        format!("mcp_servers.{}.port", name),
                        format!("Port {} already used by server '{}'", port, existing),
                    )
                    .with_suggestion("Assign unique ports to each MCP server"),
                );
            } else {
                used_ports.insert(port, name.clone());
            }
        }

        Ok(report)
    }
}
