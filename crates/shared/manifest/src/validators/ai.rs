//! AI configuration validator.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::validation_report::{ValidationIssue, ValidationReport, ValidationWarning};
use super::{DomainConfig, DomainConfigError, ValidationConfigProvider};
use crate::ServicesConfig;
use crate::services::CatalogContext;
use systemprompt_traits::ConfigProvider;

#[derive(Debug, Default)]
pub struct AiConfigValidator {
    config: Option<ServicesConfig>,
}

impl AiConfigValidator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DomainConfig for AiConfigValidator {
    fn domain_id(&self) -> &'static str {
        "ai"
    }

    fn priority(&self) -> u32 {
        50
    }

    fn dependencies(&self) -> &[&'static str] {
        &["mcp"]
    }

    fn load(&mut self, config: &dyn ConfigProvider) -> Result<(), DomainConfigError> {
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
        let mut report = ValidationReport::new("ai");
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| DomainConfigError::ValidationError {
                message: "Not loaded".into(),
            })?;
        let ai_config = &config.ai;

        Self::validate_catalog(&mut report, config);
        Self::validate_mcp_config(&mut report, ai_config);

        if ai_config.history.retention_days == 0 {
            report.add_warning(ValidationWarning::new(
                "ai.history.retention_days",
                "History retention set to 0 days, history will not be retained",
            ));
        }

        Ok(report)
    }
}

impl AiConfigValidator {
    fn validate_catalog(report: &mut ValidationReport, config: &ServicesConfig) {
        let catalog = match config.resolve_catalog(&CatalogContext::default()) {
            Ok(catalog) => catalog,
            Err(error) => {
                report.add_error(ValidationIssue::new("catalog", error.to_string()));
                return;
            },
        };
        if catalog.default_model().is_none() {
            report.add_error(
                ValidationIssue::new("gateway.default_model", "No default model configured")
                    .with_suggestion("Set gateway.default_model to a model in models.yaml"),
            );
        }
        if catalog.providers().all(|provider| !provider.enabled) {
            report.add_error(
                ValidationIssue::new("providers", "No AI providers are enabled")
                    .with_suggestion("Enable at least one provider in providers.yaml"),
            );
        }
    }

    fn validate_mcp_config(report: &mut ValidationReport, ai_config: &crate::AiConfig) {
        if ai_config.mcp.resilience.connect_timeout_ms == 0 {
            report.add_error(ValidationIssue::new(
                "ai.mcp.resilience.connect_timeout_ms",
                "MCP connect timeout must be greater than 0",
            ));
        }
        if ai_config.mcp.resilience.request_timeout_ms == 0 {
            report.add_error(ValidationIssue::new(
                "ai.mcp.resilience.request_timeout_ms",
                "MCP execution timeout must be greater than 0",
            ));
        }
    }
}
